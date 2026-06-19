# API Reference (Browser / WASM)

Complete API for `@protoruf/wasm`. All binary values are `Uint8Array`. Every function **throws**
on error. Call `await init()` once before any other call.

---

## `init()` — default export

```ts
export default function init(moduleOrPath?: InitInput | Promise<InitInput>): Promise<InitOutput>;
```

Loads and instantiates the WASM module. With a bundler you can `await init()` with no arguments;
otherwise pass a URL / `Response` / bytes for the `.wasm`. Call it **once** before using any
other export.

```ts
import init from "@protoruf/wasm";
await init();
```

> The `nodejs` wasm-pack target initialises synchronously and does **not** require `init()`.

---

## `compileProtoFromSources()`

Compile `.proto` sources provided **in memory** — no filesystem access.

```ts
function compileProtoFromSources(files: Record<string, string>, root: string): Uint8Array;
```

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `files` | `Record<string, string>` | yes | Map of logical file name → source text. `import`s resolve by name; Google well-known types are provided automatically. |
| `root` | `string` | yes | Entry file (a key of `files`) |

**Returns** `Uint8Array`. **Throws** if compilation fails or `root`/an import is missing.

> `compileProto(path)` is intentionally **not** exposed in WASM (no filesystem).

---

## `jsonToProtobuf()`

```ts
function jsonToProtobuf(jsonStr: string, descriptorBytes: Uint8Array, messageType: string): Uint8Array;
```

| Parameter | Type | Description |
|-----------|------|-------------|
| `jsonStr` | `string` | Valid JSON to convert |
| `descriptorBytes` | `Uint8Array` | Descriptor from `compileProtoFromSources` |
| `messageType` | `string` | Fully-qualified message name (e.g. `"user.User"`) |

**Returns** `Uint8Array` — the encoded message. **Throws** on invalid JSON or unknown `messageType`.

---

## `protobufToJson()`

```ts
function protobufToJson(
  protobufBytes: Uint8Array,
  descriptorBytes: Uint8Array,
  pretty: boolean,
  messageType: string,
): string;
```

| Parameter | Type | Description |
|-----------|------|-------------|
| `protobufBytes` | `Uint8Array` | Encoded message |
| `descriptorBytes` | `Uint8Array` | Descriptor set |
| `pretty` | `boolean` | Indent the output |
| `messageType` | `string` | Fully-qualified message name |

**Returns** `string`. **Throws** on decode/serialization failure.

---

## `class DescriptorCache`

A reusable, pre-decoded descriptor pool. Decode the pool once and reuse it across conversions.

```ts
class DescriptorCache {
  constructor(descriptorBytes: Uint8Array);
  jsonToProtobuf(jsonStr: string, messageType: string): Uint8Array;
  protobufToJson(protobufBytes: Uint8Array, messageType: string, pretty?: boolean): string;
  free(): void;
}
```

- **constructor** throws if `descriptorBytes` is not a valid descriptor set
- **`jsonToProtobuf` / `protobufToJson`** throw on invalid input or unknown `messageType`
- **`free()`** releases the instance's WASM linear-memory state — call it when done, or use a
  `using` declaration (the class implements `Symbol.dispose`)

```ts
using cache = new DescriptorCache(descriptor);
const wire = cache.jsonToProtobuf('{"id":"123"}', "user.User");
const json = cache.protobufToJson(wire, "user.User", false);
```

---

## JSON shape

Output matches protoruf's shared format across all targets:

- field names use the **proto field name** (snake_case)
- **enums** are emitted as their **numeric value** (input accepts name or number)
- **64-bit integers** are emitted as JSON **numbers** — `JSON.parse` loses precision above 2^53
- proto3 **default values** are omitted

## Errors

Failures throw a standard `Error`; inspect `err.message` (e.g. `Invalid JSON: ...`,
`Message type '...' not found in descriptor`).
