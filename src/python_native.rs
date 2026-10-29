//! Python binding of the native codec: protobuf bytes <-> Python objects (dict,
//! list, int, float, str, bytes, bool, datetime, date, timedelta) and Pydantic
//! models, without JSON.
//!
//! Decoding modes:
//! - `mode="dict"` (or no model): nested dicts, no Pydantic at all;
//! - `mode="validate"`: dicts validated by the model's pydantic-core validator, by
//!   field name;
//! - `mode="construct"`: model instances built directly, nested models and enum
//!   members included, without validation (like `model_construct`).
//!
//! Field names: the proto name by default, or the Python name given in
//! `field_names` (`{message full name: {field number: Python name}}`).

use crate::descriptor_resolver::DescriptorResolver;
use crate::native::plan::{CodecPlan, EnumPlan, FieldPlan, FieldType, Scalar, Value, WellKnown};
use crate::native::wire::{
    WIRE_I64, WIRE_LEN, WIRE_VARINT, encode_tag, read_fixed32, read_fixed64, read_len, read_tag,
    read_varint, skip_field, unzigzag32, unzigzag64, write_len_delimited, write_varint, zigzag32,
    zigzag64,
};
use crate::native::wkt;
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::ffi;
use pyo3::intern;
use pyo3::prelude::*;
use pyo3::types::{
    PyBool, PyBytes, PyDate, PyDateAccess, PyDateTime, PyDelta, PyDeltaAccess, PyDict, PyFloat,
    PyInt, PyList, PySet, PyString, PyTimeAccess, PyTuple, PyType, PyTzInfo, PyTzInfoAccess,
};
use std::collections::{HashMap, HashSet};

/// `{message full name: {field number: Python name}}`.
type FieldNames = HashMap<String, HashMap<u32, String>>;

/// Same nesting limit as the reference protobuf implementations.
const MAX_DEPTH: u32 = 100;

// Default kinds returned by `protoruf._native.construct_info`.
const DEFAULT_SHARED: u8 = 0;
const DEFAULT_COPIED: u8 = 1;
const DEFAULT_FACTORY: u8 = 2;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Validate,
    Construct,
    Dict,
}

/// Python-side data attached to each planned field.
struct PyField {
    /// Interned field name: dict lookups hit the cached hash.
    key: Py<PyString>,
    /// Value filled in when an implicit-presence scalar is absent on the wire.
    zero: Option<Py<PyAny>>,
    /// Enum class of the model field (construct mode only).
    enum_class: Option<EnumClass>,
    /// Whether the bound model declares this field (construct mode only).
    in_model: bool,
}

struct EnumClass {
    class: Py<PyAny>,
    /// The class's live `_value2member_map_`, so pseudo-members created later by an
    /// open enum are found too.
    members: Py<PyDict>,
}

enum Default {
    Shared(Py<PyAny>),
    Copied(Py<PyAny>),
    Factory(Py<PyAny>),
    Required,
}

/// How to build instances of the model bound to a message (construct mode).
struct Construct {
    class: Py<PyType>,
    /// Assemble the instance directly; otherwise call `model_construct`.
    fast: bool,
    /// The model declares exactly the message fields, in field-number order: the
    /// decoded values can be written straight into `__dict__`, in order.
    same_order: bool,
    extra_allow: bool,
    /// Every model field, in declaration order.
    defaults: Vec<(Py<PyString>, Default)>,
}

/// A field's decoded value while a message is being read.
enum Slot<'py> {
    Empty,
    Value(Bound<'py, PyAny>),
    List(Bound<'py, PyList>),
    Map(Bound<'py, PyDict>),
}

struct PyMessage {
    fields: Vec<PyField>,
    construct: Option<Construct>,
}

/// `(fast, extra_allow, [(field name, default kind, value)])` from
/// `protoruf._native.construct_info`.
type ConstructInfo<'py> = (bool, bool, Vec<(String, u8, Bound<'py, PyAny>)>);

/// Python objects looked up once.
struct PyCache {
    utc: Py<PyTzInfo>,
    deepcopy: Py<PyAny>,
    /// `{"by_name": True}`: decoded dicts are keyed by field name, not alias.
    by_name: Py<PyDict>,
}

/// A scalar value read from Python, ready to be written.
enum Encoded<'a> {
    Varint(u64),
    I32(u32),
    I64(u64),
    Len(&'a [u8]),
}

impl Encoded<'_> {
    fn is_zero(&self) -> bool {
        match self {
            Encoded::Varint(v) | Encoded::I64(v) => *v == 0,
            Encoded::I32(v) => *v == 0,
            Encoded::Len(b) => b.is_empty(),
        }
    }

    fn write(&self, buf: &mut Vec<u8>) {
        match self {
            Encoded::Varint(v) => write_varint(buf, *v),
            Encoded::I32(v) => buf.extend_from_slice(&v.to_le_bytes()),
            Encoded::I64(v) => buf.extend_from_slice(&v.to_le_bytes()),
            Encoded::Len(b) => {
                write_varint(buf, b.len() as u64);
                buf.extend_from_slice(b);
            }
        }
    }
}

fn decode_error(message: impl std::fmt::Display) -> PyErr {
    PyValueError::new_err(format!("protobuf decode error: {message}"))
}

fn field_error(py: Python<'_>, message: &str, field: &FieldPlan, err: PyErr) -> PyErr {
    let kind = if err.is_instance_of::<PyTypeError>(py) {
        PyTypeError::new_err
    } else {
        PyValueError::new_err
    };
    kind(format!("{message}.{}: {}", field.name, err.value(py)))
}

fn type_name(value: &Bound<'_, PyAny>) -> String {
    value
        .get_type()
        .name()
        .map_or_else(|_| "?".into(), |n| n.to_string())
}

fn field_value(ty: FieldType) -> Value {
    match ty {
        FieldType::Single(value) => value,
        FieldType::Repeated { value, .. } | FieldType::Map { value, .. } => value,
    }
}

