# Advanced Features (Node.js)

Performance, object/Zod integration, and production patterns.

## High throughput: `DescriptorCache`

This is the single most impactful optimization. The free functions (`jsonToProtobuf` /
`protobufToJson`) **re-decode the descriptor set on every call**, which dominates the cost when
converting many messages.

`DescriptorCache` decodes the pool **once** and reuses it across every conversion. Build it once,
reuse it everywhere.

```ts
import { compileProto, DescriptorCache } from "@protoruf/node";

const cache = new DescriptorCache(compileProto("schema.proto"));

for (const json of jsonStream) {
  const wire = cache.jsonToProtobuf(json, "message.Message"); // no descriptor argument
  process(wire);
}

// round-trip back to JSON
const restored = cache.protobufToJson(wire, "message.Message", false);
```

A single cache instance handles every message type in the descriptor. The output is identical
to the free functions.

!!! tip "Prefer the cache"
    Reach for the free functions only for one-off conversions. Any loop or long-lived service
    should hold a `DescriptorCache`.

## Object and Zod integration

`jsonToProtobuf` takes a JSON string, so any object goes through `JSON.stringify`. Wrap it for
an idiomatic, type-safe API — the Node equivalent of Python's Pydantic helpers — using
[Zod](https://zod.dev/) to validate the decoded shape:

```ts
import { z } from "zod";
import { jsonToProtobuf, protobufToJson } from "@protoruf/node";

/** Object -> Protobuf (like pydantic_to_protobuf). */
export function objectToProtobuf<T>(obj: T, descriptor: Buffer, messageType: string): Buffer {
  return jsonToProtobuf(JSON.stringify(obj), descriptor, messageType);
}

/** Protobuf -> validated, typed object (like protobuf_to_pydantic). */
export function protobufToObject<T>(
  bytes: Buffer,
  descriptor: Buffer,
  schema: z.ZodType<T>,
  messageType: string,
): T {
  return schema.parse(JSON.parse(protobufToJson(bytes, descriptor, messageType, false)));
}
```

```ts
const User = z.object({ id: z.string(), tags: z.array(z.string()) });

const wire = objectToProtobuf({ id: "123", tags: ["a", "b"] }, descriptor, "user.User");
const user = protobufToObject(wire, descriptor, User, "user.User"); // typed & validated
```

## Multiple message types

A single descriptor can define many messages; pass the type per call:

```ts
const descriptor = compileProto("ecommerce.proto");

const productPb = jsonToProtobuf('{"id":"p1","price":9.99}', descriptor, "ecommerce.Product");
const orderPb = jsonToProtobuf('{"order_id":"o1","total":9.99}', descriptor, "ecommerce.Order");
```

## Production patterns

### Service wrapper

```ts
import { compileProto, DescriptorCache } from "@protoruf/node";

export class ProtoService {
  private cache: DescriptorCache;
  constructor(protoFile: string) {
    this.cache = new DescriptorCache(compileProto(protoFile));
  }
  encode(json: string, messageType: string): Buffer {
    return this.cache.jsonToProtobuf(json, messageType);
  }
  decode(bytes: Buffer, messageType: string, pretty = false): string {
    return this.cache.protobufToJson(bytes, messageType, pretty);
  }
}
```

### Descriptor registry

```ts
import { compileProto } from "@protoruf/node";

const registry = new Map<string, Buffer>();
registry.set("user", compileProto("user.proto"));
registry.set("order", compileProto("order.proto"));
```

## The 64-bit integer caveat

`core` serializes `int64`/`uint64` as **JSON numbers** (not strings). JavaScript's `JSON.parse`
loads them as `number` (a double) and **loses precision above 2^53**:

```ts
const text = protobufToJson(wire, descriptor, "T", false);
// the JSON *text* keeps the exact digits, e.g. ..."big":9223372036854775807...
JSON.parse(text).big; // 9223372036854776000  <-- precision lost by JSON.parse
```

If you handle large 64-bit values, parse with a BigInt-aware JSON parser (e.g.
[`json-bigint`](https://www.npmjs.com/package/json-bigint)) instead of `JSON.parse`.

## Debugging

```ts
// inspect descriptor size
console.log(`descriptor: ${descriptor.length} bytes`);

// verify a round-trip preserves data
const original = '{"id":"123"}';
const back = protobufToJson(jsonToProtobuf(original, d, "M"), d, "M", false);
console.assert(JSON.parse(back).id === "123");
```

## Next Steps

- [API Reference](api.md)
