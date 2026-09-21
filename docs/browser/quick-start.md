# Quick Start (Browser / WASM)

Convert JSON ↔ Protobuf entirely in the browser in a few minutes.

## Step 1: Initialise the module

Always `await init()` once before anything else.

```ts
import init from "@protoruf/wasm";

await init(); // loads & instantiates the .wasm
```

## Step 2: Compile a schema (in memory)

There is no filesystem in the browser, so schemas are compiled from in-memory sources with
`compileProtoFromSources`:

```ts
import { compileProtoFromSources } from "@protoruf/wasm";

const descriptor = compileProtoFromSources(
  {
    "message.proto":
      'syntax="proto3"; package message; message Message { string id = 1; string content = 2; repeated string tags = 3; }',
  },
  "message.proto",
);
```

The descriptor (a `Uint8Array`) holds the compiled schema and is required for every conversion.

## Step 3: JSON → Protobuf

```ts
import { jsonToProtobuf } from "@protoruf/wasm";

const json = '{"id":"123","content":"Hello","tags":["greeting"]}';
const wire = jsonToProtobuf(json, descriptor, "message.Message"); // Uint8Array
```

## Step 4: Protobuf → JSON

```ts
import { protobufToJson } from "@protoruf/wasm";

const out = protobufToJson(wire, descriptor, "message.Message", /* pretty */ true);
console.log(out);
```

**Output:**

```json
{
  "id": "123",
  "content": "Hello",
  "tags": ["greeting"]
}
```

## Complete example

```ts
import init, { compileProtoFromSources, jsonToProtobuf, protobufToJson } from "@protoruf/wasm";

await init();

const descriptor = compileProtoFromSources(
  { "message.proto": 'syntax="proto3"; package message; message Message { string id = 1; }' },
  "message.proto",
);

const wire = jsonToProtobuf('{"id":"123"}', descriptor, "message.Message");
console.log(protobufToJson(wire, descriptor, "message.Message", true));
```

!!! tip "Fixed schemas? Skip compilation"
    If your schemas don't change, compile them **ahead of time** (CLI or server) and ship only
    the descriptor bytes — `fetch` them and skip `compileProtoFromSources` entirely. See
    [Proto Files & Compilation](proto-files.md#loading-a-pre-compiled-descriptor).

## What's Next?

- [Basic Usage](basic-usage.md) — every supported Protobuf feature
- [Proto Files & Compilation](proto-files.md) — imports, well-known types, pre-compiled descriptors
- [Advanced Features](advanced.md) — `DescriptorCache`, Web Workers, security, Zod