fn scalar_zero(py: Python<'_>, scalar: Scalar) -> Bound<'_, PyAny> {
    match scalar {
        Scalar::Double | Scalar::Float => PyFloat::new(py, 0.0).into_any(),
        Scalar::Bool => PyBool::new(py, false).to_owned().into_any(),
        Scalar::String => PyString::new(py, "").into_any(),
        Scalar::Bytes => PyBytes::new(py, b"").into_any(),
        _ => 0i64.into_pyobject(py).unwrap().into_any(),
    }
}

fn zero_value(py: Python<'_>, field: &FieldPlan) -> Option<Py<PyAny>> {
    match field.ty {
        FieldType::Single(Value::Scalar(scalar)) if !field.explicit_presence => {
            Some(scalar_zero(py, scalar).unbind())
        }
        _ => None,
    }
}

/// Read a non-enum scalar.
fn read_plain_scalar<'py>(
    py: Python<'py>,
    scalar: Scalar,
    data: &[u8],
    pos: &mut usize,
) -> PyResult<Bound<'py, PyAny>> {
    let obj = match scalar {
        Scalar::Int32 | Scalar::Enum(_) => {
            let v = read_varint(data, pos).map_err(decode_error)? as i32;
            v.into_pyobject(py)?.into_any()
        }
        Scalar::Int64 => (read_varint(data, pos).map_err(decode_error)? as i64)
            .into_pyobject(py)?
            .into_any(),
        Scalar::Uint32 => (read_varint(data, pos).map_err(decode_error)? as u32)
            .into_pyobject(py)?
            .into_any(),
        Scalar::Uint64 => read_varint(data, pos)
            .map_err(decode_error)?
            .into_pyobject(py)?
            .into_any(),
        Scalar::Sint32 => unzigzag32(read_varint(data, pos).map_err(decode_error)?)
            .into_pyobject(py)?
            .into_any(),
        Scalar::Sint64 => unzigzag64(read_varint(data, pos).map_err(decode_error)?)
            .into_pyobject(py)?
            .into_any(),
        Scalar::Bool => {
            let v = read_varint(data, pos).map_err(decode_error)? != 0;
            PyBool::new(py, v).to_owned().into_any()
        }
        Scalar::Fixed32 => read_fixed32(data, pos)
            .map_err(decode_error)?
            .into_pyobject(py)?
            .into_any(),
        Scalar::Sfixed32 => (read_fixed32(data, pos).map_err(decode_error)? as i32)
            .into_pyobject(py)?
            .into_any(),
        Scalar::Float => {
            let v = f32::from_bits(read_fixed32(data, pos).map_err(decode_error)?);
            PyFloat::new(py, f64::from(v)).into_any()
        }
        Scalar::Fixed64 => read_fixed64(data, pos)
            .map_err(decode_error)?
            .into_pyobject(py)?
            .into_any(),
        Scalar::Sfixed64 => (read_fixed64(data, pos).map_err(decode_error)? as i64)
            .into_pyobject(py)?
            .into_any(),
        Scalar::Double => {
            let v = f64::from_bits(read_fixed64(data, pos).map_err(decode_error)?);
            PyFloat::new(py, v).into_any()
        }
        Scalar::String => {
            let bytes = read_len(data, pos).map_err(decode_error)?;
            let text = std::str::from_utf8(bytes).map_err(decode_error)?;
            PyString::new(py, text).into_any()
        }
        Scalar::Bytes => PyBytes::new(py, read_len(data, pos).map_err(decode_error)?).into_any(),
    };
    Ok(obj)
}

/// Read the `seconds` (1) / `nanos` (2) pair shared by Timestamp and Duration.
fn read_seconds_nanos(payload: &[u8]) -> PyResult<(i64, i32)> {
    let (mut seconds, mut nanos) = (0i64, 0i32);
    let mut pos = 0;
    while pos < payload.len() {
        let (number, wire_type) = read_tag(payload, &mut pos).map_err(decode_error)?;
        match (number, wire_type) {
            (1, WIRE_VARINT) => {
                seconds = read_varint(payload, &mut pos).map_err(decode_error)? as i64
            }
            (2, WIRE_VARINT) => {
                nanos = read_varint(payload, &mut pos).map_err(decode_error)? as i32
            }
            _ => skip_field(payload, &mut pos, wire_type).map_err(decode_error)?,
        }
    }
    Ok((seconds, nanos))
}

fn write_tag(buf: &mut Vec<u8>, number: u32, wire_type: u8) {
    write_varint(buf, encode_tag(number, wire_type));
}

/// Encode/decode one message type (and the types it references) natively.
///
/// `model` is an optional Pydantic model class. Options:
/// - `mode`: `"validate"` (default), `"construct"` (no validation; needs `model`) or
///   `"dict"` (plain dicts, the model is ignored);
/// - `field_names`: `{message full name: {field number: Python name}}`, for fields
///   whose Python name is not the proto one;
/// - `unknown_enum`: `"keep"` (default, proto3 open enums) or `"error"`;
/// - `naive_datetime`: `"error"` (default) or `"utc"` for `Timestamp` fields.
#[pyclass(frozen, name = "Codec", module = "protoruf._protoruf")]
pub struct NativeCodec {
    plan: CodecPlan,
    messages: Vec<PyMessage>,
    validate: Option<Py<PyAny>>,
    mode: Mode,
    strict_enums: bool,
    naive_utc: bool,
    cache: PyCache,
}

#[pymethods]
impl NativeCodec {
    #[new]
    #[pyo3(signature = (
        descriptor_bytes,
        message_type,
        model = None,
        *,
        mode = "validate",
        field_names = None,
        unknown_enum = "keep",
        naive_datetime = "error",
    ))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        py: Python<'_>,
        descriptor_bytes: &[u8],
        message_type: &str,
        model: Option<&Bound<'_, PyAny>>,
        mode: &str,
        field_names: Option<FieldNames>,
        unknown_enum: &str,
        naive_datetime: &str,
    ) -> PyResult<Self> {
        let mode = match mode {
            "validate" => Mode::Validate,
            "construct" => Mode::Construct,
            "dict" => Mode::Dict,
            other => {
                return Err(PyValueError::new_err(format!(
                    "mode must be 'validate', 'construct' or 'dict', got {other:?}"
                )));
            }
        };
        let strict_enums = match unknown_enum {
            "keep" => false,
            "error" => true,
            other => {
                return Err(PyValueError::new_err(format!(
                    "unknown_enum must be 'keep' or 'error', got {other:?}"
                )));
            }
        };
        let naive_utc = match naive_datetime {
            "error" => false,
            "utc" => true,
            other => {
                return Err(PyValueError::new_err(format!(
                    "naive_datetime must be 'error' or 'utc', got {other:?}"
                )));
            }
        };
        if mode == Mode::Construct && model.is_none() {
            return Err(PyValueError::new_err("mode='construct' requires a model"));
        }

        let resolver = DescriptorResolver::from_descriptor_bytes(descriptor_bytes)
            .map_err(PyValueError::new_err)?;
        let descriptor = resolver
            .resolve(message_type)
            .map_err(PyValueError::new_err)?;
        let plan = CodecPlan::build(&descriptor).map_err(PyValueError::new_err)?;
        let field_names = field_names.unwrap_or_default();
        let mut messages: Vec<PyMessage> = plan
            .messages
            .iter()
            .map(|message| {
                let names = field_names.get(&message.full_name);
                PyMessage {
                    fields: message
                        .fields
                        .iter()
                        .map(|field| {
                            let name = names
                                .and_then(|names| names.get(&field.number))
                                .unwrap_or(&field.name);
                            PyField {
                                key: PyString::intern(py, name).unbind(),
                                zero: zero_value(py, field),
                                enum_class: None,
                                in_model: true,
                            }
                        })
                        .collect(),
                    construct: None,
                }
            })
            .collect();

        let mut validate = None;
        if let Some(model) = model {
            match mode {
                Mode::Validate => {
                    validate = Some(
                        model
                            .getattr(intern!(py, "__pydantic_validator__"))?
                            .getattr(intern!(py, "validate_python"))?
                            .unbind(),
                    );
                }
                Mode::Construct => bind_models(py, &plan, &mut messages, model)?,
                Mode::Dict => {}
            }
        }

        let by_name = PyDict::new(py);
        by_name.set_item(intern!(py, "by_name"), true)?;
        let cache = PyCache {
            utc: PyTzInfo::utc(py)?.to_owned().unbind(),
            deepcopy: PyModule::import(py, "copy")?.getattr("deepcopy")?.unbind(),
            by_name: by_name.unbind(),
        };
        Ok(Self {
            plan,
            messages,
            validate,
            mode,
            strict_enums,
            naive_utc,
            cache,
        })
    }

    /// Decode bytes into a model instance (validated, or constructed without
    /// validation in construct mode), or into a dict without a model.
    fn decode<'py>(&self, py: Python<'py>, data: &[u8]) -> PyResult<Bound<'py, PyAny>> {
        if self.mode == Mode::Construct {
            return self.decode_message(py, 0, data, 0, true);
        }
        let dict = self.decode_message(py, 0, data, 0, false)?;
        match &self.validate {
            Some(validate) => validate
                .bind(py)
                .call((dict,), Some(self.cache.by_name.bind(py))),
            None => Ok(dict),
        }
    }

    /// Decode bytes into nested dicts, without any validation.
    fn decode_dict<'py>(&self, py: Python<'py>, data: &[u8]) -> PyResult<Bound<'py, PyAny>> {
        self.decode_message(py, 0, data, 0, false)
    }

    /// Encode a model instance (read through its `__dict__`) or a dict.
    fn encode<'py>(
        &self,
        py: Python<'py>,
        obj: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let mut buf = Vec::with_capacity(128);
        self.encode_message(py, 0, obj, &mut buf, 0)?;
        Ok(PyBytes::new(py, &buf))
    }
}

