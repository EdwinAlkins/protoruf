# protoruf

<p align="center">
  <img src="https://raw.githubusercontent.com/EdwinAlkins/protoruf/main/docs/assets/logo.png" alt="protoruf logo" width="200">
</p>

<p align="center">
  <strong>High-performance JSON ↔ Protobuf conversion for Python, powered by Rust</strong>
</p>

<p align="center">
  <a href="https://EdwinAlkins.github.io/protoruf/">📖 Documentation</a> •
  <a href="https://github.com/EdwinAlkins/protoruf">💻 Source</a> •
  <a href="https://github.com/EdwinAlkins/protoruf/issues">🐛 Issues</a>
</p>

---

A high-performance Python library written in Rust for converting between JSON and Protobuf messages.

## Features

- ⚡ **Fast** — JSON ↔ Protobuf conversion powered by Rust
- 🔒 **Type-safe** — Explicit message type specification
- 📦 **Built-in proto compilation** — Uses Rust protox, no external dependencies

## Installation

```bash
pip install protoruf
```

## Quick Start

```python
from protoruf import json_to_protobuf, protobuf_to_json, compile_proto

# 1. Compile your .proto file
descriptor = compile_proto("message.proto")

# 2. Convert JSON to Protobuf
json_data = '{"id": "123", "content": "Hello", "priority": 1}'
protobuf_bytes = json_to_protobuf(json_data, descriptor, message_type="message.Message")

# 3. Convert Protobuf back to JSON
json_str = protobuf_to_json(protobuf_bytes, descriptor, message_type="message.Message", pretty=True)
print(json_str)
```

## API Reference

### `compile_proto(proto_path, include_paths=None, output_path=None)`

Compile a `.proto` file to a descriptor set.

| Parameter        | Type                | Description                          |
|------------------|---------------------|--------------------------------------|
| `proto_path`     | `str`               | Path to the `.proto` file            |
| `include_paths`  | `list[str] \| None` | Optional include paths for imports   |
| `output_path`    | `str \| None`       | Optional path to save the descriptor |

**Returns:** `bytes` — The compiled descriptor set.

---

### `load_descriptor(descriptor_path)`

Load a pre-compiled descriptor set from a file.

| Parameter         | Type  | Description              |
|-------------------|-------|--------------------------|
| `descriptor_path` | `str` | Path to the `.desc` file |

**Returns:** `bytes` — The descriptor set.

---

### `json_to_protobuf(json_str, descriptor_bytes, message_type)`

Convert a JSON string to a Protobuf message.

| Parameter          | Type    | Description                                       |
|--------------------|---------|---------------------------------------------------|
| `json_str`         | `str`   | JSON string to convert                            |
| `descriptor_bytes` | `bytes` | Compiled protobuf descriptor set                  |
| `message_type`     | `str`   | Full message type name (e.g. `"message.Message"`) |

**Returns:** `bytes` — The Protobuf message.

---

### `protobuf_to_json(protobuf_bytes, descriptor_bytes, message_type, pretty=False)`

Convert a Protobuf message to a JSON string.

| Parameter          | Type     | Description                                       |
|--------------------|----------|---------------------------------------------------|
| `protobuf_bytes`   | `bytes`  | Protobuf message as bytes                         |
| `descriptor_bytes` | `bytes`  | Compiled protobuf descriptor set                  |
| `message_type`     | `str`    | Full message type name (e.g. `"message.Message"`) |
| `pretty`           | `bool`   | If `True`, format JSON with indentation           |

**Returns:** `str` — The JSON representation.

## Usage with Pydantic

```python
from pydantic import BaseModel
from protoruf import compile_proto, json_to_protobuf, protobuf_to_json

class Message(BaseModel):
    id: str = ""
    content: str = ""
    priority: int = 0

descriptor = compile_proto("message.proto")
msg = Message(id="123", content="Hello", priority=1)

protobuf_bytes = json_to_protobuf(msg.model_dump_json(), descriptor, message_type="message.Message")
result = protobuf_to_json(protobuf_bytes, descriptor, message_type="message.Message")
```

## Links

- **📖 Full Documentation:** https://EdwinAlkins.github.io/protoruf/
- **💻 Source Code:** https://github.com/EdwinAlkins/protoruf
- **🐛 Bug Reports:** https://github.com/EdwinAlkins/protoruf/issues
- **📦 PyPI:** https://pypi.org/project/protoruf/

## License

MIT
