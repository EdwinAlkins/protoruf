# protoruf — LLM Usage Prompt (Browser / WASM)

<div class="llm-copy-mount"></div>

> Feed this file to an LLM (or paste it into a system prompt) so it can use the
> **Browser (WebAssembly)** target of `protoruf` correctly with no other context.
> It is a complete, self-contained reference for version **0.2.0**.

## What protoruf is

`protoruf` converts between **JSON and Protobuf** *dynamically* — no `protoc`, no
generated classes, no codegen step. You compile a `.proto` to a **descriptor set**
once, then convert JSON ↔ Protobuf wire bytes in both directions. The browser
target is a WebAssembly module (built with wasm-bindgen) wrapping a pure-Rust
engine. It runs in the WASM sandbox: **no filesystem, no network, no DOM**.

Core idea: a `descriptor` (compiled schema, a `Uint8Array`) + a `messageType`
(fully qualified name like `"user.User"`) are required for every conversion.

## Install

The npm registry release is coming soon. For v0.2.0, install the prebuilt tarball
by URL:

```bash
npm install https://github.com/EdwinAlkins/protoruf/releases/download/v0.2.0/protoruf-wasm-0.2.0.tgz
```

The published package is the **`web`/bundler** build — it works with bundlers
(Vite, webpack, Rollup) and native ESM (`<script type="module">`). Once
published, the install becomes `npm i @protoruf/wasm` with identical imports.

## ⚠️ Two things unique to the WASM target

1. **You MUST call `init()` once before anything else.** The `.wasm` loads
   asynchronously; the default export is an async initializer. Await it at
   startup.
2. **There is NO `compileProto(path)`.** The browser has no filesystem. Compile
   from in-memory sources with `compileProtoFromSources`, or ship a pre-compiled
   descriptor's bytes and pass them straight to the conversion functions.

```ts
import init, { compileProtoFromSources, jsonToProtobuf } from "@protoruf/wasm";

await init(); // fetch + instantiate the .wasm — do this ONCE at startup
```

If you serve the files yourself, the module and the `.wasm` must be served over
**HTTP(S)** — ES modules and `WebAssembly` fetch do not work from `file://`. With
a bundler, `init()` resolves the `.wasm` asset for you.

## Complete public API

Binary values cross as `Uint8Array`. Every function **throws** a JS `Error` on
failure. Available exports: the default `init`, `compileProtoFromSources`,
`jsonToProtobuf`, `protobufToJson`, and the `DescriptorCache` class.

### Compilation (in-memory only)

```ts
function compileProtoFromSources(
  files: Record<string, string>,
  root: string,
  includeImports?: boolean, // default true
): Uint8Array;
```
Compile `.proto` sources held **in memory**. `files` maps a logical filename → its
source text; `import`s resolve by name within this map, and Google well-known
types are provided automatically. `root` is the entry file (a key of `files`).
`includeImports=true` embeds imported files so the descriptor is self-contained.

> There is intentionally **no** `compileProto(path)` in the browser target.

### Conversion (one-shot)

```ts
function jsonToProtobuf(jsonStr: string, descriptorBytes: Uint8Array, messageType: string): Uint8Array;
function protobufToJson(protobufBytes: Uint8Array, descriptorBytes: Uint8Array, messageType: string, pretty?: boolean): string;
```
`messageType` is the **fully qualified** name `"<package>.<Message>"`, e.g.
`"user.User"`. `pretty` (default `false`) indents the JSON output. These
re-resolve the message type on every call; for hot loops use `DescriptorCache`.

### `DescriptorCache` (high-throughput)

Decoding the descriptor set is the dominant cost. `DescriptorCache` decodes the
pool **once** and memoizes resolved message descriptors. One instance handles
every message type in the descriptor.

```ts
class DescriptorCache {
  constructor(descriptorBytes: Uint8Array);
  jsonToProtobuf(jsonStr: string, messageType: string): Uint8Array;
  protobufToJson(protobufBytes: Uint8Array, messageType: string, pretty?: boolean): string;
  free(): void; // release the instance's WASM linear-memory state
}
```
Cache methods take **no** `descriptorBytes` argument (the pool is already
decoded). Output format is identical to the free functions.

