# API Reference

Complete API documentation for protoruf with function signatures, parameters, and examples.

## Python API

The Python API consists of the core conversion functions plus the
[`DescriptorCache`](#descriptorcache) class for high-throughput workloads:

---

### `compile_proto()`

Compile a `.proto` file to a descriptor set using Rust protox.

```python
def compile_proto(
    proto_path: str | Path,
    include_paths: list[str | Path] | None = None,
    output_path: str | Path | None = None,
) -> bytes
```

#### Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `proto_path` | `str \| Path` | Yes | Path to the `.proto` file to compile |
| `include_paths` | `list[str \| Path] \| None` | No | List of include directories for resolving imports. Defaults to parent directory of `proto_path` |
| `output_path` | `str \| Path \| None` | No | Optional path to save the descriptor set as a `.desc` file |

#### Returns

`bytes` — Compiled descriptor set containing all message definitions.

#### Raises

- `RuntimeError` — If proto compilation fails (syntax errors, missing imports, etc.)

#### Example

```python
from protoruf import compile_proto

# Basic usage
descriptor = compile_proto("schema.proto")

# With include paths and output file
descriptor = compile_proto(
    proto_path="api/service.proto",
    include_paths=["protos", "common"],
    output_path="descriptors/service.desc"
)
```

---

### `load_descriptor()`

Load a pre-compiled descriptor set from a file.

```python
def load_descriptor(descriptor_path: str | Path) -> bytes
```

#### Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `descriptor_path` | `str \| Path` | Yes | Path to the `.desc` file |

#### Returns

`bytes` — Descriptor set contents.

#### Raises

- `FileNotFoundError` — If the descriptor file does not exist

#### Example

```python
from protoruf import load_descriptor

descriptor = load_descriptor("schema.desc")
```

---

### `compile_proto_from_sources()`

Compile `.proto` sources held in memory, with no filesystem access. `import`
statements are resolved by name against the `files` mapping, and Google well-known
types are resolved automatically.

```python
def compile_proto_from_sources(
    files: dict[str, str],
    root: str,
    include_imports: bool = True,
    output_path: str | Path | None = None,
) -> bytes
```

#### Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `files` | `dict[str, str]` | Yes | Mapping of file name → `.proto` source text |
| `root` | `str` | Yes | Entry file to compile (must be a key of `files`) |
| `include_imports` | `bool` | No | Embed imported files in the descriptor (default `True`) |
| `output_path` | `str \| Path` | No | Optional path to save the descriptor |

#### Returns

`bytes` — The compiled descriptor set. Identical to what `compile_proto()` would
produce for the same sources.

#### Raises

- `ValueError` — If `root` is missing from `files` or a source fails to compile

#### Example

```python
from protoruf import compile_proto_from_sources

files = {
    "user.proto": """
syntax = "proto3";
package user;
message User { string id = 1; string email = 2; }
""",
}

descriptor = compile_proto_from_sources(files, root="user.proto")
```

---

### `compile_protos()`

Compile several root `.proto` files in one go, from memory, disk, or both. Imports are
resolved, in this order, from `sources`, from each directory of `include_paths`, from
the standard protos embedded in protoruf, and from the `google/protobuf/*` well-known
types.

```python
def compile_protos(
    roots: Sequence[str | Path],
    include_paths: Sequence[str | Path] = (),
    sources: Mapping[str, str] | None = None,
    *,
    include_imports: bool = True,
    source_info: bool = False,
    standard_protos: bool = True,
    output_path: str | Path | None = None,
) -> bytes
```

#### Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `roots` | `Sequence[str \| Path]` | Yes | Files to compile: import names (`"orders/v1/orders.proto"`, a key of `sources` or a path relative to an include path), or paths inside one of `include_paths` |
| `include_paths` | `Sequence[str \| Path]` | No | Directories to resolve imports from |
| `sources` | `Mapping[str, str]` | No | In-memory files, name → source text; they take priority over disk |
| `include_imports` | `bool` | No | Embed every imported file so the descriptor is self-contained (default `True`) |
| `source_info` | `bool` | No | Keep comments and source positions, needed to read comments through `Schema` (default `False`; makes the descriptor several times larger) |
| `standard_protos` | `bool` | No | Resolve the embedded `google/rpc/*`, `google/type/*`, `grpc/health/v1` and `grpc/reflection/{v1,v1alpha}` files (default `True`) |
| `output_path` | `str \| Path` | No | Optional path to save the descriptor |

#### Returns

`bytes` — The compiled descriptor set (`FileDescriptorSet`).

#### Raises

- `RuntimeError` — If compilation fails. The message gives the location:
  `Failed to compile proto file: orders.proto:4:3: name 'Missing' is not defined`

#### Example

```python
from protoruf import compile_protos

# A whole proto/ directory, with comments, plus a file generated in memory.
descriptor = compile_protos(
    ["orders/v1/orders.proto", "orders/v1/admin.proto", "gen/app.proto"],
    include_paths=["proto"],
    sources={"gen/app.proto": generated_text},
    source_info=True,
)
```

---

### `Schema`

Immutable, typed view of a compiled descriptor set: files, messages (nested ones and
map entries included), fields, enums, services and their comments. Used to generate
code from `.proto` files, check models against them, and serve gRPC server reflection.

```python
class Schema:
    def __init__(self, descriptor: bytes) -> None: ...
    descriptor: bytes
    files: Mapping[str, FileInfo]          # by file name
    messages: Mapping[str, MessageInfo]    # by full name
    enums: Mapping[str, EnumInfo]
    services: Mapping[str, ServiceInfo]
    def file(self, name: str) -> FileInfo: ...
    def message(self, full_name: str) -> MessageInfo: ...
    def enum(self, full_name: str) -> EnumInfo: ...
    def service(self, full_name: str) -> ServiceInfo: ...
    def codec(self, message_type: str, model: type | None = None, **options) -> Codec: ...
    # gRPC server reflection
    def file_descriptor_proto(self, name: str) -> bytes: ...
    def file_containing_symbol(self, symbol: str) -> str | None: ...
    def file_dependencies(self, name: str) -> tuple[str, ...]: ...
```

The info objects (`protoruf.schema`) are frozen dataclasses:

| Class | Main attributes |
|-------|-----------------|
| `FileInfo` | `name`, `package`, `syntax`, `dependencies`, top-level `messages` / `enums` / `services` (full names) |
| `MessageInfo` | `name`, `full_name`, `file`, `parent`, `fields` (declaration order), `oneofs` (real ones only), `reserved_ranges` (inclusive pairs), `reserved_names`, `is_map_entry`, `deprecated`, `comments`; `field(name)` |
| `FieldInfo` | `name`, `number`, `json_name`, `kind` (`"int32"`, `"string"`, …, `"enum"`, `"message"`), `type_name`, `cardinality`, `is_map`, `map_key_kind`, `map_value_kind`, `map_value_type_name`, `is_packed`, `has_presence`, `proto3_optional`, `oneof`, `default_value`, `deprecated`, `comments`; `is_list` |
| `EnumInfo` / `EnumValueInfo` | `full_name`, `parent`, `values` (`name`, `number`, `deprecated`), `reserved_ranges`, `reserved_names` |
| `ServiceInfo` / `MethodInfo` | `methods`; `input_type`, `output_type`, `client_streaming`, `server_streaming`; `method(name)` |
| `Comments` | `leading`, `trailing`, `detached` as written; `text` (cleaned leading, else trailing comment) |

Comments are only filled when the descriptor was compiled with `source_info=True`.
Lookups of unknown names raise `KeyError`.

#### Example

```python
from protoruf import Schema, compile_protos

schema = Schema(compile_protos(["orders/v1/orders.proto"], ["proto"], source_info=True))
order = schema.message("orders.v1.Order")
for field in order.fields:
    print(field.number, field.name, field.kind, field.type_name, field.has_presence)

schema.file_containing_symbol("orders.v1.Orders.GetOrder")  # "orders/v1/orders.proto"
```

---

### `Codec`

Protobuf bytes ↔ Python objects and Pydantic models, **without JSON**. One codec per
message type, built once (the descriptor is resolved and every nested message planned when
the codec is created), then reused.

```python
schema.codec(
    message_type: str,                      # "orders.v1.Order"
    model: type | None = None,              # Pydantic model; None: plain dicts
    *,
    mode: Literal["validate", "construct", "dict"] = "validate",
    field_names: Mapping[str, Mapping[int, str]] | None = None,
    unknown_enum: Literal["keep", "error"] = "keep",
    naive_datetime: Literal["error", "utc"] = "error",
) -> Codec

class Codec:
    def encode(self, obj: BaseModel | dict) -> bytes: ...
    def decode(self, data: bytes) -> Any: ...        # model instance, or dict
    def decode_dict(self, data: bytes) -> dict: ...  # always plain dicts
```

- **`mode`**: `"validate"` decodes into dicts then validates them with the model (by field
  name, so aliases do not get in the way); `"construct"` builds model instances directly,
  nested models and enum members included, without validation (like `model_construct`,
  about 2× faster); `"dict"` ignores the model.
- **`field_names`**: `{message full name: {field number: Python name}}` for fields whose
  Python name differs from the proto one, in any message of the tree (`from` → `from_`).
- **Encoding** reads a model instance through its `__dict__` (no `model_dump`), or a dict.
- **Proto3 semantics**: zero values of implicit-presence fields are not written and are
  filled back on decode; `optional` fields, messages and oneof members are `None` when
  absent; enums are open (an unknown number is kept as an int, or as a pseudo-member of an
  `OpenIntEnum`, compared with `==`, not `is`) unless `unknown_enum="error"`, which refuses
  undeclared numbers on decoding and on encoding.
- **oneof**: encoding two members of a oneof raises `ValueError`; when decoding, the last
  member read wins, as in the reference implementations.
- **Repeated singular messages** on the wire are merged (scalars from the last occurrence,
  repeated fields concatenated, sub-messages merged), in time linear in their size.

| proto | Python |
|-------|--------|
| `google.protobuf.Timestamp` | aware `datetime.datetime` (UTC); naive refused unless `naive_datetime="utc"` |
| `google.protobuf.Duration` | `datetime.timedelta` |
| `google.type.Date` | `datetime.date` (a partial date, with a zero month or day, raises) |
| `google.protobuf.Struct` / `Value` / `ListValue` | `dict` / JSON value / `list` |
| `google.protobuf.*Value` wrappers | the scalar, or `None` |
| `google.protobuf.FieldMask` | `list[str]` |

Decoding errors (truncated or malformed bytes, field number 0 or above 2^29 − 1, groups,
wrong wire types, invalid UTF-8, nesting deeper than 100) raise `ValueError`; encoding
errors name the field. The encodings are byte-identical to `google.protobuf` (upb), checked
by a differential test.

#### Example

```python
from pydantic import BaseModel
from protoruf import Schema, compile_protos

class Order(BaseModel):
    order_number: str = ""
    from_: str = ""

schema = Schema(compile_protos(["orders/v1/orders.proto"], ["proto"]))
codec = schema.codec("orders.v1.Order", Order, field_names={"orders.v1.Order": {2: "from_"}})
data = codec.encode(Order(order_number="A", from_="Paris"))
assert codec.decode(data) == Order(order_number="A", from_="Paris")
```

`NativeCodec` (the class's name in 0.3 and 0.4) remains available from
`protoruf._protoruf`.

---

### `json_to_protobuf()`

Convert a JSON string to a Protobuf message.

```python
def json_to_protobuf(
    json_str: str,
    descriptor_bytes: bytes,
    message_type: str,
) -> bytes
```

#### Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `json_str` | `str` | Yes | Valid JSON string to convert |
| `descriptor_bytes` | `bytes` | Yes | Compiled descriptor set from `compile_proto()` or `load_descriptor()` |
| `message_type` | `str` | Yes | Full message type name (e.g., `"user.User"`, `"ecommerce.Order"`) |

#### Returns

`bytes` — Protobuf-encoded message.

#### Raises

- `ValueError` — If JSON is invalid or message type is not found in descriptor
- `RuntimeError` — If Protobuf serialization fails

#### Example

```python
from protoruf import json_to_protobuf

json_data = '{"id": "123", "name": "Alice", "email": "alice@example.com"}'

protobuf_bytes = json_to_protobuf(
    json_data,
    descriptor,
    message_type="user.User"
)
```

---

### `protobuf_to_json()`

Convert a Protobuf message to a JSON string.

```python
def protobuf_to_json(
    protobuf_bytes: bytes,
    descriptor_bytes: bytes,
    message_type: str,
    pretty: bool = False,
    include_default_values: bool = False,
) -> str
```

#### Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `protobuf_bytes` | `bytes` | Yes | Protobuf-encoded message |
| `descriptor_bytes` | `bytes` | Yes | Compiled descriptor set |
| `message_type` | `str` | Yes | Full message type name (e.g., `"user.User"`) |
| `pretty` | `bool` | No | If `True`, format JSON with indentation (default: `False`) |
| `include_default_values` | `bool` | No | If `True`, emit fields without presence even at their default value (`0`, `""`, `false`, `[]`, `{}`). Fields with presence (`optional`, messages, `oneof` members) are still emitted only when set (default: `False`) |

#### Returns

`str` — JSON representation of the Protobuf message.

#### Raises

- `RuntimeError` — If Protobuf decoding or JSON serialization fails

#### Example

```python
from protoruf import protobuf_to_json

# Compact JSON (default)
json_str = protobuf_to_json(
    protobuf_bytes,
    descriptor,
    message_type="user.User"
)

# Pretty-printed JSON
json_str = protobuf_to_json(
    protobuf_bytes,
    descriptor,
    message_type="user.User",
    pretty=True
)
```

---

### `pydantic_to_protobuf()`

Convert a Pydantic model directly to a Protobuf message.

```python
def pydantic_to_protobuf(
    pydantic_model: BaseModel,
    descriptor_bytes: bytes,
    message_type: str,
) -> bytes
```

#### Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `pydantic_model` | `BaseModel` | Yes | Pydantic model instance to convert |
| `descriptor_bytes` | `bytes` | Yes | Compiled descriptor set from `compile_proto()` or `load_descriptor()` |
| `message_type` | `str` | Yes | Full message type name (e.g., `"user.User"`, `"ecommerce.Order"`) |

#### Returns

`bytes` — Protobuf-encoded message.

#### Raises

- `ValueError` — If Pydantic model is invalid or message type is not found in descriptor
- `RuntimeError` — If Protobuf serialization fails

#### Example

```python
from pydantic import BaseModel
from protoruf import pydantic_to_protobuf

class Message(BaseModel):
    id: str = ""
    content: str = ""
    priority: int = 0

msg = Message(id="123", content="Hello", priority=1)

protobuf_bytes = pydantic_to_protobuf(
    msg,
    descriptor,
    message_type="message.Message"
)
```

---

### `protobuf_to_pydantic()`

Convert a Protobuf message directly to a Pydantic model instance.

```python
def protobuf_to_pydantic(
    protobuf_bytes: bytes,
    descriptor_bytes: bytes,
    model_class: Type[T],
    message_type: str,
) -> T
```

#### Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `protobuf_bytes` | `bytes` | Yes | Protobuf-encoded message |
| `descriptor_bytes` | `bytes` | Yes | Compiled descriptor set |
| `model_class` | `Type[T]` | Yes | The Pydantic model class to instantiate |
| `message_type` | `str` | Yes | Full message type name (e.g., `"user.User"`) |

#### Returns

`T` — An instance of the specified Pydantic model class.

#### Raises

- `RuntimeError` — If Protobuf decoding or JSON serialization fails
- `ValidationError` — If the decoded JSON doesn't match the Pydantic model schema

#### Example

```python
from pydantic import BaseModel
from protoruf import protobuf_to_pydantic

class Message(BaseModel):
    id: str = ""
    content: str = ""
    priority: int = 0

result = protobuf_to_pydantic(
    protobuf_bytes,
    descriptor,
    Message,
    message_type="message.Message"
)

print(result.content)  # Output: Hello
```

---

### `DescriptorCache`

A reusable, pre-decoded descriptor pool.

The free functions above reuse decoded pools through a process-wide LRU, but
hash the descriptor bytes and resolve the message type on each call.
`DescriptorCache` holds a pool and memoizes message descriptors, avoiding that
per-call work. Build one instance for repeated conversions and measure the
benefit for your workload.

```python
class DescriptorCache:
    def __init__(self, descriptor_bytes: bytes) -> None: ...
    def json_to_protobuf(self, json_str: str, message_type: str) -> bytes: ...
    def protobuf_to_json(
        self,
        protobuf_bytes: bytes,
        message_type: str,
        pretty: bool = False,
        include_default_values: bool = False,
    ) -> str: ...
    def pydantic_to_protobuf(
        self,
        pydantic_model: BaseModel,
        message_type: str,
    ) -> bytes: ...
    def protobuf_to_pydantic(
        self,
        protobuf_bytes: bytes,
        model_class: Type[T],
        message_type: str,
    ) -> T: ...
```

#### Constructor

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `descriptor_bytes` | `bytes` | Yes | Compiled descriptor set from `compile_proto()` or `load_descriptor()` |

Raises `ValueError` if the descriptor bytes cannot be decoded.

#### Methods

`json_to_protobuf(json_str, message_type) -> bytes`

Convert a JSON string to a Protobuf message. Raises `ValueError` if the JSON is
invalid or the message type is not found.

`protobuf_to_json(protobuf_bytes, message_type, pretty=False, include_default_values=False) -> str`

Convert a Protobuf message to a JSON string. Raises `RuntimeError` on decoding
or serialization failure, `ValueError` if the message type is not found.

`pydantic_to_protobuf(pydantic_model, message_type) -> bytes`

Convert a Pydantic model to a Protobuf message. Pure-Python wrapper that calls
`model_dump_json()` and feeds the result to the cached `json_to_protobuf`. Models
with `bytes` fields (at any depth) are dumped through Python objects instead, with
`bytes` written as base64 as the protobuf JSON mapping requires.

`protobuf_to_pydantic(protobuf_bytes, model_class, message_type) -> T`

Convert a Protobuf message to an instance of `model_class`. Pure-Python wrapper
around the cached `protobuf_to_json` (with `include_default_values=True`, so a
required field holding `0`, `""` or `false` is not reported as missing) followed by
`model_class.model_validate_json()`. Models with `bytes` fields are validated in
Python mode after decoding their base64 values; a `strict=True` model then needs
Python types for its other fields.

A single cache instance handles every message type defined in the descriptor and
is safe to share across threads.

#### Example

```python
from protoruf import compile_proto, DescriptorCache

descriptor = compile_proto("schema.proto")

# Decode the pool once, reuse it for every message
cache = DescriptorCache(descriptor)

for json_data in json_stream:
    protobuf_bytes = cache.json_to_protobuf(json_data, "user.User")
    process(protobuf_bytes)

# Round-trip back to JSON
restored = cache.protobuf_to_json(protobuf_bytes, "user.User", pretty=True)

# The same cache also drives Pydantic conversions
protobuf_bytes = cache.pydantic_to_protobuf(user_model, "user.User")
user_model = cache.protobuf_to_pydantic(protobuf_bytes, User, "user.User")
```

The output format is identical to the free functions (snake_case field names,
64-bit integers as JSON numbers, enums as numbers).

## Type Stubs

protoruf includes complete type stubs for IDE autocomplete and type checking:

```python
# _protoruf.pyi
def compile_proto(proto_path: str, include_paths: list[str] | None = None) -> bytes: ...
def compile_proto_from_sources(files: dict[str, str], root: str, include_imports: bool = True) -> bytes: ...
def json_to_protobuf(json_str: str, descriptor_bytes: bytes, message_type: str) -> bytes: ...
def protobuf_to_json(protobuf_bytes: bytes, descriptor_bytes: bytes, message_type: str, pretty: bool = False, include_default_values: bool = False) -> str: ...

class DescriptorCache:
    def __init__(self, descriptor_bytes: bytes) -> None: ...
    def json_to_protobuf(self, json_str: str, message_type: str) -> bytes: ...
    def protobuf_to_json(self, protobuf_bytes: bytes, message_type: str, pretty: bool = False, include_default_values: bool = False) -> str: ...
```

## Module Exports

All public API is exported via `__all__`:

```python
__all__ = [
    "json_to_protobuf",
    "protobuf_to_json",
    "compile_proto",
    "load_descriptor",
    "compile_proto_from_sources",
    "compile_protos",
    "protobuf_to_pydantic",
    "pydantic_to_protobuf",
    "Codec",
    "DescriptorCache",
    "OpenIntEnum",
    "Schema",
]
```

## Error Types

| Exception | When Raised |
|-----------|-------------|
| `ValueError` | Invalid JSON input or message type not found in descriptor |
| `RuntimeError` | Protobuf decoding failure or serialization failure |
| `FileNotFoundError` | Descriptor file path does not exist |

## Rust Architecture

protoruf is implemented in Rust with Python bindings via PyO3:

### Core Components

| Module | Purpose |
|--------|---------|
| `src/core.rs` | Core conversion logic (pure Rust, testable) |
| `src/lib.rs` | Python bindings (PyO3 extension module) |

### Rust Dependencies

| Crate | Purpose |
|-------|---------|
| `prost-reflect` | Protobuf reflection and dynamic message handling |
| `protox` | Proto file compilation to descriptors |
| `serde_json` | JSON parsing and serialization |
| `pyo3` | Python-Rust bindings |

### Extension Module

The Rust extension is exposed as `protoruf._protoruf` and wrapped by Python functions in `protoruf/__init__.py`.

## Next Steps

- Learn about [contributing](../development/contributing.md) to protoruf
- Review the [User Guide](../guide/basic-usage.md) for usage examples