/// Bind model and enum classes to the plan (construct mode), following the field
/// annotations from the root model.
fn bind_models(
    py: Python<'_>,
    plan: &CodecPlan,
    messages: &mut [PyMessage],
    model: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let helpers = PyModule::import(py, "protoruf._native")?;
    let links = helpers
        .call_method1("model_links", (model,))?
        .cast_into::<PyDict>()?;

    let mut classes: Vec<Option<Bound<'_, PyAny>>> = vec![None; plan.messages.len()];
    classes[0] = Some(model.clone());
    let mut pending = vec![0usize];
    while let Some(index) = pending.pop() {
        let Some(class) = classes[index].clone() else {
            continue;
        };
        let Some(targets) = links.get_item(&class)? else {
            continue;
        };
        let targets = targets.cast_into::<PyDict>()?;
        for (i, field) in plan.messages[index].fields.iter().enumerate() {
            let key = messages[index].fields[i].key.clone_ref(py);
            let Some(target) = targets.get_item(key.bind(py))? else {
                continue;
            };
            match field_value(field.ty) {
                Value::Message(child) if classes[child].is_none() => {
                    classes[child] = Some(target);
                    pending.push(child);
                }
                Value::Scalar(Scalar::Enum(_)) => {
                    let members = target
                        .getattr(intern!(py, "_value2member_map_"))?
                        .cast_into::<PyDict>()?;
                    messages[index].fields[i].enum_class = Some(EnumClass {
                        class: target.unbind(),
                        members: members.unbind(),
                    });
                }
                _ => {}
            }
        }
    }

    for (index, class) in classes.into_iter().enumerate() {
        let Some(class) = class else {
            continue;
        };
        let (fast, extra_allow, raw_defaults): ConstructInfo<'_> = helpers
            .call_method1("construct_info", (&class,))?
            .extract()?;
        let names: HashSet<&str> = raw_defaults.iter().map(|(n, _, _)| n.as_str()).collect();
        let keys: Vec<String> = messages[index]
            .fields
            .iter()
            .map(|f| f.key.bind(py).to_string())
            .collect();
        for (py_field, key) in messages[index].fields.iter_mut().zip(&keys) {
            py_field.in_model = names.contains(key.as_str());
        }
        let same_order = raw_defaults
            .iter()
            .map(|(name, _, _)| name.as_str())
            .eq(keys.iter().map(String::as_str));
        let defaults = raw_defaults
            .iter()
            .map(|(name, kind, value)| {
                let default = match *kind {
                    DEFAULT_SHARED => Default::Shared(value.clone().unbind()),
                    DEFAULT_COPIED => Default::Copied(value.clone().unbind()),
                    DEFAULT_FACTORY => Default::Factory(value.clone().unbind()),
                    _ => Default::Required,
                };
                (PyString::intern(py, name).unbind(), default)
            })
            .collect();
        messages[index].construct = Some(Construct {
            class: class.cast_into::<PyType>()?.unbind(),
            fast,
            same_order,
            extra_allow,
            defaults,
        });
    }
    Ok(())
}

impl NativeCodec {
    // ------------------------------------------------------------------ decode

    fn decode_message<'py>(
        &self,
        py: Python<'py>,
        index: usize,
        data: &[u8],
        depth: u32,
        construct: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        if depth > MAX_DEPTH {
            return Err(decode_error("message nesting too deep"));
        }
        let plan = &self.plan.messages[index];
        let message = &self.messages[index];
        let fields = &message.fields;
        let mut slots: Vec<Slot<'py>> = (0..plan.fields.len()).map(|_| Slot::Empty).collect();
        // Payloads of singular message fields, decoded once the message is read: a field
        // met several times is the merge of all, which is the decoding of their
        // concatenation (decoding at each occurrence would be quadratic).
        let mut pending: Pending<'_> = Vec::new();

        let mut pos = 0;
        while pos < data.len() {
            let (number, wire_type) = read_tag(data, &mut pos).map_err(decode_error)?;
            let Some(i) = plan.field_index(number) else {
                skip_field(data, &mut pos, wire_type).map_err(decode_error)?;
                continue;
            };
            let field = &plan.fields[i];
            let py_field = &fields[i];
            match field.ty {
                FieldType::Single(Value::Message(_) | Value::WellKnown(_))
                    if wire_type == WIRE_LEN =>
                {
                    let payload = read_len(data, &mut pos).map_err(decode_error)?;
                    match pending.iter_mut().find(|(f, _, _)| *f == i) {
                        Some((_, _, later)) => later.push(payload),
                        None => pending.push((i, payload, Vec::new())),
                    }
                    if let Some(group) = field.oneof {
                        clear_oneof(&plan.oneofs[group].fields, i, &mut slots, &mut pending);
                    }
                }
                FieldType::Single(value) => {
                    slots[i] = Slot::Value(self.read_value(
                        py, value, wire_type, data, &mut pos, depth, py_field, construct,
                    )?);
                    if let Some(group) = field.oneof {
                        clear_oneof(&plan.oneofs[group].fields, i, &mut slots, &mut pending);
                    }
                }
                FieldType::Repeated { value, .. } => {
                    if !matches!(slots[i], Slot::List(_)) {
                        slots[i] = Slot::List(PyList::empty(py));
                    }
                    let Slot::List(list) = &slots[i] else {
                        unreachable!()
                    };
                    match value {
                        Value::Scalar(scalar) if wire_type == WIRE_LEN && scalar.packable() => {
                            let packed = read_len(data, &mut pos).map_err(decode_error)?;
                            let mut p = 0;
                            while p < packed.len() {
                                list.append(self.read_scalar(
                                    py, scalar, packed, &mut p, py_field, construct,
                                )?)?;
                            }
                        }
                        _ => list.append(self.read_value(
                            py, value, wire_type, data, &mut pos, depth, py_field, construct,
                        )?)?,
                    }
                }
                FieldType::Map { key, value } => {
                    if wire_type != WIRE_LEN {
                        return Err(decode_error(format!(
                            "map field {} is not length-delimited",
                            field.name
                        )));
                    }
                    let entry = read_len(data, &mut pos).map_err(decode_error)?;
                    let (k, v) =
                        self.read_map_entry(py, key, value, entry, depth, py_field, construct)?;
                    if !matches!(slots[i], Slot::Map(_)) {
                        slots[i] = Slot::Map(PyDict::new(py));
                    }
                    let Slot::Map(map) = &slots[i] else {
                        unreachable!()
                    };
                    map.set_item(k, v)?;
                }
            }
        }

