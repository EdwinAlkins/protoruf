# protoruf

<p align="center">
  <img src="docs/assets/logo.png" alt="protoruf logo" width="200">
</p>

<p align="center">
  <strong>High-performance JSON ↔ Protobuf conversion for Python, powered by Rust</strong>
</p>

<p align="center">
  <a href="https://EdwinAlkins.github.io/protoruf/">📖 Documentation</a> •
  <a href="https://github.com/EdwinAlkins/protoruf">💻 Source</a> •
  <a href="https://pypi.org/project/protoruf/">📦 PyPI</a>
</p>

---

A high-performance Python library written in Rust for converting between JSON and Protobuf messages.

## Features

- ⚡ Fast JSON ↔ Protobuf conversion powered by Rust
- 🔒 Type-safe with explicit message type specification
- 📦 Built-in proto compilation with Rust protox (no external dependencies!)

## Installation

```bash
uv pip install -e .
```

## Usage

```python
from protoruf import json_to_protobuf, protobuf_to_json, compile_proto

# Step 1: Compile your .proto file to a descriptor set (using Rust protox)
descriptor = compile_proto("proto/message.proto")

# Or save to a file for reuse
# compile_proto("proto/message.proto", output_path="proto/message.desc")
# descriptor = load_descriptor("proto/message.desc")

# Step 2: Convert JSON to Protobuf (message_type is required)
json_data = '{"id": "123", "content": "Hello", "priority": 1, "tags": ["greeting"]}'
protobuf_bytes = json_to_protobuf(json_data, descriptor, message_type="message.Message")

# Step 3: Convert Protobuf back to JSON (message_type is required)
json_str = protobuf_to_json(protobuf_bytes, descriptor, message_type="message.Message", pretty=True)
print(json_str)
```

### With Pydantic models (in your own code)

You can use Pydantic models in your application code to structure your data:

```python
from pydantic import BaseModel, Field
from protoruf import compile_proto, json_to_protobuf, protobuf_to_json

# Define your own models in your project
class Metadata(BaseModel):
    author: str = ""
    created_at: int = 0
    attributes: dict[str, str] = {}

class Message(BaseModel):
    id: str = ""
    content: str = ""
    priority: int = 0
    tags: list[str] = []
    metadata: Metadata = None

# Compile your proto file
descriptor = compile_proto("proto/message.proto")

# Create a message using Pydantic
msg = Message(
    id="123",
    content="Hello",
    priority=1,
    tags=["greeting"],
    metadata=Metadata(author="Alice")
)

# Convert to Protobuf (message_type is required)
protobuf_bytes = json_to_protobuf(msg.model_dump_json(), descriptor, message_type="message.Message")

# Convert back to JSON
result = protobuf_to_json(protobuf_bytes, descriptor, message_type="message.Message")
```

## Examples

See the `examples/` directory for more usage examples:

- `01_basic_user_example.py` - Basic user service example
- `02_ecommerce_example.py` - E-commerce order example
- `03_iot_sensors_example.py` - IoT sensors data example
- `04_pydantic_integration.py` - Pydantic integration example

## Development

```bash
# Install dependencies
uv sync

# Build the Rust extension
uv run maturin develop

# Run Python tests
uv run pytest

# Run Rust tests
cargo test --lib

# Type checking with mypy
uv run mypy python/protoruf/ --ignore-missing-imports

# Run all checks (tests + type checking)
uv run pytest && cargo test --lib && uv run mypy python/protoruf/ --ignore-missing-imports
```

### Pre-commit checklist

Before committing, make sure all checks pass:

```bash
# 1. Python tests
uv run pytest tests/ -v

# 2. Rust tests
cargo test --lib

# 3. Type checking
uv run mypy python/protoruf/ --ignore-missing-imports

# 4. Build verification
uv run maturin develop
```

## Project Structure

```
rust-json-probuff/
├── python/protoruf/           # Python package
│   ├── __init__.py             # Main API: json_to_protobuf, protobuf_to_json
│   ├── _protoruf.pyi          # Type stubs for Rust extension
│   ├── compiler.py             # Proto compilation utilities
│   └── py.typed                # PEP 561 marker
├── src/                        # Rust source code
│   ├── core.rs                 # Core logic (pure Rust, testable)
│   └── lib.rs                  # Python bindings (PyO3)
├── tests/                      # Python test suite
│   ├── proto/                  # Test proto files
│   ├── test_models.py          # Pydantic models for tests
│   └── test_rust_json_probuff.py
├── examples/                   # Usage examples with their own .proto files
└── doc/                        # Documentation
```

## API Reference

### `compile_proto(proto_path, include_paths=None, output_path=None)`

Compile a `.proto` file to a descriptor set.

- **proto_path**: Path to the `.proto` file
- **include_paths**: Optional list of include paths for imports
- **output_path**: Optional path to save the descriptor set

Returns the descriptor set as bytes.

### `load_descriptor(descriptor_path)`

Load a pre-compiled descriptor set from a file.

- **descriptor_path**: Path to the `.desc` file

Returns the descriptor set as bytes.

### `json_to_protobuf(json_str, descriptor_bytes, message_type)`

Convert a JSON string to a Protobuf message.

- **json_str**: JSON string to convert
- **descriptor_bytes**: Compiled protobuf descriptor set
- **message_type**: Full message type name (e.g., `"user.User"`, `"ecommerce.Order"`)

Returns the Protobuf message as bytes.

### `protobuf_to_json(protobuf_bytes, descriptor_bytes, message_type, pretty=False)`

Convert a Protobuf message to a JSON string.

- **protobuf_bytes**: Protobuf message as bytes
- **descriptor_bytes**: Compiled protobuf descriptor set
- **message_type**: Full message type name (e.g., `"user.User"`, `"ecommerce.Order"`)
- **pretty**: If `True`, format JSON with indentation

Returns the JSON string representation.

## License

MIT
