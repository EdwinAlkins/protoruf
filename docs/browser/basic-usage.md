# Basic Usage (Browser / WASM)

This guide covers converting between JSON and Protobuf in the browser. Always
[`await init()`](installation.md#initialise) once before any call.

## Core Functions

| Function | Description |
|----------|-------------|
| `init()` (default export) | Load & instantiate the `.wasm` (call once) |
| `compileProtoFromSources()` | Compile `.proto` sources held in memory |
| `jsonToProtobuf()` | Convert a JSON string to Protobuf bytes |
| `protobufToJson()` | Convert Protobuf bytes to a JSON string |
| `DescriptorCache` | Pre-decoded pool for high-throughput conversion |

!!! note "No `compileProto(path)`"
    There is no filesystem in the browser, so the disk-based `compileProto` is **not** exposed.
    Compile from in-memory sources, or load a pre-compiled descriptor's bytes.

All binary values are `Uint8Array`. Every function **throws** on error.

## Compiling

```ts
const descriptor = compileProtoFromSources({ "schema.proto": "syntax=\"proto3\"; ..." }, "schema.proto");
```

See [Proto Files & Compilation](proto-files.md) for imports and well-known types.

## JSON → Protobuf

```ts
const wire = jsonToProtobuf('{"id":"123","name":"Alice"}', descriptor, "user.User");
```

- `jsonStr: string`, `descriptorBytes: Uint8Array`, `messageType: string`
- **returns** `Uint8Array`

## Protobuf → JSON

```ts
const json = protobufToJson(wire, descriptor, "user.User", /* pretty */ true);
```

- `protobufBytes: Uint8Array`, `descriptorBytes: Uint8Array`, `pretty: boolean`, `messageType: string`
- **returns** `string`

## Supported Protobuf Features

### Scalar types

```protobuf
message User { string name = 1; int32 age = 2; int64 id = 3; double score = 4; bool active = 5; bytes data = 6; }
```

```ts
JSON.stringify({ name: "Alice", age: 30, id: 123456789, score: 95.5, active: true, data: "base64==" });
```

### Nested messages

```protobuf
message Address { string street = 1; string city = 2; }
message User { string name = 1; Address address = 2; }
```

```ts
JSON.stringify({ name: "Bob", address: { street: "123 Main St", city: "Springfield" } });
```

### Enums

On **input**, enums accept the name or the number; on **output** they are emitted as the
**numeric value**:

```protobuf
enum Priority { LOW = 0; MEDIUM = 1; HIGH = 2; }
message Task { string title = 1; Priority priority = 2; }
```

```ts
jsonToProtobuf('{"title":"x","priority":"HIGH"}', d, "Task"); // name in
JSON.parse(protobufToJson(wire, d, "Task", false)).priority;  // 2 (number out)
```

### Repeated fields & maps

```protobuf
message Team { repeated string members = 1; map<string, string> labels = 2; }
```

```ts
JSON.stringify({ members: ["Alice", "Bob"], labels: { env: "prod" } });
```

### Oneof fields

```protobuf
message Event { oneof kind { string text = 1; int32 code = 2; } }
```

Set exactly one arm; only the set field appears on output.

### Default values

proto3 defaults (empty string, `0`, `false`, enum `0`, empty list/map) are **omitted** from the
output JSON.

## Error Handling

Failures throw a standard `Error`:

```ts
try {
  jsonToProtobuf("not json", descriptor, "user.User");
} catch (err) {
  console.error((err as Error).message); // "Invalid JSON: ..."
}
```

## Next Steps

- [Proto Files & Compilation](proto-files.md)
- [Advanced Features](advanced.md) — `DescriptorCache`, Web Workers, security, Zod
- [API Reference](api.md)
