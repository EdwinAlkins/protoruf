# API Rust

Documentation de l'implémentation Rust de `protoruf`.

## Vue d'ensemble

La bibliothèque Rust expose des fonctions Python via PyO3 et utilise:

- **protox** - Compilation de fichiers `.proto` sans dépendance à `protoc`
- **prost** - Support Protobuf
- **prost-reflect** - Réflexion et messages dynamiques
- **serde_json** - Sérialisation JSON

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Python (PyO3)                             │
│  compile_proto()  │  json_to_protobuf()  │  protobuf_to_json│
├─────────────────────────────────────────────────────────────┤
│                    Rust Core                                 │
│  protox::compile() │  DynamicMessage     │  serde_json      │
├─────────────────────────────────────────────────────────────┤
│                    Protobuf                                  │
│  DescriptorPool    │  MessageDescriptor  │  FieldDescriptor │
└─────────────────────────────────────────────────────────────┘
```

---

## Fonctions exposées

### `compile_proto`

```rust
#[pyfunction]
fn compile_proto(
    proto_path: &str,
    include_paths: Option<Vec<String>>,
) -> PyResult<Vec<u8>>
```

Compile un fichier `.proto` en descriptor set.

**Implémentation:**

```rust
use protox;
use std::path::PathBuf;

fn compile_proto(
    proto_path: &str,
    include_paths: Option<Vec<String>>,
) -> PyResult<Vec<u8>> {
    let proto_path = PathBuf::from(proto_path);
    
    // Déterminer les include paths
    let include_paths: Vec<String> = include_paths.unwrap_or_else(|| {
        proto_path.parent()
            .map(|p| vec![p.to_string_lossy().to_string()])
            .unwrap_or_default()
    });
    
    let include_paths_ref: Vec<&str> = include_paths.iter()
        .map(|s| s.as_str())
        .collect();
    
    // Compiler avec protox
    let file_descriptor_set = protox::compile(&[&proto_path], &include_paths_ref)
        .map_err(|e| PyErr::new::<PyRuntimeError, _>(
            format!("Failed to compile proto file: {}", e)
        ))?;
    
    // Sérialiser en bytes
    let mut descriptor_bytes = Vec::new();
    file_descriptor_set.encode(&mut descriptor_bytes)
        .map_err(|e| PyErr::new::<PyRuntimeError, _>(
            format!("Failed to encode descriptor set: {}", e)
        ))?;
    
    Ok(descriptor_bytes)
}
```

---

### `json_to_protobuf`

```rust
#[pyfunction]
fn json_to_protobuf<'py>(
    py: Python<'py>,
    json_str: &str,
    descriptor_bytes: &[u8],
    message_type: Option<&str>,
) -> PyResult<Bound<'py, PyBytes>>
```

Convertit JSON en message Protobuf.

**Implémentation:**

```rust
use prost_reflect::{DescriptorPool, DynamicMessage};
use serde_json::Value as JsonValue;

fn json_to_protobuf<'py>(
    py: Python<'py>,
    json_str: &str,
    descriptor_bytes: &[u8],
    message_type: Option<&str>,
) -> PyResult<Bound<'py, PyBytes>> {
    // Parser JSON
    let json_value: JsonValue = serde_json::from_str(json_str)
        .map_err(|e| PyErr::new::<PyValueError, _>(
            format!("Invalid JSON: {}", e)
        ))?;

    // Charger le descriptor pool
    let pool = DescriptorPool::decode(descriptor_bytes)
        .map_err(|e| PyErr::new::<PyValueError, _>(
            format!("Failed to load descriptor pool: {}", e)
        ))?;
    
    // Obtenir le descripteur du message
    let message_type_name = message_type.unwrap_or("message.Message");
    let message_descriptor = pool.get_message_by_name(message_type_name)
        .ok_or_else(|| PyErr::new::<PyValueError, _>(
            format!("Message type '{}' not found", message_type_name)
        ))?;

    // Convertir JSON → DynamicMessage
    let dynamic_message = json_to_dynamic_message(
        &message_descriptor, 
        &json_value
    )?;

    // Encoder en bytes
    let mut buf = Vec::new();
    dynamic_message.encode(&mut buf)
        .map_err(|e| PyErr::new::<PyRuntimeError, _>(
            format!("Encoding error: {}", e)
        ))?;

    Ok(PyBytes::new(py, &buf))
}
```

---

### `protobuf_to_json`

```rust
#[pyfunction]
fn protobuf_to_json(
    protobuf_bytes: &[u8],
    descriptor_bytes: &[u8],
    pretty: bool,
    message_type: Option<&str>,
) -> PyResult<String>
```

Convertit un message Protobuf en JSON.

**Implémentation:**

```rust
use prost_reflect::{DescriptorPool, DynamicMessage};

