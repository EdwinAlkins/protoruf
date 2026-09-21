# protoruf — LLM Usage Prompt (Node.js)

<div class="llm-copy-mount"></div>

> Feed this file to an LLM (or paste it into a system prompt) so it can use the
> **Node.js** target of `protoruf` correctly with no other context. It is a
> complete, self-contained reference for version **0.2.0**.

## What protoruf is

`protoruf` converts between **JSON and Protobuf** *dynamically* — no `protoc`, no
generated classes, no codegen step. You compile a `.proto` to a **descriptor set**
once, then convert JSON ↔ Protobuf wire bytes in both directions. The Node target
is a native addon (built with napi-rs) wrapping a pure-Rust engine, so it runs at
Rust speed and needs no Rust toolchain at install time.

Core idea: a `descriptor` (compiled schema, a `Buffer`) + a `messageType` (fully
qualified name like `"user.User"`) are required for every conversion.

## Install

The npm registry release is coming soon. For v0.2.0, install the self-contained
prebuilt tarball by URL:

```bash
npm install https://github.com/EdwinAlkins/protoruf/releases/download/v0.2.0/protoruf-node-0.2.0.tgz
```

Requires **Node.js ≥ 18** (N-API v8). The tarball bundles native binaries for
Linux x64/arm64, macOS x64/arm64, and Windows x64; the loader picks the right one
at runtime. TypeScript definitions ship in the package (`index.d.ts`) — no
`@types/...` needed. Once published, the install becomes `npm i @protoruf/node`
with identical imports.

```ts
import { compileProto, jsonToProtobuf, protobufToJson } from "@protoruf/node";
```

## The mental model (3 steps)

1. **Compile** a `.proto` schema to a descriptor (`Buffer`) — do this once.
2. **Convert** with `jsonToProtobuf` / `protobufToJson` using that descriptor and
   a `messageType`.
3. For throughput, wrap the descriptor in a **`DescriptorCache`** and reuse it.

## Complete public API

Binary values are Node `Buffer`s (which are `Uint8Array`s at runtime). Every
function **throws** a standard `Error` on failure.

### Compilation

```ts
function compileProto(protoPath: string, includePaths?: string[]): Buffer;
```
Compile a `.proto` **from disk**. `includePaths` defaults to the parent directory
of `protoPath`. Node-only (reads the filesystem).

```ts
function compileProtoFromSources(
  files: Record<string, string>,
  root: string,
  includeImports?: boolean, // default true
): Buffer;
```
Compile `.proto` sources held **in memory** (no filesystem). `files` maps a
logical filename → its source text; `import`s resolve by name within this map, and
Google well-known types are provided automatically. `root` is the entry file (a
key of `files`). `includeImports=true` embeds imported files so the descriptor is
self-contained.

### Conversion (one-shot)

```ts
function jsonToProtobuf(jsonStr: string, descriptorBytes: Buffer, messageType: string): Buffer;
function protobufToJson(protobufBytes: Buffer, descriptorBytes: Buffer, messageType: string, pretty?: boolean): string;
```
`messageType` is the **fully qualified** name `"<package>.<Message>"`, e.g.
`"user.User"`. `pretty` (default `false`) indents the JSON output. These
re-resolve the message type on every call; for hot loops use `DescriptorCache`.

### `DescriptorCache` (high-throughput)

Free functions reuse decoded pools through a process-wide LRU, but hash the
descriptor bytes and resolve the message type on every call. `DescriptorCache`
holds the pool and memoizes message descriptors, avoiding that per-call work.
One instance handles every message type in the descriptor.

```ts
class DescriptorCache {
  constructor(descriptorBytes: Buffer);
  jsonToProtobuf(jsonStr: string, messageType: string): Buffer;
  protobufToJson(protobufBytes: Buffer, messageType: string, pretty?: boolean): string;
}
```
Cache methods take **no** `descriptorBytes` argument (the pool is already
decoded). Output format is identical to the free functions.

## Canonical examples

### Minimal round-trip (from disk)

```ts
import { compileProto, jsonToProtobuf, protobufToJson } from "@protoruf/node";

const descriptor = compileProto("message.proto");
const pb = jsonToProtobuf('{"id":"123"}', descriptor, "message.Message");
console.log(protobufToJson(pb, descriptor, "message.Message", true));
```

### Compile from in-memory sources

```ts
import { compileProtoFromSources, jsonToProtobuf } from "@protoruf/node";

const descriptor = compileProtoFromSources(
  { "user.proto": 'syntax="proto3"; package user; message User { string id = 1; string email = 2; }' },
  "user.proto",
);
const pb = jsonToProtobuf('{"id":"1","email":"a@b.com"}', descriptor, "user.User");
```

### High-throughput loop with `DescriptorCache`

```ts
import { compileProto, DescriptorCache } from "@protoruf/node";

const cache = new DescriptorCache(compileProto("schema.proto")); // decode pool once
for (const jsonData of jsonStream) {
  const pb = cache.jsonToProtobuf(jsonData, "message.Message");
  process(pb);
}
const restored = cache.protobufToJson(pb, "message.Message", true);
```

## JSON shape — READ THIS (non-standard)

protoruf uses a **specific, non-default** JSON mapping. When generating or
validating JSON for it, follow these rules exactly:

- **Field names are proto names (`snake_case`)**, NOT the camelCase of canonical
  protobuf JSON. Use `created_at`, not `createdAt`.
- **Enums are emitted as numbers** (their integer value), not names.
- **64-bit integers (`int64`/`uint64`/…) are JSON numbers, not strings.**
  ⚠️ In JavaScript, numbers above `Number.MAX_SAFE_INTEGER` (2^53 − 1) lose
  precision. For exact large 64-bit values, parse the JSON with a BigInt-aware
  reader; `JSON.parse` will silently round.
- **`bytes` fields** map to standard base64-encoded strings.
- **Fields at their proto3 default (0, `""`, `false`, empty repeated/map) are
  omitted** from the output JSON. Don't rely on them being present after a
  round-trip; re-parsing yields the default anyway.
- Standard proto3 JSON applies otherwise (nested messages → objects, repeated →
  arrays, map → object, `oneof` → the single set field, well-known types like
  `Timestamp`/`Duration`/`Struct` use their canonical JSON forms).

## Errors

All functions **throw** a standard `Error` whose message describes the failure:
proto compilation errors (syntax, missing import), invalid JSON, an unknown
`messageType`, or a protobuf decode/serialize failure. Wrap calls in `try/catch`.

## Rules for the LLM

1. Always compile (or load) a descriptor **before** converting; every conversion
   needs both `descriptorBytes` and a fully-qualified `messageType`.
2. `messageType` must be `"<package>.<Message>"` matching the `.proto`'s `package`
   and message name — a wrong name throws.
3. `compileProto(path)` reads the filesystem and is Node-only — for the browser
   use the WASM target's `compileProtoFromSources`.
4. For more than a few conversions, build one `DescriptorCache` and reuse it;
   don't call the free functions in a tight loop.
5. Emit JSON in protoruf's shape (snake_case fields, numeric enums, numeric
   64-bit ints) — do not assume canonical camelCase protobuf JSON.
6. Binary payloads are `Buffer`s; pass the descriptor `Buffer` through unchanged.