        for (i, first, later) in pending {
            let FieldType::Single(value) = plan.fields[i].ty else {
                unreachable!("only singular message fields are pending")
            };
            let decoded = if later.is_empty() {
                self.read_payload(py, value, first, depth, construct)?
            } else {
                let mut merged = first.to_vec();
                later
                    .iter()
                    .for_each(|payload| merged.extend_from_slice(payload));
                self.read_payload(py, value, &merged, depth, construct)?
            };
            slots[i] = Slot::Value(decoded);
        }

        let binding = message.construct.as_ref().filter(|_| construct);
        if let Some(binding) = binding
            && binding.fast
            && binding.same_order
        {
            return self.build_in_order(py, binding, &plan.fields, fields, slots);
        }

        // Field-number order; absent fields without presence get their zero value.
        let dict = PyDict::new(py);
        for (i, slot) in slots.into_iter().enumerate() {
            if let Some(value) =
                self.present_value(py, &plan.fields[i], &fields[i], slot, construct)?
            {
                dict.set_item(fields[i].key.bind(py), value)?;
            }
        }
        match binding {
            Some(binding) => self.build_reordered(py, binding, fields, dict),
            None => Ok(dict.into_any()),
        }
    }

    /// The value to store for a decoded slot: what was read, or the zero value of a
    /// field without presence (empty list/map, `0`, `""`, enum zero member). `None`
    /// for an absent field with presence.
    fn present_value<'py>(
        &self,
        py: Python<'py>,
        field: &FieldPlan,
        py_field: &PyField,
        slot: Slot<'py>,
        construct: bool,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        Ok(match (slot, field.ty) {
            (Slot::Value(value), _) => Some(value),
            (Slot::List(list), _) => Some(list.into_any()),
            (Slot::Map(map), _) => Some(map.into_any()),
            (Slot::Empty, FieldType::Repeated { .. }) => Some(PyList::empty(py).into_any()),
            (Slot::Empty, FieldType::Map { .. }) => Some(PyDict::new(py).into_any()),
            (Slot::Empty, FieldType::Single(_)) => match &py_field.zero {
                Some(_) if construct && py_field.enum_class.is_some() => {
                    Some(self.enum_object(py, 0, py_field, true)?)
                }
                Some(zero) => Some(zero.bind(py).clone()),
                None => None,
            },
        })
    }

    fn default_value<'py>(
        &self,
        py: Python<'py>,
        default: &Default,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        Ok(match default {
            Default::Shared(value) => Some(value.bind(py).clone()),
            Default::Copied(value) => Some(self.cache.deepcopy.bind(py).call1((value.bind(py),))?),
            Default::Factory(factory) => Some(factory.bind(py).call0()?),
            Default::Required => None,
        })
    }

    /// Construct mode, model fields == message fields in the same order: build
    /// `__dict__` in one pass, model defaults included.
    fn build_in_order<'py>(
        &self,
        py: Python<'py>,
        binding: &Construct,
        plan_fields: &[FieldPlan],
        fields: &[PyField],
        slots: Vec<Slot<'py>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let values = PyDict::new(py);
        let mut present = Vec::with_capacity(slots.len());
        for (((slot, field), py_field), (key, default)) in slots
            .into_iter()
            .zip(plan_fields)
            .zip(fields)
            .zip(&binding.defaults)
        {
            let key = key.bind(py);
            if let Some(value) = self.present_value(py, field, py_field, slot, true)? {
                values.set_item(key, value)?;
                present.push(key.clone());
            } else if let Some(value) = self.default_value(py, default)? {
                values.set_item(key, value)?;
            }
        }
        let fields_set = PySet::new(py, present)?;
        self.new_instance(py, binding, values, fields_set)
    }

    /// Construct mode, fallback: rebuild the values in declaration order (the
    /// serializer follows `__dict__` order), dropping fields the model does not
    /// declare, or delegate to `model_construct` for models that need it.
    fn build_reordered<'py>(
        &self,
        py: Python<'py>,
        binding: &Construct,
        fields: &[PyField],
        dict: Bound<'py, PyDict>,
    ) -> PyResult<Bound<'py, PyAny>> {
        if !binding.fast {
            for field in fields.iter().filter(|f| !f.in_model) {
                let key = field.key.bind(py);
                if dict.contains(key)? {
                    dict.del_item(key)?;
                }
            }
            return binding.class.bind(py).call_method(
                intern!(py, "model_construct"),
                (),
                Some(&dict),
            );
        }
        let values = PyDict::new(py);
        let fields_set = PySet::empty(py)?;
        for (key, default) in &binding.defaults {
            let key = key.bind(py);
            if let Some(value) = dict.get_item(key)? {
                values.set_item(key, value)?;
                fields_set.add(key)?;
            } else if let Some(value) = self.default_value(py, default)? {
                values.set_item(key, value)?;
            }
        }
        self.new_instance(py, binding, values, fields_set)
    }

    /// Allocate a model instance and set its pydantic slots through the CPython API,
    /// exactly what `BaseModel.model_construct` does with `object.__setattr__`.
    fn new_instance<'py>(
        &self,
        py: Python<'py>,
        binding: &Construct,
        values: Bound<'py, PyDict>,
        fields_set: Bound<'py, PySet>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let class = binding.class.bind(py);
        let extra = if binding.extra_allow {
            PyDict::new(py).into_any()
        } else {
            py.None().into_bound(py)
        };
        let none = py.None().into_bound(py);
        let attributes = [
            (intern!(py, "__dict__"), values.as_any()),
            (intern!(py, "__pydantic_fields_set__"), fields_set.as_any()),
            (intern!(py, "__pydantic_extra__"), &extra),
            (intern!(py, "__pydantic_private__"), &none),
        ];
        // SAFETY: `class` is a type object (checked when bound). `tp_alloc` returns a
        // new reference or NULL with an exception set, as `object.__new__` does for
        // a class without a custom `__new__` (Pydantic models). The generic setattr
        // goes through the same descriptors as `object.__setattr__`.
        unsafe {
            let ty = class.as_ptr().cast::<ffi::PyTypeObject>();
            let alloc = (*ty).tp_alloc.unwrap_or(ffi::PyType_GenericAlloc);
            let instance = Bound::from_owned_ptr_or_err(py, alloc(ty, 0))?;
            for (name, value) in attributes {
                if ffi::PyObject_GenericSetAttr(instance.as_ptr(), name.as_ptr(), value.as_ptr())
                    != 0
                {
                    return Err(PyErr::fetch(py));
                }
            }
            Ok(instance)
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn read_value<'py>(
        &self,
        py: Python<'py>,
        value: Value,
        wire_type: u8,
        data: &[u8],
        pos: &mut usize,
        depth: u32,
        field: &PyField,
        construct: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        match value {
            Value::Scalar(scalar) => {
                if wire_type != scalar.wire_type() {
                    return Err(decode_error(format!(
                        "wire type {wire_type} does not match {scalar:?}"
                    )));
                }
                self.read_scalar(py, scalar, data, pos, field, construct)
            }
            Value::Message(_) | Value::WellKnown(_) => {
                if wire_type != WIRE_LEN {
                    return Err(decode_error("message field is not length-delimited"));
                }
                let payload = read_len(data, pos).map_err(decode_error)?;
                self.read_payload(py, value, payload, depth, construct)
            }
        }
    }

    /// Decode the payload of a message or well-known type field.
    fn read_payload<'py>(
        &self,
        py: Python<'py>,
        value: Value,
        payload: &[u8],
        depth: u32,
        construct: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        match value {
            Value::Message(index) => self.decode_message(py, index, payload, depth + 1, construct),
            Value::WellKnown(wkt) => self.read_wkt(py, wkt, payload, depth + 1),
            Value::Scalar(_) => unreachable!("scalars have no payload"),
        }
    }

    fn read_scalar<'py>(
        &self,
        py: Python<'py>,
        scalar: Scalar,
        data: &[u8],
        pos: &mut usize,
        field: &PyField,
        construct: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        let Scalar::Enum(index) = scalar else {
            return read_plain_scalar(py, scalar, data, pos);
        };
        let number = read_varint(data, pos).map_err(decode_error)? as i32;
        if self.strict_enums {
            let plan = &self.plan.enums[index as usize];
            if !plan.contains(number) {
                return Err(decode_error(format!(
                    "unknown value {number} for enum {}",
                    plan.full_name
                )));
            }
        }
        self.enum_object(py, number, field, construct)
    }

    /// The Python value of an enum number: the enum member in construct mode when the
    /// field is bound to an enum class, the int otherwise. An unknown number stays an
    /// int unless the class creates a member for it (see `OpenIntEnum`).
    fn enum_object<'py>(
        &self,
        py: Python<'py>,
        number: i32,
        field: &PyField,
        construct: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        let number = number.into_pyobject(py)?.into_any();
        if construct && let Some(enum_class) = &field.enum_class {
            if let Some(member) = enum_class.members.bind(py).get_item(&number)? {
                return Ok(member);
            }
            match enum_class.class.bind(py).call1((&number,)) {
                Ok(member) => return Ok(member),
                Err(err) if err.is_instance_of::<PyValueError>(py) => {}
                Err(err) => return Err(err),
            }
        }
        Ok(number)
    }

    #[allow(clippy::too_many_arguments)]
    fn read_map_entry<'py>(
        &self,
        py: Python<'py>,
        key: Scalar,
        value: Value,
        entry: &[u8],
        depth: u32,
        field: &PyField,
        construct: bool,
    ) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
        let mut k = None;
        let mut v = None;
        let mut pos = 0;
        while pos < entry.len() {
            let (number, wire_type) = read_tag(entry, &mut pos).map_err(decode_error)?;
            match number {
                1 => {
                    k = Some(self.read_value(
                        py,
                        Value::Scalar(key),
                        wire_type,
                        entry,
                        &mut pos,
                        depth,
                        field,
                        construct,
                    )?)
                }
                2 => {
                    v = Some(self.read_value(
                        py, value, wire_type, entry, &mut pos, depth, field, construct,
                    )?)
                }
                _ => skip_field(entry, &mut pos, wire_type).map_err(decode_error)?,
            }
        }
        let k = k.unwrap_or_else(|| scalar_zero(py, key));
        let v = match (v, value) {
            (Some(v), _) => v,
            (None, Value::Scalar(Scalar::Enum(_))) => self.enum_object(py, 0, field, construct)?,
            (None, Value::Scalar(scalar)) => scalar_zero(py, scalar),
            (None, Value::Message(index)) => {
                self.decode_message(py, index, &[], depth + 1, construct)?
            }
            (None, Value::WellKnown(wkt)) => self.read_wkt(py, wkt, &[], depth + 1)?,
        };
        Ok((k, v))
    }

    fn read_wkt<'py>(
        &self,
        py: Python<'py>,
        wkt: WellKnown,
        payload: &[u8],
        depth: u32,
    ) -> PyResult<Bound<'py, PyAny>> {
        if depth > MAX_DEPTH {
            return Err(decode_error("message nesting too deep"));
        }
        match wkt {
            WellKnown::Timestamp => {
                let (seconds, nanos) = read_seconds_nanos(payload)?;
                wkt::check_timestamp(seconds, nanos).map_err(decode_error)?;
                let c = wkt::timestamp_to_civil(seconds);
                let utc = self.cache.utc.bind(py);
                Ok(PyDateTime::new(
                    py,
                    c.year,
                    c.month,
                    c.day,
                    c.hour,
                    c.minute,
                    c.second,
                    (nanos / 1000) as u32,
                    Some(utc),
                )?
                .into_any())
            }
            WellKnown::Duration => {
                let (seconds, nanos) = read_seconds_nanos(payload)?;
                wkt::check_duration(seconds, nanos).map_err(decode_error)?;
                let (days, secs, micros) = wkt::duration_to_parts(seconds, nanos);
                Ok(PyDelta::new(py, days as i32, secs as i32, micros as i32, false)?.into_any())
            }
            WellKnown::Wrapper(scalar) => {
                let mut value = None;
                let mut pos = 0;
                while pos < payload.len() {
                    let (number, wire_type) = read_tag(payload, &mut pos).map_err(decode_error)?;
                    if number == 1 && wire_type == scalar.wire_type() {
                        value = Some(read_plain_scalar(py, scalar, payload, &mut pos)?);
                    } else {
                        skip_field(payload, &mut pos, wire_type).map_err(decode_error)?;
                    }
                }
                Ok(value.unwrap_or_else(|| scalar_zero(py, scalar)))
            }
            WellKnown::FieldMask => {
                let paths = PyList::empty(py);
                let mut pos = 0;
                while pos < payload.len() {
                    let (number, wire_type) = read_tag(payload, &mut pos).map_err(decode_error)?;
                    if number == 1 && wire_type == WIRE_LEN {
                        paths.append(read_plain_scalar(py, Scalar::String, payload, &mut pos)?)?;
                    } else {
                        skip_field(payload, &mut pos, wire_type).map_err(decode_error)?;
                    }
                }
                Ok(paths.into_any())
            }
            WellKnown::Struct => self.read_struct(py, payload, depth),
            WellKnown::Value => self.read_json_value(py, payload, depth),
            WellKnown::ListValue => self.read_list_value(py, payload, depth),
            WellKnown::Date => {
                let (mut year, mut month, mut day) = (0i32, 0i32, 0i32);
                let mut pos = 0;
                while pos < payload.len() {
                    let (number, wire_type) = read_tag(payload, &mut pos).map_err(decode_error)?;
                    let target = match (number, wire_type) {
                        (1, WIRE_VARINT) => &mut year,
                        (2, WIRE_VARINT) => &mut month,
                        (3, WIRE_VARINT) => &mut day,
                        _ => {
                            skip_field(payload, &mut pos, wire_type).map_err(decode_error)?;
                            continue;
                        }
                    };
                    *target = read_varint(payload, &mut pos).map_err(decode_error)? as i32;
                }
                let (Ok(month), Ok(day)) = (u8::try_from(month), u8::try_from(day)) else {
                    return Err(decode_error(format!("invalid date {year}-{month}-{day}")));
                };
                PyDate::new(py, year, month, day)
                    .map(Bound::into_any)
                    .map_err(|_| {
                        // A partial date (year 0, month 0 or day 0) has no Python value.
                        decode_error(format!("invalid or partial date {year}-{month}-{day}"))
                    })
            }
        }
    }

    /// `google.protobuf.Struct` -> `dict[str, JSON value]`.
    fn read_struct<'py>(
        &self,
        py: Python<'py>,
        payload: &[u8],
        depth: u32,
    ) -> PyResult<Bound<'py, PyAny>> {
        let dict = PyDict::new(py);
        let mut pos = 0;
        while pos < payload.len() {
            let (number, wire_type) = read_tag(payload, &mut pos).map_err(decode_error)?;
            if number != 1 || wire_type != WIRE_LEN {
                skip_field(payload, &mut pos, wire_type).map_err(decode_error)?;
                continue;
            }
            let entry = read_len(payload, &mut pos).map_err(decode_error)?;
            let (mut key, mut value) = (None, None);
            let mut p = 0;
            while p < entry.len() {
                let (number, wire_type) = read_tag(entry, &mut p).map_err(decode_error)?;
                match (number, wire_type) {
                    (1, WIRE_LEN) => {
                        key = Some(read_plain_scalar(py, Scalar::String, entry, &mut p)?)
                    }
                    (2, WIRE_LEN) => {
                        let inner = read_len(entry, &mut p).map_err(decode_error)?;
                        value = Some(self.read_json_value(py, inner, depth + 1)?);
                    }
                    _ => skip_field(entry, &mut p, wire_type).map_err(decode_error)?,
                }
            }
            dict.set_item(
                key.unwrap_or_else(|| PyString::new(py, "").into_any()),
                value.unwrap_or_else(|| py.None().into_bound(py)),
            )?;
        }
        Ok(dict.into_any())
    }

    /// `google.protobuf.Value` -> `None | bool | float | str | dict | list`.
    fn read_json_value<'py>(
        &self,
        py: Python<'py>,
        payload: &[u8],
        depth: u32,
    ) -> PyResult<Bound<'py, PyAny>> {
        if depth > MAX_DEPTH {
            return Err(decode_error("message nesting too deep"));
        }
        let mut value = py.None().into_bound(py);
        let mut pos = 0;
        while pos < payload.len() {
            let (number, wire_type) = read_tag(payload, &mut pos).map_err(decode_error)?;
            value = match (number, wire_type) {
                (1, WIRE_VARINT) => {
                    read_varint(payload, &mut pos).map_err(decode_error)?;
                    py.None().into_bound(py)
                }
                (2, WIRE_I64) => read_plain_scalar(py, Scalar::Double, payload, &mut pos)?,
                (3, WIRE_LEN) => read_plain_scalar(py, Scalar::String, payload, &mut pos)?,
                (4, WIRE_VARINT) => read_plain_scalar(py, Scalar::Bool, payload, &mut pos)?,
                (5, WIRE_LEN) => {
                    let inner = read_len(payload, &mut pos).map_err(decode_error)?;
                    self.read_struct(py, inner, depth + 1)?
                }
                (6, WIRE_LEN) => {
                    let inner = read_len(payload, &mut pos).map_err(decode_error)?;
                    self.read_list_value(py, inner, depth + 1)?
                }
                _ => {
                    skip_field(payload, &mut pos, wire_type).map_err(decode_error)?;
                    continue;
                }
            };
        }
        Ok(value)
    }

    /// `google.protobuf.ListValue` -> `list[JSON value]`.
    fn read_list_value<'py>(
        &self,
        py: Python<'py>,
        payload: &[u8],
        depth: u32,
    ) -> PyResult<Bound<'py, PyAny>> {
        let list = PyList::empty(py);
        let mut pos = 0;
        while pos < payload.len() {
            let (number, wire_type) = read_tag(payload, &mut pos).map_err(decode_error)?;
            if number == 1 && wire_type == WIRE_LEN {
                let inner = read_len(payload, &mut pos).map_err(decode_error)?;
                list.append(self.read_json_value(py, inner, depth + 1)?)?;
            } else {
                skip_field(payload, &mut pos, wire_type).map_err(decode_error)?;
            }
        }
        Ok(list.into_any())
    }

    // ------------------------------------------------------------------ encode

    fn encode_message(
        &self,
        py: Python<'_>,
        index: usize,
        obj: &Bound<'_, PyAny>,
        buf: &mut Vec<u8>,
        depth: u32,
    ) -> PyResult<()> {
        if depth > MAX_DEPTH {
            return Err(PyValueError::new_err(
                "protobuf encode error: message nesting too deep",
            ));
        }
        let plan = &self.plan.messages[index];
        let fields = &self.messages[index].fields;
        // Pydantic v2 keeps field values in the instance `__dict__`.
        let values = match obj.cast::<PyDict>() {
            Ok(dict) => dict.clone(),
            Err(_) => obj
                .getattr(intern!(py, "__dict__"))
                .map_err(|_| {
                    PyTypeError::new_err(format!(
                        "expected a dict or a model for {}, got {}",
                        plan.full_name,
                        type_name(obj)
                    ))
                })?
                .cast_into::<PyDict>()?,
        };
        // The member set in each oneof, to refuse a second one.
        let mut oneof_set: Vec<Option<usize>> = vec![None; plan.oneofs.len()];
        for (i, field) in plan.fields.iter().enumerate() {
            let Some(value) = values.get_item(fields[i].key.bind(py))? else {
                continue;
            };
            if value.is_none() {
                continue;
            }
            if let Some(group) = field.oneof {
                if let Some(other) = oneof_set[group] {
                    return Err(PyValueError::new_err(format!(
                        "{}: oneof {} has several fields set: {}, {}",
                        plan.full_name,
                        plan.oneofs[group].name,
                        fields[other].key.bind(py),
                        fields[i].key.bind(py),
                    )));
                }
                oneof_set[group] = Some(i);
            }
            self.encode_field(py, field, &value, buf, depth)
                .map_err(|err| field_error(py, &plan.full_name, field, err))?;
        }
        Ok(())
    }

    fn encode_field(
        &self,
        py: Python<'_>,
        field: &FieldPlan,
        value: &Bound<'_, PyAny>,
        buf: &mut Vec<u8>,
        depth: u32,
    ) -> PyResult<()> {
        let number = field.number;
        match field.ty {
            FieldType::Single(Value::Scalar(scalar)) => {
                let encoded = self.encode_scalar(scalar, value)?;
                if field.explicit_presence || !encoded.is_zero() {
                    write_tag(buf, number, scalar.wire_type());
                    encoded.write(buf);
                }
            }
            FieldType::Single(Value::Message(index)) => {
                write_tag(buf, number, WIRE_LEN);
                write_len_delimited(buf, |b| self.encode_message(py, index, value, b, depth + 1))?;
            }
            FieldType::Single(Value::WellKnown(wkt)) => {
                write_tag(buf, number, WIRE_LEN);
                write_len_delimited(buf, |b| self.encode_wkt(py, wkt, value, b, depth + 1))?;
            }
            FieldType::Repeated {
                value: Value::Scalar(scalar),
                packed,
            } => {
                if packed && scalar.packable() {
                    if value.len()? == 0 {
                        return Ok(());
                    }
                    write_tag(buf, number, WIRE_LEN);
                    write_len_delimited(buf, |b| -> PyResult<()> {
                        for item in value.try_iter()? {
                            self.encode_scalar(scalar, &item?)?.write(b);
                        }
                        Ok(())
                    })?;
                } else {
                    for item in value.try_iter()? {
                        let item = item?;
                        write_tag(buf, number, scalar.wire_type());
                        self.encode_scalar(scalar, &item)?.write(buf);
                    }
                }
            }
            FieldType::Repeated {
                value: Value::Message(index),
                ..
            } => {
                for item in value.try_iter()? {
                    let item = item?;
                    write_tag(buf, number, WIRE_LEN);
                    write_len_delimited(buf, |b| {
                        self.encode_message(py, index, &item, b, depth + 1)
                    })?;
                }
            }
            FieldType::Repeated {
                value: Value::WellKnown(wkt),
                ..
            } => {
                for item in value.try_iter()? {
                    let item = item?;
                    write_tag(buf, number, WIRE_LEN);
                    write_len_delimited(buf, |b| self.encode_wkt(py, wkt, &item, b, depth + 1))?;
                }
            }
            FieldType::Map {
                key,
                value: map_value,
            } => {
                for (k, v) in value.cast::<PyDict>()?.iter() {
                    write_tag(buf, number, WIRE_LEN);
                    write_len_delimited(buf, |b| -> PyResult<()> {
                        write_tag(b, 1, key.wire_type());
                        self.encode_scalar(key, &k)?.write(b);
                        match map_value {
                            Value::Scalar(scalar) => {
                                write_tag(b, 2, scalar.wire_type());
                                self.encode_scalar(scalar, &v)?.write(b);
                            }
                            Value::Message(index) => {
                                write_tag(b, 2, WIRE_LEN);
                                write_len_delimited(b, |bb| {
                                    self.encode_message(py, index, &v, bb, depth + 1)
                                })?;
                            }
                            Value::WellKnown(wkt) => {
                                write_tag(b, 2, WIRE_LEN);
                                write_len_delimited(b, |bb| {
                                    self.encode_wkt(py, wkt, &v, bb, depth + 1)
                                })?;
                            }
                        }
                        Ok(())
                    })?;
                }
            }
        }
        Ok(())
    }

    fn encode_scalar<'a>(
        &self,
        scalar: Scalar,
        value: &'a Bound<'_, PyAny>,
    ) -> PyResult<Encoded<'a>> {
        Ok(match scalar {
            Scalar::Int32 => Encoded::Varint(value.extract::<i32>()? as i64 as u64),
            Scalar::Enum(index) => Encoded::Varint(self.enum_number(index, value)? as i64 as u64),
            Scalar::Int64 => Encoded::Varint(value.extract::<i64>()? as u64),
            Scalar::Uint32 => Encoded::Varint(u64::from(value.extract::<u32>()?)),
            Scalar::Uint64 => Encoded::Varint(value.extract::<u64>()?),
            Scalar::Sint32 => Encoded::Varint(zigzag32(value.extract::<i32>()?)),
            Scalar::Sint64 => Encoded::Varint(zigzag64(value.extract::<i64>()?)),
            Scalar::Bool => Encoded::Varint(u64::from(value.extract::<bool>()?)),
            Scalar::Fixed32 => Encoded::I32(value.extract::<u32>()?),
            Scalar::Sfixed32 => Encoded::I32(value.extract::<i32>()? as u32),
            Scalar::Float => Encoded::I32((value.extract::<f64>()? as f32).to_bits()),
            Scalar::Fixed64 => Encoded::I64(value.extract::<u64>()?),
            Scalar::Sfixed64 => Encoded::I64(value.extract::<i64>()? as u64),
            Scalar::Double => Encoded::I64(value.extract::<f64>()?.to_bits()),
            Scalar::String => Encoded::Len(value.cast::<PyString>()?.to_str()?.as_bytes()),
            Scalar::Bytes => Encoded::Len(value.cast::<PyBytes>()?.as_bytes()),
        })
    }

    /// The number of an enum value given as an int, an `IntEnum`, an `Enum` whose
    /// value is an int, or a declared proto name (as a str, an `Enum` value or an
    /// `Enum` member name). With `unknown_enum="error"`, an undeclared number is refused,
    /// as on decoding.
    fn enum_number(&self, index: u32, value: &Bound<'_, PyAny>) -> PyResult<i32> {
        let plan = &self.plan.enums[index as usize];
        let number = Self::any_enum_number(plan, value)?;
        if self.strict_enums && !plan.contains(number) {
            return Err(PyValueError::new_err(format!(
                "unknown value {number} for enum {}",
                plan.full_name
            )));
        }
        Ok(number)
    }

    fn any_enum_number(plan: &EnumPlan, value: &Bound<'_, PyAny>) -> PyResult<i32> {
        if let Ok(number) = value.extract::<i32>() {
            return Ok(number);
        }
        let py = value.py();
        let mut candidates = vec![value.clone()];
        if let Ok(inner) = value.getattr(intern!(py, "value")) {
            if let Ok(number) = inner.extract::<i32>() {
                return Ok(number);
            }
            candidates.push(inner);
        }
        if let Ok(name) = value.getattr(intern!(py, "name")) {
            candidates.push(name);
        }
        for candidate in &candidates {
            if let Ok(text) = candidate.cast::<PyString>()
                && let Some(number) = plan.number_of(text.to_str()?)
            {
                return Ok(number);
            }
        }
        Err(PyValueError::new_err(format!(
            "{} is not a value of enum {}",
            value.repr()?,
            plan.full_name
        )))
    }

    fn encode_wkt(
        &self,
        py: Python<'_>,
        wkt: WellKnown,
        value: &Bound<'_, PyAny>,
        buf: &mut Vec<u8>,
        depth: u32,
    ) -> PyResult<()> {
        if depth > MAX_DEPTH {
            return Err(PyValueError::new_err(
                "protobuf encode error: message nesting too deep",
            ));
        }
        match wkt {
            WellKnown::Timestamp => {
                let (seconds, nanos) = self.timestamp_of(py, value)?;
                write_seconds_nanos(buf, seconds, nanos);
            }
            WellKnown::Duration => {
                let delta = value.cast::<PyDelta>().map_err(|_| {
                    PyTypeError::new_err(format!(
                        "expected datetime.timedelta, got {}",
                        type_name(value)
                    ))
                })?;
                let (seconds, nanos) = wkt::duration_from_parts(
                    delta.get_days().into(),
                    delta.get_seconds().into(),
                    delta.get_microseconds().into(),
                );
                wkt::check_duration(seconds, nanos).map_err(PyValueError::new_err)?;
                write_seconds_nanos(buf, seconds, nanos);
            }
            WellKnown::Wrapper(scalar) => {
                let encoded = self.encode_scalar(scalar, value)?;
                if !encoded.is_zero() {
                    write_tag(buf, 1, scalar.wire_type());
                    encoded.write(buf);
                }
            }
            WellKnown::FieldMask => {
                for path in value.try_iter()? {
                    let path = path?;
                    write_tag(buf, 1, WIRE_LEN);
                    Encoded::Len(path.cast::<PyString>()?.to_str()?.as_bytes()).write(buf);
                }
            }
            WellKnown::Struct => self.encode_struct(py, value, buf, depth)?,
            WellKnown::Value => self.encode_json_value(py, value, buf, depth)?,
            WellKnown::ListValue => self.encode_list_value(py, value, buf, depth)?,
            WellKnown::Date => {
                let parts = match value.cast::<PyDate>() {
                    // A datetime is a date too: refuse it rather than drop its time.
                    Ok(date) if value.cast::<PyDateTime>().is_err() => [
                        date.get_year(),
                        i32::from(date.get_month()),
                        i32::from(date.get_day()),
                    ],
                    // A `{"year", "month", "day"}` dict, as protoruf 0.4 took it.
                    _ if value.cast::<PyDict>().is_ok() => {
                        let dict = value.cast::<PyDict>()?;
                        let part = |key: &str| -> PyResult<i32> {
                            dict.get_item(key)?.map_or(Ok(0), |v| v.extract::<i32>())
                        };
                        [part("year")?, part("month")?, part("day")?]
                    }
                    _ => {
                        return Err(PyTypeError::new_err(format!(
                            "expected datetime.date, got {}",
                            type_name(value)
                        )));
                    }
                };
                for (number, part) in (1..).zip(parts) {
                    // Proto3: zero parts (of a partial date) are not written.
                    if part != 0 {
                        write_tag(buf, number, WIRE_VARINT);
                        write_varint(buf, part as i64 as u64);
                    }
                }
            }
        }
        Ok(())
    }

    /// `(seconds, nanos)` of an aware `datetime` (or a naive one with
    /// `naive_datetime="utc"`).
    fn timestamp_of(&self, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<(i64, i32)> {
        let dt = value.cast::<PyDateTime>().map_err(|_| {
            PyTypeError::new_err(format!(
                "expected datetime.datetime, got {}",
                type_name(value)
            ))
        })?;
        let offset = match dt.get_tzinfo() {
            Some(tz) if tz.is(self.cache.utc.bind(py)) => Some(0),
            Some(_) => {
                let offset = dt.call_method0(intern!(py, "utcoffset"))?;
                if offset.is_none() {
                    None
                } else {
                    let delta = offset.cast::<PyDelta>()?;
                    Some(
                        i64::from(delta.get_days()) * 86_400_000_000
                            + i64::from(delta.get_seconds()) * 1_000_000
                            + i64::from(delta.get_microseconds()),
                    )
                }
            }
            None => None,
        };
        let offset = match offset {
            Some(offset) => offset,
            None if self.naive_utc => 0,
            None => {
                return Err(PyValueError::new_err(
                    "naive datetime: attach a timezone, or create the codec with \
                     naive_datetime='utc'",
                ));
            }
        };
        let civil = wkt::Civil {
            year: dt.get_year(),
            month: dt.get_month(),
            day: dt.get_day(),
            hour: dt.get_hour(),
            minute: dt.get_minute(),
            second: dt.get_second(),
        };
        let (seconds, nanos) = wkt::civil_to_timestamp(civil, dt.get_microsecond(), offset);
        wkt::check_timestamp(seconds, nanos).map_err(PyValueError::new_err)?;
        Ok((seconds, nanos))
    }

    fn encode_struct(
        &self,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
        buf: &mut Vec<u8>,
        depth: u32,
    ) -> PyResult<()> {
        let dict = value.cast::<PyDict>().map_err(|_| {
            PyTypeError::new_err(format!(
                "expected a dict for google.protobuf.Struct, got {}",
                type_name(value)
            ))
        })?;
        for (key, item) in dict.iter() {
            let key = key
                .cast::<PyString>()
                .map_err(|_| PyTypeError::new_err("google.protobuf.Struct keys must be str"))?;
            write_tag(buf, 1, WIRE_LEN);
            write_len_delimited(buf, |b| -> PyResult<()> {
                write_tag(b, 1, WIRE_LEN);
                Encoded::Len(key.to_str()?.as_bytes()).write(b);
                write_tag(b, 2, WIRE_LEN);
                write_len_delimited(b, |bb| self.encode_json_value(py, &item, bb, depth + 1))
            })?;
        }
        Ok(())
    }

    fn encode_json_value(
        &self,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
        buf: &mut Vec<u8>,
        depth: u32,
    ) -> PyResult<()> {
        if depth > MAX_DEPTH {
            return Err(PyValueError::new_err(
                "protobuf encode error: message nesting too deep",
            ));
        }
        if value.is_none() {
            write_tag(buf, 1, WIRE_VARINT);
            write_varint(buf, 0);
        } else if value.is_instance_of::<PyBool>() {
            write_tag(buf, 4, WIRE_VARINT);
            write_varint(buf, u64::from(value.is_truthy()?));
        } else if value.is_instance_of::<PyInt>() || value.is_instance_of::<PyFloat>() {
            write_tag(buf, 2, WIRE_I64);
            Encoded::I64(value.extract::<f64>()?.to_bits()).write(buf);
        } else if let Ok(text) = value.cast::<PyString>() {
            write_tag(buf, 3, WIRE_LEN);
            Encoded::Len(text.to_str()?.as_bytes()).write(buf);
        } else if value.is_instance_of::<PyDict>() {
            write_tag(buf, 5, WIRE_LEN);
            write_len_delimited(buf, |b| self.encode_struct(py, value, b, depth + 1))?;
        } else if value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>() {
            write_tag(buf, 6, WIRE_LEN);
            write_len_delimited(buf, |b| self.encode_list_value(py, value, b, depth + 1))?;
        } else {
            return Err(PyTypeError::new_err(format!(
                "{} cannot be encoded as google.protobuf.Value",
                type_name(value)
            )));
        }
        Ok(())
    }

    fn encode_list_value(
        &self,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
        buf: &mut Vec<u8>,
        depth: u32,
    ) -> PyResult<()> {
        for item in value.try_iter()? {
            let item = item?;
            write_tag(buf, 1, WIRE_LEN);
            write_len_delimited(buf, |b| self.encode_json_value(py, &item, b, depth + 1))?;
        }
        Ok(())
    }
}

/// Payloads of singular message fields still to decode: (field, first payload, later
/// payloads). The usual single occurrence needs no allocation.
type Pending<'a> = Vec<(usize, &'a [u8], Vec<&'a [u8]>)>;

/// A oneof member was just read: the other members of its group are unset.
fn clear_oneof(members: &[usize], set: usize, slots: &mut [Slot<'_>], pending: &mut Pending<'_>) {
    for &member in members.iter().filter(|&&m| m != set) {
        slots[member] = Slot::Empty;
    }
    pending.retain(|(field, _, _)| *field == set || !members.contains(field));
}

fn write_seconds_nanos(buf: &mut Vec<u8>, seconds: i64, nanos: i32) {
    if seconds != 0 {
        write_tag(buf, 1, WIRE_VARINT);
        write_varint(buf, seconds as u64);
    }
    if nanos != 0 {
        write_tag(buf, 2, WIRE_VARINT);
        write_varint(buf, i64::from(nanos) as u64);
    }
}