fn protobuf_to_json(
    protobuf_bytes: &[u8],
    descriptor_bytes: &[u8],
    pretty: bool,
    message_type: Option<&str>,
) -> PyResult<String> {
    // Charger le descriptor pool
    let pool = DescriptorPool::decode(descriptor_bytes)
        .map_err(|e| PyErr::new::<PyValueError, _>(
            format!("Failed to load descriptor pool: {}", e)
        ))?;
    
    // Obtenir le descripteur du message
    let message_type_name = message_type.unwrap_or("message.Message");
    let message_descriptor = pool.get_message_by_name(message_type_name)
        .ok_or_else(|| PyErr::new::<PyValueError, _>(
            format!("Message type '{}' not found", message_type_name)
        ))?;

    // Décoder les bytes → DynamicMessage
    let dynamic_message = DynamicMessage::decode(
        message_descriptor.clone(), 
        protobuf_bytes
    )
    .map_err(|e| PyErr::new::<PyRuntimeError, _>(
        format!("Decoding error: {}", e)
    ))?;

    // Convertir → JSON
    let json_value = dynamic_message_to_json(&dynamic_message);

    // Sérialiser
    let json_str = if pretty {
        serde_json::to_string_pretty(&json_value)
    } else {
        serde_json::to_string(&json_value)
    }
    .map_err(|e| PyErr::new::<PyRuntimeError, _>(
        format!("JSON serialization error: {}", e)
    ))?;

    Ok(json_str)
}
```

---

## Conversion JSON ↔ Protobuf

### `json_to_dynamic_message`

Convertit un JSON en `DynamicMessage`:

```rust
fn json_to_dynamic_message(
    descriptor: &MessageDescriptor,
    value: &JsonValue,
) -> PyResult<DynamicMessage> {
    let obj = value.as_object()
        .ok_or_else(|| PyErr::new::<PyTypeError, _>("Expected JSON object"))?;

    let mut message = DynamicMessage::new(descriptor.clone());

    for (key, val) in obj {
        let field = descriptor.get_field_by_name(key)
            .ok_or_else(|| PyErr::new::<PyValueError, _>(
                format!("Unknown field: {}", key)
            ))?;

        let field_value = json_to_prost_value(&field, val)?;
        message.set_field(&field, field_value);
    }

    Ok(message)
}
```

### `json_to_prost_value`

Convertit une valeur JSON en `prost_reflect::Value`:

```rust
fn json_to_prost_value(
    field: &FieldDescriptor,
    value: &JsonValue,
) -> PyResult<Value> {
    Ok(match value {
        JsonValue::Null => Value::default_value_for_field(field),
        JsonValue::Bool(b) => Value::Bool(*b),
        JsonValue::Number(n) => {
            // Gestion des types numériques
            match field.kind() {
                Kind::Int32 | Kind::Sint32 => {
                    Value::I32(n.as_i64().ok_or(...) as i32)
                }
                Kind::Int64 => Value::I64(n.as_i64().ok_or(...)?),
                Kind::Uint32 => Value::U32(n.as_u64().ok_or(...) as u32),
                Kind::Uint64 => Value::U64(n.as_u64().ok_or(...)?),
                Kind::Float => Value::F32(n.as_f64().ok_or(...) as f32),
                Kind::Double => Value::F64(n.as_f64().ok_or(...)?),
                _ => return Err(...),
            }
        }
        JsonValue::String(s) => {
            // Gestion des enums
            if let Kind::Enum(enum_desc) = field.kind() {
                if let Some(enum_value) = enum_desc.get_value_by_name(s) {
                    Value::EnumNumber(enum_value.number())
                } else {
                    return Err(...);
                }
            } else {
                Value::String(s.clone())
            }
        }
        JsonValue::Array(arr) => {
            Value::List(arr.iter()
                .map(|v| json_to_prost_value(field, v))
                .collect::<PyResult<_>>()?)
        }
        JsonValue::Object(obj) => {
            if let Kind::Message(msg_desc) = field.kind() {
                // Message imbriqué
                ...
            } else {
                return Err(...);
            }
        }
    })
}
```

### `dynamic_message_to_json`

Convertit un `DynamicMessage` en JSON:

```rust
fn dynamic_message_to_json(message: &DynamicMessage) -> JsonValue {
    let mut obj = serde_json::Map::new();
    
    for field in message.descriptor().fields() {
        if !message.has_field(&field) {
            continue;
        }
        
        let value = message.get_field(&field);
        obj.insert(field.name().to_string(), prost_value_to_json(&value));
    }
    
    JsonValue::Object(obj)
}
```

### `prost_value_to_json`

Convertit une valeur `prost_reflect::Value` en JSON:

```rust
fn prost_value_to_json(value: &Value) -> JsonValue {
    match value {
        Value::Bool(b) => JsonValue::Bool(*b),
        Value::I32(i) => JsonValue::Number((*i).into()),
        Value::I64(i) => JsonValue::Number((*i).into()),
        Value::U32(u) => JsonValue::Number((*u).into()),
        Value::U64(u) => JsonValue::Number((*u).into()),
        Value::F32(f) => serde_json::Number::from_f64((*f).into())
            .map(JsonValue::Number)
            .unwrap_or(JsonValue::Null),
        Value::F64(f) => serde_json::Number::from_f64(*f)
            .map(JsonValue::Number)
            .unwrap_or(JsonValue::Null),
        Value::String(s) => JsonValue::String(s.clone()),
        Value::EnumNumber(e) => JsonValue::Number((*e).into()),
        Value::Message(msg) => dynamic_message_to_json(msg),
        Value::List(list) => {
            JsonValue::Array(list.iter()
                .map(|v| prost_value_to_json(v))
                .collect())
        }
        Value::Map(map) => {
            let obj: serde_json::Map<_, _> = map.iter()
                .map(|(k, v)| {
                    let key = match k {
                        MapKey::String(s) => s.clone(),
                        MapKey::I32(i) => i.to_string(),
                        MapKey::U64(u) => u.to_string(),
                        // ...
                    };
                    (key, prost_value_to_json(v))
                })
                .collect();
            JsonValue::Object(obj)
        }
        _ => JsonValue::Null,
    }
}
```

---

## Module Python

```rust
use pyo3::prelude::*;

