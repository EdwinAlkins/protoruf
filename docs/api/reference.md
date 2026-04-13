# API Reference

Complete API documentation for protoruf with function signatures, parameters, and examples.

## Python API

The Python API consists of four core functions:

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
    pretty: bool = False,
    message_type: str = None,
) -> str
```

#### Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `protobuf_bytes` | `bytes` | Yes | Protobuf-encoded message |
| `descriptor_bytes` | `bytes` | Yes | Compiled descriptor set |
| `pretty` | `bool` | No | If `True`, format JSON with indentation (default: `False`) |
| `message_type` | `str` | Yes | Full message type name (e.g., `"user.User"`) |

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

## Type Stubs

protoruf includes complete type stubs for IDE autocomplete and type checking:

```python
# _protoruf.pyi
def compile_proto(proto_path: str, include_paths: list[str] | None = None) -> bytes: ...
def json_to_protobuf(json_str: str, descriptor_bytes: bytes, message_type: str) -> bytes: ...
def protobuf_to_json(protobuf_bytes: bytes, descriptor_bytes: bytes, pretty: bool = False, message_type: str = ...) -> str: ...
```

## Module Exports

All public API is exported via `__all__`:

```python
__all__ = [
    "json_to_protobuf",
    "protobuf_to_json",
    "compile_proto",
    "load_descriptor",
    "protobuf_to_pydantic",
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
