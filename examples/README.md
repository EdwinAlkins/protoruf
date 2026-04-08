# Examples for protoruf

This directory contains examples demonstrating how to use the `protoruf` library in various scenarios.

## Prerequisites

Make sure the library is installed:

```bash
# From the project root
uv sync
uv run maturin develop
```

## Available Examples

### 1. Basic User Service Example
**File:** `01_basic_user_example.py`  
**Proto:** `user_service.proto`

Demonstrates basic JSON ↔ Protobuf conversion with:
- Simple fields (strings, integers, booleans)
- Nested messages (Profile)
- Enums (UserRole)
- Repeated fields (permissions)
- Map fields (social_links)

```bash
uv run python examples/01_basic_user_example.py
```

### 2. E-commerce Order System
**File:** `02_ecommerce_example.py`  
**Proto:** `ecommerce.proto`

Shows more complex scenarios:
- Deeply nested messages (Order → OrderItem, Address, PaymentInfo)
- Multiple enums (OrderStatus, PaymentMethod, PaymentStatus)
- Repeated messages (items list)
- Compression ratio comparison

```bash
uv run python examples/02_ecommerce_example.py
```

### 3. IoT Sensor Data
**File:** `03_iot_sensors_example.py`  
**Proto:** `iot_sensors.proto`

Demonstrates advanced Protobuf features:
- `oneof` fields (SensorData with type-specific data)
- Multiple message types (Temperature, Humidity, Pressure, etc.)
- Batch processing (SensorBatch)
- Alert systems (DeviceAlert)
- Device registration

```bash
uv run python examples/03_iot_sensors_example.py
```

### 4. Pydantic Integration
**File:** `04_pydantic_integration.py`  
**Proto:** `user_service.proto`

Shows integration with Pydantic for:
- Type-safe JSON validation before Protobuf conversion
- Field validation (email, patterns, lengths)
- Custom validators
- Round-trip verification
- Batch processing with validation

```bash
uv run python examples/04_pydantic_integration.py
```

## Quick Start

```python
from protoruf import compile_proto, json_to_protobuf, protobuf_to_json

# 1. Compile your .proto file
descriptor = compile_proto("path/to/schema.proto")

# 2. Convert JSON to Protobuf
json_data = '{"id": "123", "name": "Example"}'
protobuf_bytes = json_to_protobuf(json_data, descriptor)

# 3. Convert Protobuf back to JSON
result = protobuf_to_json(protobuf_bytes, descriptor, pretty=True)
```

## API Reference

### `compile_proto(proto_path, include_paths=None, output_path=None)`

Compile a `.proto` file to a descriptor set.

- **proto_path**: Path to the `.proto` file
- **include_paths**: Optional list of include directories (defaults to parent of proto_path)
- **output_path**: Optional path to save the descriptor (.desc file)
- **Returns**: Descriptor set as bytes

### `json_to_protobuf(json_str, descriptor_bytes)`

Convert a JSON string to Protobuf bytes.

- **json_str**: JSON string to convert
- **descriptor_bytes**: Compiled descriptor set from `compile_proto`
- **Returns**: Protobuf encoded bytes

### `protobuf_to_json(protobuf_bytes, descriptor_bytes, pretty=False)`

Convert Protobuf bytes to JSON string.

- **protobuf_bytes**: Protobuf encoded bytes
- **descriptor_bytes**: Compiled descriptor set
- **pretty**: If True, format JSON with indentation
- **Returns**: JSON string

## Proto Files

| File | Description |
|------|-------------|
| `user_service.proto` | User management with roles, profiles, permissions |
| `ecommerce.proto` | Order system with items, payments, inventory |
| `iot_sensors.proto` | IoT sensor readings with oneof, batches, alerts |
