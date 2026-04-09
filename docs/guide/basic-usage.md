# Basic Usage

This guide covers the core functionality of protoruf: converting between JSON and Protobuf messages.

## Core Functions

protoruf provides four main functions:

| Function | Description |
|----------|-------------|
| `compile_proto()` | Compile a `.proto` file to a descriptor |
| `load_descriptor()` | Load a pre-compiled descriptor from a file |
| `json_to_protobuf()` | Convert JSON string to Protobuf bytes |
| `protobuf_to_json()` | Convert Protobuf bytes to JSON string |

## Compiling Proto Files

Before converting, you must compile your `.proto` file into a descriptor:

```python
from protoruf import compile_proto

# Compile and get descriptor in memory
descriptor = compile_proto("schema.proto")
```

### Saving Descriptors

For production use, compile once and save the descriptor:

```python
# Compile and save
compile_proto("schema.proto", output_path="schema.desc")

# Load later (faster than recompiling)
from protoruf import load_descriptor
descriptor = load_descriptor("schema.desc")
```

## JSON → Protobuf

Convert a JSON string to Protobuf bytes:

```python
from protoruf import json_to_protobuf

json_data = '{"id": "123", "name": "Alice", "email": "alice@example.com"}'

protobuf_bytes = json_to_protobuf(
    json_data,
    descriptor,
    message_type="user.User"
)
```

### Parameters

- **`json_str`**: Valid JSON string
- **`descriptor_bytes`**: Descriptor from `compile_proto()` or `load_descriptor()`
- **`message_type`**: Full message type (e.g., `"package.Message"`)

### Returns

Protobuf message as `bytes`.

## Protobuf → JSON

Convert Protobuf bytes back to a JSON string:

```python
from protoruf import protobuf_to_json

json_str = protobuf_to_json(
    protobuf_bytes,
    descriptor,
    message_type="user.User",
    pretty=True  # Optional: formatted output
)
```

### Parameters

- **`protobuf_bytes`**: Protobuf message as bytes
- **`descriptor_bytes`**: Compiled descriptor
- **`message_type`**: Full message type name
- **`pretty`**: If `True`, format JSON with indentation (default: `False`)

### Returns

JSON string representation of the message.

## Supported Protobuf Features

### Scalar Types

All standard Protobuf scalar types are supported:

```protobuf
message User {
  string name = 1;
  int32 age = 2;
  int64 id = 3;
  double score = 4;
  bool active = 5;
  bytes data = 6;
}
```

```python
json_data = '''
{
  "name": "Alice",
  "age": 30,
  "id": 123456789,
  "score": 95.5,
  "active": true,
  "data": "base64encoded=="
}
'''
```

### Nested Messages

```protobuf
message Address {
  string street = 1;
  string city = 2;
  string country = 3;
}

message User {
  string name = 1;
  Address address = 2;
}
```

```python
json_data = '''
{
  "name": "Bob",
  "address": {
    "street": "123 Main St",
    "city": "Springfield",
    "country": "US"
  }
}
'''
```

### Enums

Enums are converted as strings:

```protobuf
enum Priority {
  LOW = 0;
  MEDIUM = 1;
  HIGH = 2;
}

message Task {
  string title = 1;
  Priority priority = 2;
}
```

```python
json_data = '{"title": "Fix bug", "priority": "HIGH"}'
# or
json_data = '{"title": "Fix bug", "priority": 2}'
```

### Repeated Fields

```protobuf
message Team {
  string name = 1;
  repeated string members = 2;
}
```

```python
json_data = '''
{
  "name": "Engineering",
  "members": ["Alice", "Bob", "Charlie"]
}
'''
```

### Maps

```protobuf
message Config {
  map<string, string> settings = 1;
}
```

```python
json_data = '''
{
  "settings": {
    "theme": "dark",
    "language": "en",
    "timezone": "UTC"
  }
}
'''
```

### Oneof Fields

```protobuf
message Event {
  oneof event_type {
    string text_message = 1;
    int32 numeric_code = 2;
  }
}
```

```python
# Only one field from the oneof should be present
json_data = '{"text_message": "Hello"}'
# or
json_data = '{"numeric_code": 200}'
```

## Error Handling

protoruf raises exceptions for invalid inputs:

```python
try:
    protobuf_bytes = json_to_protobuf(
        '{"invalid": "data"}',
        descriptor,
        message_type="user.User"
    )
except ValueError as e:
    print(f"Conversion failed: {e}")
```

### Common Errors

- **`ValueError`**: Invalid JSON or message type not found in descriptor
- **`RuntimeError`**: Protobuf decoding or serialization failure
- **`FileNotFoundError`**: Descriptor file does not exist

## Next Steps

- Learn about [Proto Files & Compilation](proto-files.md) for advanced compiler usage
- Explore [Advanced Features](advanced.md) including Pydantic integration
