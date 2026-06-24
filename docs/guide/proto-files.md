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

## Compiling From In-Memory Sources

When your `.proto` definitions are not stored on disk — generated at runtime,
received over the network, or kept in a database — use
`compile_proto_from_sources()`. It compiles entirely from memory and never touches
the filesystem.

```python
compile_proto_from_sources(
    files: dict[str, str],
    root: str,
    include_imports: bool = True,
    output_path: str | Path | None = None,
) -> bytes
```

### Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `files` | `dict[str, str]` | Yes | Mapping of file name → `.proto` source text |
| `root` | `str` | Yes | Entry file to compile (must be a key of `files`) |
| `include_imports` | `bool` | No | Embed imported files in the descriptor (default `True`) |
| `output_path` | `str \| Path` | No | Save the descriptor to a file |

### Returns

Compiled descriptor set as `bytes`. The output is identical to what
`compile_proto()` produces for the same sources.

### Basic Example

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

### Imports Between In-Memory Files

`import` statements are resolved by name against the keys of the `files` mapping,
so multi-file schemas work without any include paths:

```python
files = {
    "common/types.proto": """
syntax = "proto3";
package common;
message Timestamp { int64 seconds = 1; }
""",
    "api/service.proto": """
syntax = "proto3";
package api;
import "common/types.proto";
message Request {
  common.Timestamp timestamp = 1;
  string endpoint = 2;
}
""",
}

descriptor = compile_proto_from_sources(files, root="api/service.proto")
```

!!! note "Well-known types"
    Google well-known types (`google/protobuf/*.proto`, e.g. `Timestamp`,
    `Struct`, `Any`) are resolved automatically — you don't need to add them to
    `files`.

### The `include_imports` Option

When `True` (the default), every transitively-imported file — including Google
well-known types — is embedded in the descriptor, making it fully self-contained.
Set it to `False` for a smaller descriptor that decodes faster, when the consumer
does not need the embedded imports:

```python
descriptor = compile_proto_from_sources(
    files,
    root="api/service.proto",
    include_imports=False,
)
```

### Saving the Descriptor

Like `compile_proto()`, you can write the result straight to a file:

```python
compile_proto_from_sources(files, root="user.proto", output_path="user.desc")
```

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