**Memory management (WASM-specific):** the cache holds state in the WASM linear
memory, which the JS garbage collector cannot reclaim. Call `cache.free()` when
done (or use `using cache = new DescriptorCache(...)` — it implements
`Symbol.dispose`) to avoid leaks. This applies only to the WASM target.

## Canonical examples

### Minimal round-trip

```ts
import init, { compileProtoFromSources, jsonToProtobuf, protobufToJson } from "@protoruf/wasm";

await init();
const descriptor = compileProtoFromSources(
  { "message.proto": 'syntax="proto3"; package message; message Message { string id = 1; }' },
  "message.proto",
);
const pb = jsonToProtobuf('{"id":"123"}', descriptor, "message.Message");
console.log(protobufToJson(pb, descriptor, "message.Message", true));
```

### High-throughput loop with `DescriptorCache`

```ts
import init, { compileProtoFromSources, DescriptorCache } from "@protoruf/wasm";

await init();
const descriptor = compileProtoFromSources(
  { "user.proto": 'syntax="proto3"; package user; message User { string id = 1; }' },
  "user.proto",
);
const cache = new DescriptorCache(descriptor); // decode pool once
for (const jsonData of jsonStream) {
  const pb = cache.jsonToProtobuf(jsonData, "user.User");
  handle(pb);
}
```

### Using a pre-compiled descriptor (no compilation in the browser)

```ts
import init, { jsonToProtobuf } from "@protoruf/wasm";

await init();
// descriptorBytes fetched from your server / bundled as an asset
const descriptor = new Uint8Array(await (await fetch("/schema.desc")).arrayBuffer());
const pb = jsonToProtobuf('{"id":"123"}', descriptor, "user.User");
```

## JSON shape — READ THIS (non-standard)

protoruf uses a **specific, non-default** JSON mapping. When generating or
validating JSON for it, follow these rules exactly:

- **Field names are proto names (`snake_case`)**, NOT the camelCase of canonical
  protobuf JSON. Use `created_at`, not `createdAt`.
- **Enums are emitted as numbers** (their integer value), not names.
- **64-bit integers (`int64`/`uint64`/…) are JSON numbers, not strings.**
  ⚠️ In JavaScript, numbers above `Number.MAX_SAFE_INTEGER` (2^53 − 1) lose
  precision. For exact large 64-bit values, parse with a BigInt-aware reader.
- **`bytes` fields** map to standard base64-encoded strings.
- **Fields at their proto3 default (0, `""`, `false`, empty repeated/map) are
  omitted** from the output JSON. Don't rely on them being present after a
  round-trip; re-parsing yields the default anyway.
- Standard proto3 JSON applies otherwise (nested messages → objects, repeated →
  arrays, map → object, `oneof` → the single set field, well-known types like
  `Timestamp`/`Duration`/`Struct` use their canonical JSON forms).

## Errors

All functions **throw** a JS `Error` whose message describes the failure:
compilation errors (syntax, missing import), invalid JSON, an unknown
`messageType`, or a protobuf decode/serialize failure. Wrap calls in `try/catch`.
A common early mistake is calling a function **before `await init()`** — always
initialize first.

## Rules for the LLM

1. **Call `await init()` exactly once at startup before any other protoruf call.**
2. There is **no filesystem**: never use `compileProto(path)` (it doesn't exist
   here). Use `compileProtoFromSources`, or pass pre-compiled descriptor bytes.
3. Every conversion needs both `descriptorBytes` (`Uint8Array`) and a
   fully-qualified `messageType` (`"<package>.<Message>"`).
4. For more than a few conversions, build one `DescriptorCache` and reuse it.
5. Emit JSON in protoruf's shape (snake_case fields, numeric enums, numeric
   64-bit ints) — do not assume canonical camelCase protobuf JSON.
6. Serve the app over HTTP(S); WASM/ESM won't load from `file://`.
