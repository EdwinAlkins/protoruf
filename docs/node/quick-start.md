# Quick Start (Node.js)

Get up and running with `@protoruf/node` in under 5 minutes.

## Step 1: Define a schema

You can keep a `.proto` file on disk, or compile sources held in memory. Here's the schema we'll
use:

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

## Step 2: Compile to a descriptor

```ts
import { compileProto, compileProtoFromSources } from "@protoruf/node";

// From disk:
const descriptor = compileProto("message.proto");

// ...or fully in memory (no file needed):
const descriptor2 = compileProtoFromSources(
  {
    "message.proto":
      'syntax="proto3"; package message; message Message { string id = 1; string content = 2; int32 priority = 3; repeated string tags = 4; }',
  },
  "message.proto",
);
```

The descriptor (a `Buffer`) holds the compiled schema and is required for every conversion.

!!! tip "Reuse the descriptor"
    Compiling is a one-time cost. Compile once at startup and reuse the descriptor (or a
    [`DescriptorCache`](advanced.md#high-throughput-descriptorcache)) for every conversion.

## Step 3: JSON → Protobuf

```ts
import { jsonToProtobuf } from "@protoruf/node";

const json = '{"id":"123","content":"Hello","priority":1,"tags":["greeting"]}';
const wire = jsonToProtobuf(json, descriptor, "message.Message");

console.log(`Protobuf: ${wire.length} bytes`);
```

## Step 4: Protobuf → JSON

```ts
import { protobufToJson } from "@protoruf/node";

const out = protobufToJson(wire, descriptor, "message.Message", /* pretty */ true);
console.log(out);
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

## Complete example

```ts
import { compileProto, jsonToProtobuf, protobufToJson } from "@protoruf/node";

const descriptor = compileProto("message.proto");

const json = '{"id":"123","content":"Hello","priority":1,"tags":["greeting"]}';
const wire = jsonToProtobuf(json, descriptor, "message.Message");

const out = protobufToJson(wire, descriptor, "message.Message", true);
console.log(out);
```

Run it (Node ≥ 23 executes `.ts` directly; otherwise use `npx tsx`):

```bash
node quick-start.ts
```

## What's Next?

- [Basic Usage](basic-usage.md) — every supported Protobuf feature
- [Proto Files & Compilation](proto-files.md) — imports, include paths, in-memory compilation
- [Advanced Features](advanced.md) — `DescriptorCache`, Zod integration, performance
