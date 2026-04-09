# Advanced Features

This guide covers advanced usage patterns including Pydantic integration, performance optimization, and production best practices.

## Pydantic Integration

protoruf works seamlessly with [Pydantic](https://docs.pydantic.dev/) models for type-safe data handling.

### Define Your Models

```python
from pydantic import BaseModel, Field
from protoruf import compile_proto, json_to_protobuf, protobuf_to_json

# Define Pydantic models matching your proto schema
class Metadata(BaseModel):
    author: str = ""
    created_at: int = 0
    attributes: dict[str, str] = {}

class Message(BaseModel):
    id: str = ""
    content: str = ""
    priority: int = 0
    tags: list[str] = []
    metadata: Metadata | None = None
```

### Convert via JSON Serialization

```python
# Compile proto
descriptor = compile_proto("message.proto")

# Create message with Pydantic
msg = Message(
    id="123",
    content="Hello, World!",
    priority=1,
    tags=["greeting", "example"],
    metadata=Metadata(author="Alice", created_at=1234567890)
)

# Convert to Protobuf via JSON
json_str = msg.model_dump_json()
protobuf_bytes = json_to_protobuf(
    json_str,
    descriptor,
    message_type="message.Message"
)

# Convert back and reconstruct model
result_json = protobuf_to_json(
    protobuf_bytes,
    descriptor,
    message_type="message.Message"
)
result_msg = Message.model_validate_json(result_json)
```

### Benefits of Pydantic Integration

- ✅ **Validation**: Automatic data validation before serialization
- ✅ **Type Safety**: Catch errors at model creation time
- ✅ **Defaults**: Handle default values consistently
- ✅ **IDE Support**: Autocomplete and type checking

## Performance Optimization

### 1. Cache Descriptors

Compile descriptors once and reuse them:

```python
from functools import lru_cache
from protoruf import compile_proto

@lru_cache(maxsize=32)
def get_descriptor(proto_file: str) -> bytes:
    return compile_proto(proto_file)

# First call compiles, subsequent calls return cached
descriptor = get_descriptor("schema.proto")
```

### 2. Use Descriptor Files

Avoid recompiling in production:

```python
# Build step
compile_proto("schema.proto", output_path="schema.desc")

# Runtime (faster)
from protoruf import load_descriptor
descriptor = load_descriptor("schema.desc")
```

### 3. Batch Processing

For high-throughput scenarios, minimize function calls:

```python
# Good: reuse descriptor for multiple messages
descriptor = compile_proto("schema.proto")

for json_data in json_stream:
    protobuf_bytes = json_to_protobuf(json_data, descriptor, "message.Message")
    process(protobuf_bytes)
```

### 4. Pretty Printing Overhead

Skip pretty printing in production:

```python
# Faster (default)
json_str = protobuf_to_json(protobuf_bytes, descriptor, "message.Message")

# Slower (use only for debugging/logs)
json_str = protobuf_to_json(protobuf_bytes, descriptor, "message.Message", pretty=True)
```

## Error Handling Best Practices

### Validate Early

Catch errors as close to input as possible:

```python
def convert_safely(json_str: str, descriptor: bytes, msg_type: str) -> bytes:
    try:
        # Validate JSON first
        import json
        json.loads(json_str)  # Raises ValueError if invalid
        
        return json_to_protobuf(json_str, descriptor, msg_type)
    except ValueError as e:
        print(f"Invalid input: {e}")
        raise
    except RuntimeError as e:
        print(f"Conversion failed: {e}")
        raise
```

### Handle Missing Message Types

```python
def convert_with_fallback(json_str: str, descriptor: bytes, msg_type: str) -> bytes | None:
    try:
        return json_to_protobuf(json_str, descriptor, msg_type)
    except ValueError as e:
        if "not found" in str(e):
            print(f"Message type '{msg_type}' not in descriptor")
        else:
            print(f"Invalid JSON: {e}")
        return None
```

## Working with Multiple Message Types

### Descriptor with Multiple Messages

A single `.proto` file can define multiple messages:

```protobuf
syntax = "proto3";

package ecommerce;

message Product {
  string id = 1;
  string name = 2;
  double price = 3;
}

message Order {
  string order_id = 1;
  repeated Product items = 2;
  double total = 3;
}
```

Use different message types with the same descriptor:

```python
descriptor = compile_proto("ecommerce.proto")

# Convert Product
product_json = '{"id": "p1", "name": "Widget", "price": 9.99}'
product_pb = json_to_protobuf(product_json, descriptor, "ecommerce.Product")

# Convert Order
order_json = '{"order_id": "o1", "items": [{"id": "p1", "name": "Widget", "price": 9.99}], "total": 9.99}'
order_pb = json_to_protobuf(order_json, descriptor, "ecommerce.Order")
```

## Production Patterns

### Service Wrapper

Create a service class for clean APIs:

```python
from protoruf import compile_proto, json_to_protobuf, protobuf_to_json

class ProtoService:
    def __init__(self, proto_file: str):
        self.descriptor = compile_proto(proto_file)
    
    def encode(self, json_str: str, message_type: str) -> bytes:
        return json_to_protobuf(json_str, self.descriptor, message_type)
    
    def decode(self, protobuf_bytes: bytes, message_type: str, pretty: bool = False) -> str:
        return protobuf_to_json(protobuf_bytes, self.descriptor, message_type, pretty)

# Usage
service = ProtoService("schema.proto")
encoded = service.encode('{"id": "123"}', "message.Message")
decoded = service.decode(encoded, "message.Message", pretty=True)
```

### Descriptor Registry

Manage multiple proto schemas:

```python
class DescriptorRegistry:
    def __init__(self):
        self._descriptors: dict[str, bytes] = {}
    
    def register(self, name: str, proto_file: str):
        from protoruf import compile_proto
        self._descriptors[name] = compile_proto(proto_file)
    
    def get(self, name: str) -> bytes:
        if name not in self._descriptors:
            raise KeyError(f"Descriptor '{name}' not registered")
        return self._descriptors[name]

# Usage
registry = DescriptorRegistry()
registry.register("user", "user.proto")
registry.register("order", "order.proto")

user_desc = registry.get("user")
```

## Debugging Tips

### Inspect Descriptor Contents

Print the descriptor to understand its structure:

```python
descriptor = compile_proto("schema.proto")
print(f"Descriptor size: {len(descriptor)} bytes")
```

### Test Round-Trips

Verify conversions preserve data:

```python
original = '{"id": "123", "name": "Test"}'
encoded = json_to_protobuf(original, descriptor, "message.Message")
decoded = protobuf_to_json(encoded, descriptor, "message.Message")

assert original == decoded, "Round-trip failed!"
```

## Next Steps

- Review the complete [API Reference](../api/reference.md)
- Learn about [contributing](../development/contributing.md) to protoruf
