# Basic Usage (Node.js)

This guide covers the core functionality: converting between JSON and Protobuf.

## Core Functions

| Function | Description |
|----------|-------------|
| `compileProto()` | Compile a `.proto` file from disk to a descriptor |
| `compileProtoFromSources()` | Compile `.proto` sources held in memory |
| `jsonToProtobuf()` | Convert a JSON string to Protobuf bytes |
| `protobufToJson()` | Convert Protobuf bytes to a JSON string |
| `DescriptorCache` | Pre-decoded pool for high-throughput conversion |

!!! tip "Converting many messages?"
    For hot loops and long-lived services, use
    [`DescriptorCache`](advanced.md#high-throughput-descriptorcache) instead of the free
    functions — it decodes the descriptor once and is significantly faster.

All binary values are Node `Buffer` (a `Uint8Array` at runtime). Every function **throws** on
error.

## Compiling

```ts
import { compileProto } from "@protoruf/node";

const descriptor = compileProto("schema.proto");
```

See [Proto Files & Compilation](proto-files.md) for imports, include paths and in-memory
compilation.

## JSON → Protobuf

```ts
import { jsonToProtobuf } from "@protoruf/node";

const json = '{"id":"123","name":"Alice","email":"alice@example.com"}';
const wire = jsonToProtobuf(json, descriptor, "user.User");
```

**Parameters**

- `jsonStr: string` — valid JSON
- `descriptorBytes: Buffer` — descriptor from `compileProto*`
- `messageType: string` — fully-qualified message name (e.g. `"user.User"`)

**Returns** `Buffer` — the encoded Protobuf message.

## Protobuf → JSON

```ts
import { protobufToJson } from "@protoruf/node";

const json = protobufToJson(wire, descriptor, /* pretty */ true, "user.User");
```

**Parameters**

- `protobufBytes: Buffer`
- `descriptorBytes: Buffer`
- `pretty: boolean` — indent the output
- `messageType: string`

**Returns** `string` — the JSON representation.

## Supported Protobuf Features

### Scalar types

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

```ts
const json = JSON.stringify({
  name: "Alice",
  age: 30,
  id: 123456789,
  score: 95.5,
  active: true,
  data: "base64encoded==", // bytes are base64-encoded strings
});
```

### Nested messages

```protobuf
message Address { string street = 1; string city = 2; }
message User { string name = 1; Address address = 2; }
```

```ts
const json = JSON.stringify({ name: "Bob", address: { street: "123 Main St", city: "Springfield" } });
```

### Enums

```protobuf
enum Priority { LOW = 0; MEDIUM = 1; HIGH = 2; }
message Task { string title = 1; Priority priority = 2; }
```

On **input**, enums accept either the name or the number:

```ts
jsonToProtobuf('{"title":"Fix bug","priority":"HIGH"}', descriptor, "Task");
jsonToProtobuf('{"title":"Fix bug","priority":2}', descriptor, "Task");
```

On **output**, enums are emitted as their **numeric value**:

```ts
JSON.parse(protobufToJson(wire, descriptor, false, "Task")).priority; // 2
```

### Repeated fields

```protobuf
message Team { string name = 1; repeated string members = 2; }
```

```ts
JSON.stringify({ name: "Engineering", members: ["Alice", "Bob", "Charlie"] });
```

### Maps

```protobuf
message Config { map<string, string> settings = 1; }
```

```ts
JSON.stringify({ settings: { theme: "dark", language: "en" } });
```

### Oneof fields

```protobuf
message Event {
  oneof event_type {
    string text_message = 1;
    int32 numeric_code = 2;
  }
}
```

Set exactly one arm; only the set field appears on output:

```ts
jsonToProtobuf('{"text_message":"Hello"}', descriptor, "Event");
jsonToProtobuf('{"numeric_code":200}', descriptor, "Event");
```

### Default values

proto3 default values (empty string, `0`, `false`, enum `0`, empty list/map) are **omitted**
from the output JSON:

```ts
JSON.parse(protobufToJson(jsonToProtobuf('{"id":"x"}', d, "user.User"), d, false, "user.User"));
// => { id: "x" }   // unset fields are absent
```

## Error Handling

Every function throws a standard `Error` on failure:

```ts
try {
  jsonToProtobuf("not json", descriptor, "user.User");
} catch (err) {
  console.error("Conversion failed:", (err as Error).message);
}
```

| Cause | Message contains |
|-------|------------------|
| Invalid JSON | `Invalid JSON: ...` |
| Unknown message type | `Message type '...' not found in descriptor` |
| Decode failure | `Decoding error: ...` |
| Compilation failure | `Failed to compile proto file: ...` |

## Working with objects

`jsonToProtobuf` takes a JSON string, so any object goes through `JSON.stringify`. The
idiomatic helper (the Node equivalent of Python's Pydantic integration) is shown in
[Advanced Features](advanced.md#object-and-zod-integration).

## Next Steps

- [Proto Files & Compilation](proto-files.md)
- [Advanced Features](advanced.md)
- [API Reference](api.md)
