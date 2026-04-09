# Proto Files & Compilation

This guide covers working with `.proto` files, including compilation options, imports, and descriptor management.

## The compile_proto Function

```python
compile_proto(
    proto_path: str | Path,
    include_paths: list[str | Path] | None = None,
    output_path: str | Path | None = None,
) -> bytes
```

### Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `proto_path` | `str \| Path` | Yes | Path to the `.proto` file |
| `include_paths` | `list[str \| Path]` | No | Additional include directories for imports |
| `output_path` | `str \| Path` | No | Save descriptor to file |

### Returns

Compiled descriptor set as `bytes`.

## Basic Compilation

The simplest usage compiles a single `.proto` file:

```python
from protoruf import compile_proto

descriptor = compile_proto("message.proto")
```

This automatically uses the parent directory of the proto file as the include path.

## Working with Imports

If your proto file imports other `.proto` files, you may need to specify include paths:

### Example Structure

```
project/
├── protos/
│   ├── common/
│   │   ├── types.proto
│   └── api/
│       ├── service.proto
```

### service.proto

```protobuf
syntax = "proto3";

package api;

import "common/types.proto";

message Request {
  common.Timestamp timestamp = 1;
  string endpoint = 2;
}
```

### Compile with Include Paths

```python
from protoruf import compile_proto

descriptor = compile_proto(
    "protos/api/service.proto",
    include_paths=["protos"]  # Root directory for imports
)
```

!!! tip "Include Path Resolution"
    The `include_paths` parameter tells the compiler where to look for imported `.proto` files. Paths are resolved relative to the current working directory.

## Saving and Loading Descriptors

### Save Descriptor to File

For production use, compile once and save:

```python
compile_proto(
    "schema.proto",
    output_path="schema.desc"
)
```

### Load Descriptor from File

```python
from protoruf import load_descriptor

descriptor = load_descriptor("schema.desc")
```

This is faster than recompiling and avoids distributing `.proto` files.

## Proto File Best Practices

### Use Explicit Packages

Always define a package in your proto files:

```protobuf
syntax = "proto3";

package myapp;  // Good

message User {
  string id = 1;
}
```

The message type will be `myapp.User`.

### Organize Imports

Group related messages in separate files:

```
protos/
├── user.proto      # User-related messages
├── order.proto     # Order-related messages
└── common.proto    # Shared types
```

### Use Descriptor Files in Production

For deployment, compile descriptors ahead of time:

```python
# build_descriptors.py
from protoruf import compile_proto

compile_proto("protos/user.proto", output_path="descriptors/user.desc")
compile_proto("protos/order.proto", output_path="descriptors/order.desc")
```

Load them in your application:

```python
from protoruf import load_descriptor, json_to_protobuf

user_desc = load_descriptor("descriptors/user.desc")
user_pb = json_to_protobuf(json_data, user_desc, message_type="user.User")
```

## Descriptor Format

Descriptors are serialized `FileDescriptorSet` protobuf messages. They contain:

- Message type definitions
- Field schemas (type, number, labels)
- Enum definitions
- Nested message structures

!!! note "Binary Format"
    Descriptor files (`.desc`) are binary and should not be edited manually. Always regenerate from `.proto` sources.

## Multiple Proto Files

Compile multiple files into a single descriptor:

```python
from protoruf import compile_proto

# Compile main file (imports are included automatically)
descriptor = compile_proto(
    "api/main.proto",
    include_paths=["api", "common"]
)
```

The compiler resolves all imports and includes them in the descriptor.

## Troubleshooting

### Import Not Found

```
Error: File not found: common/types.proto
```

**Solution:** Add the correct include path:

```python
compile_proto("api/service.proto", include_paths=["protos"])
```

### Message Type Not Found

```
ValueError: Message type 'user.User' not found
```

**Solution:** Verify the message type matches the package and name in your `.proto` file.

### Descriptor File Corrupted

```
RuntimeError: Failed to parse descriptor
```

**Solution:** Regenerate the descriptor file from the `.proto` source.

## Next Steps

- Learn about [Advanced Features](advanced.md) including Pydantic integration
- Check the [API Reference](../api/reference.md) for complete function signatures