#[pymodule]
fn _protoruf(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(compile_proto, m)?)?;
    m.add_function(wrap_pyfunction!(json_to_protobuf, m)?)?;
    m.add_function(wrap_pyfunction!(protobuf_to_json, m)?)?;
    Ok(())
}
```

---

## Cargo.toml

```toml
[package]
name = "protoruf"
version = "0.1.0"
edition = "2021"

[lib]
name = "protoruf"
crate-type = ["cdylib"]

[dependencies]
pyo3 = { version = "0.23", features = ["extension-module"] }
prost = "0.13"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
prost-reflect = "0.14"
prost-build = "0.13"
protox = "0.7"
```

---

## Extension

### Ajouter un nouveau type de conversion

1. **Ajouter la fonction Rust:**

```rust
#[pyfunction]
fn convert_custom_format(
    input: &str,
    descriptor_bytes: &[u8],
) -> PyResult<String> {
    // Implémentation
}
```

2. **Exposer dans le module:**

```rust
#[pymodule]
fn _protoruf(m: &Bound<'_, PyModule>) -> PyResult<()> {
    // ...
    m.add_function(wrap_pyfunction!(convert_custom_format, m)?)?;
}
```

3. **Ajouter le wrapper Python:**

```python
# python/protoruf/__init__.py

def convert_custom_format(input: str, descriptor_bytes: bytes) -> str:
    return _protoruf.convert_custom_format(input, descriptor_bytes)
```

---

## Tests Rust

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compile_proto() {
        // Test de compilation
    }

    #[test]
    fn test_json_to_protobuf_roundtrip() {
        // Test de round-trip
    }
}
```

Lancer les tests:

```bash
cargo test
```

---

## Bonnes pratiques

### 1. Gestion des erreurs

Toujours convertir les erreurs Rust en exceptions Python appropriées:

```rust
.map_err(|e| PyErr::new::<PyValueError, _>(format!("Error: {}", e)))
```

### 2. Lifetimes PyO3

Respecter les lifetimes pour les objets Python:

```rust
fn json_to_protobuf<'py>(
    py: Python<'py>,
    // ...
) -> PyResult<Bound<'py, PyBytes>>
```

### 3. Performance

- Réutiliser les `DescriptorPool` quand possible
- Éviter les allocations inutiles
- Utiliser `Vec::with_capacity` pour les buffers

---

## Prochaines étapes

- [API Python](python.md) - Utiliser la librairie
- [Guide d'installation](setup.md) - Configuration
- [Exemples](../examples/README.md) - Cas d'usage
