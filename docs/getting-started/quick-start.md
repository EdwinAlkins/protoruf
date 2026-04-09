# Quick Start

Get up and running with protoruf in under 5 minutes. This guide walks you through compiling a `.proto` file and converting JSON to Protobuf.

## Step 1: Create a `.proto` File

Create a file named `message.proto`:

```protobuf
syntax = "proto3";

package message;

message Message {
  string id = 1;
  string content = 2;
  int32 priority = 3;
  repeated string tags = 4;
}
```

This defines a simple message type with string, integer, and repeated fields.

## Step 2: Compile the Proto File

Use `compile_proto()` to compile your `.proto` file into a descriptor:

```python
from protoruf import compile_proto

# Compile the .proto file (returns descriptor bytes)
descriptor = compile_proto("message.proto")
```

The descriptor contains the compiled schema and is required for all conversions.

!!! tip "Save Descriptor for Reuse"
    If you compile once and reuse across runs:
    ```python
    compile_proto("message.proto", output_path="message.desc")
    # Later...
    from protoruf import load_descriptor
    descriptor = load_descriptor("message.desc")
    ```

## Step 3: Convert JSON to Protobuf

```python
from protoruf import json_to_protobuf

json_data = '{"id": "123", "content": "Hello", "priority": 1, "tags": ["greeting"]}'

# Convert to Protobuf (message_type is required)
protobuf_bytes = json_to_protobuf(
    json_data,
    descriptor,
    message_type="message.Message"
)

print(f"Protobuf bytes: {len(protobuf_bytes)} bytes")
```

## Step 4: Convert Protobuf back to JSON

```python
from protoruf import protobuf_to_json

# Convert back to JSON
json_str = protobuf_to_json(
    protobuf_bytes,
    descriptor,
    message_type="message.Message",
    pretty=True  # Format with indentation
)

print(json_str)
```

**Output:**

```json
{
  "id": "123",
  "content": "Hello",
  "priority": 1,
  "tags": ["greeting"]
}
```

## Complete Example

Here's the full workflow in one script:

```python
from protoruf import compile_proto, json_to_protobuf, protobuf_to_json

# 1. Compile proto
descriptor = compile_proto("message.proto")

# 2. JSON → Protobuf
json_data = '{"id": "123", "content": "Hello", "priority": 1, "tags": ["greeting"]}'
protobuf_bytes = json_to_protobuf(json_data, descriptor, message_type="message.Message")

# 3. Protobuf → JSON
result = protobuf_to_json(
    protobuf_bytes,
    descriptor,
    message_type="message.Message",
    pretty=True
)
print(result)
```

## What's Next?

- [**Basic Usage Guide**](../guide/basic-usage.md) — Learn about all supported Protobuf features
- [**Proto Files Guide**](../guide/proto-files.md) — Advanced proto compilation and imports
- [**Advanced Features**](../guide/advanced.md) — Pydantic integration, descriptor caching, and performance tips
