# API Reference (Node.js)

Complete API for `@protoruf/node`. All binary values are Node `Buffer` (a `Uint8Array` at
runtime). Every function **throws** a standard `Error` on failure.

---

## `compileProto()`

Compile a `.proto` file **from disk** to a descriptor set.

```ts
function compileProto(protoPath: string, includePaths?: string[]): Buffer;
```

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `protoPath` | `string` | yes | Path to the `.proto` file |
| `includePaths` | `string[]` | no | Include directories for resolving `import`s (default: parent dir of `protoPath`) |

**Returns** `Buffer` — the serialized descriptor set.

**Throws** if compilation fails (syntax error, missing import, …).

> Node/native only — reads the filesystem. In the browser use `compileProtoFromSources`.

```ts
const descriptor = compileProto("api/service.proto", ["protos", "common"]);
```

---

## `compileProtoFromSources()`

Compile `.proto` sources provided **in memory** — no filesystem access.

```ts
function compileProtoFromSources(files: Record<string, string>, root: string): Buffer;
```

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `files` | `Record<string, string>` | yes | Map of logical file name → source text. `import`s resolve by name within this map; Google well-known types are provided automatically. |
| `root` | `string` | yes | Entry file to compile (a key of `files`) |

**Returns** `Buffer`.

**Throws** if compilation fails or `root`/an import is missing.

```ts
const descriptor = compileProtoFromSources(
  { "user.proto": 'syntax="proto3"; package user; message User { string id = 1; }' },
  "user.proto",
);
```

---

## `jsonToProtobuf()`

Convert a JSON string to a Protobuf message.

```ts
function jsonToProtobuf(jsonStr: string, descriptorBytes: Buffer, messageType: string): Buffer;
```

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `jsonStr` | `string` | yes | Valid JSON to convert |
| `descriptorBytes` | `Buffer` | yes | Descriptor from `compileProto*` |
| `messageType` | `string` | yes | Fully-qualified message name (e.g. `"user.User"`) |

**Returns** `Buffer` — the encoded message.

**Throws** on invalid JSON or unknown `messageType`.

---

## `protobufToJson()`

Convert a Protobuf message to a JSON string.

```ts
function protobufToJson(
  protobufBytes: Buffer,
  descriptorBytes: Buffer,
  messageType: string,
  pretty?: boolean | undefined | null,
): string;
```

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `protobufBytes` | `Buffer` | yes | Encoded message |
| `descriptorBytes` | `Buffer` | yes | Descriptor set |
| `messageType` | `string` | yes | Fully-qualified message name |
| `pretty` | `boolean` | no | Indent the output (default `false`) |

**Returns** `string` — the JSON representation.

**Throws** on decode/serialization failure.

---

## `class DescriptorCache`

A reusable, pre-decoded descriptor pool. The free functions decode the descriptor set on
**every** call; `DescriptorCache` decodes the pool once and reuses it, the dominant performance
lever for hot loops. Build one and reuse it everywhere.

```ts
class DescriptorCache {
  constructor(descriptorBytes: Buffer);
  jsonToProtobuf(jsonStr: string, messageType: string): Buffer;
  protobufToJson(protobufBytes: Buffer, messageType: string, pretty?: boolean): string;
}
```

**Constructor** — `descriptorBytes: Buffer`. Throws if the bytes are not a valid descriptor set.

**`jsonToProtobuf(jsonStr, messageType)`** → `Buffer`. Throws on invalid JSON or unknown type.

**`protobufToJson(protobufBytes, messageType, pretty?)`** → `string`. Throws on decode failure.

A single instance handles every message type in the descriptor. The output is identical to the
free functions.

```ts
const cache = new DescriptorCache(descriptor);
const wire = cache.jsonToProtobuf('{"id":"123"}', "user.User");
const json = cache.protobufToJson(wire, "user.User", true);
```

---

## JSON shape

Output matches protoruf's shared format across all targets:

- field names use the **proto field name** (snake_case)
- **enums** are emitted as their **numeric value** (input accepts name or number)
- **64-bit integers** are emitted as JSON **numbers** — see the
  [precision caveat](advanced.md#the-64-bit-integer-caveat)
- proto3 **default values** are omitted

## Errors

Failures throw a standard `Error`; inspect `err.message`:

| Cause | Message contains |
|-------|------------------|
| Invalid JSON | `Invalid JSON: ...` |
| Unknown message type | `Message type '...' not found in descriptor` |
| Decode failure | `Decoding error: ...` |
| Compilation failure | `Failed to compile proto file: ...` |
