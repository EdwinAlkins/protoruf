# Advanced Features (Browser / WASM)

Performance, memory management, Web Workers, security and Zod integration.

## High throughput: `DescriptorCache`

The free functions decode the descriptor set on **every** call. `DescriptorCache` decodes the
pool **once** and reuses it — the dominant performance lever for repeated conversions.

```ts
import { DescriptorCache } from "@protoruf/wasm";

const cache = new DescriptorCache(descriptor);
for (const json of jsonStream) {
  const wire = cache.jsonToProtobuf(json, "message.Message");
  process(wire);
}
const restored = cache.protobufToJson(wire, "message.Message", false);
cache.free(); // release native (linear-memory) state when done
```

## Memory management

WASM objects own state in the module's **linear memory**, which the JS garbage collector does
**not** reclaim automatically. `DescriptorCache` therefore exposes `free()`:

```ts
const cache = new DescriptorCache(descriptor);
try {
  // ... use cache ...
} finally {
  cache.free();
}
```

Or use the `using` declaration (TC39 explicit resource management) for automatic cleanup:

```ts
using cache = new DescriptorCache(descriptor); // freed at scope exit
```

!!! warning "Don't leak caches"
    Forgetting to `free()` a long-lived `DescriptorCache` grows WASM linear memory. Always free
    caches you no longer need.

## Security and Web Workers

Compiling/decoding **attacker-controlled** input deserves care. The WASM sandbox has no
filesystem or network access, so the realistic worst case is a **denial of service of the
current tab** (CPU/memory) — not code execution or data exfiltration.

Recommended mitigations for untrusted input:

- **Run in a Web Worker** with a **timeout** — a hang or memory blow-up won't freeze the UI,
  and the worker can be terminated.
- **Prefer pre-compiled descriptors** — don't expose `compileProtoFromSources` to arbitrary
  user input unless your use case truly needs it (e.g. an online schema editor).
- **Bound input sizes** — cap the length of `.proto`, JSON and protobuf bytes upstream.
- **Validate the output** with a schema (Zod, below) rather than trusting decoded structure.
- **Pin integrity** — SRI on the `.wasm`, and keep dependencies updated.

Full analysis: <https://github.com/EdwinAlkins/protoruf/blob/main/dev-docs/javascript-typescript.md>.

```ts
// worker.ts
import init, { compileProtoFromSources, jsonToProtobuf } from "@protoruf/wasm";
const ready = init();
onmessage = async (e) => {
  await ready;
  const d = compileProtoFromSources(e.data.files, e.data.root);
  postMessage(jsonToProtobuf(e.data.json, d, e.data.type));
};
// main thread: terminate the worker if it exceeds a time budget.
```

## Object and Zod integration

`jsonToProtobuf` takes a JSON string, so wrap it for a typed, validated API — the browser
equivalent of Python's Pydantic helpers — using [Zod](https://zod.dev/):

```ts
import { z } from "zod";
import { jsonToProtobuf, protobufToJson } from "@protoruf/wasm";

export function objectToProtobuf<T>(obj: T, descriptor: Uint8Array, messageType: string): Uint8Array {
  return jsonToProtobuf(JSON.stringify(obj), descriptor, messageType);
}

export function protobufToObject<T>(
  bytes: Uint8Array,
  descriptor: Uint8Array,
  schema: z.ZodType<T>,
  messageType: string,
): T {
  return schema.parse(JSON.parse(protobufToJson(bytes, descriptor, false, messageType)));
}
```

## The 64-bit integer caveat

`int64`/`uint64` are emitted as JSON **numbers**; `JSON.parse` loses precision above 2^53. Use a
BigInt-aware JSON parser if you need large 64-bit values exact.

## Next Steps

- [API Reference](api.md)
