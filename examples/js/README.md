# protoruf — JS/TS examples (Node & WASM)

Two runnable TypeScript examples sharing the same Rust core, one per target:

| File | Target | Binding |
|------|--------|---------|
| `node-example.ts` | Node.js native addon | napi-rs (`../../dist/index.js`) |
| `wasm-example.ts` | WebAssembly, run in **Node** | wasm-bindgen (`../../dist/wasm/protoruf.js`) |
| `browser-example.html` | WebAssembly, run in the **browser** | wasm-bindgen (`../../dist/wasm-web/protoruf.js`) |

Both compile a schema **in memory** with `compileProtoFromSources`, do a JSON ↔ Protobuf
round-trip, and reuse a `DescriptorCache`. See [dev-docs/npm-build.md](../../dev-docs/npm-build.md)
for the full build/distribution guide.

> The `.ts` files run directly on **Node ≥ 23** (native type-stripping). On older Node,
> prefix the run command with `npx tsx` (`npm i -D tsx` first).

---

## Node example (napi)

From the **repo root**:

```bash
npm install            # installs @napi-rs/cli (devDependency)
npm run build:debug    # builds ../../dist/{index.js, index.d.ts, *.node}

node examples/js/node-example.ts
# older Node:  npx tsx examples/js/node-example.ts
```

Expected output (abridged):

```
compiled descriptor: 325 bytes
protobuf wire size: 66 bytes
round-trip: { id: 'A-1001', status: 1, items: [ ... ], labels: { ... } }
cached #0: {"id":"A-0"}
✔ node example done
```

The Node build also exposes `compileProto("schema.proto")` to read a `.proto` from disk
(not available in the browser/WASM build).

---

## WASM example

The example runs in **Node** using the `nodejs` wasm-pack target (synchronous init,
no `fetch`). From the **repo root**:

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-pack                                   # once (or use the installer script)
wasm-pack build --target nodejs --out-dir dist/wasm -- --features wasm

node examples/js/wasm-example.ts
# older Node:  npx tsx examples/js/wasm-example.ts
```

> `compileProto(path)` is **not** exposed in WASM (no filesystem). Always compile from
> in-memory sources, or load a pre-compiled descriptor's bytes.

---

## Browser example (`browser-example.html`)

The browser needs the **`web`** wasm-pack target (ESM + async `init()`), built into a
**separate** dir so it doesn't clobber the Node (`nodejs`) build:

```bash
wasm-pack build --target web --out-dir dist/wasm-web -- --features wasm
```

Then serve over HTTP and open the page (ES modules + `WebAssembly` fetch don't work from
`file://`):

```bash
# IMPORTANT: serve from the repo ROOT, not from examples/js/
cd ../..                    # if you are in examples/js
npx serve .                 # any static server works
# then open: http://localhost:3000/examples/js/browser-example.html
```

> **Common pitfall.** If you run `npx serve .` *inside* `examples/js/`, the import
> `../../dist/wasm-web/protoruf.js` escapes the server root, so `serve` returns its 404
> page as `text/html` and the browser rejects it:
> *"blocked due to a disallowed MIME type (text/html)"*. Serve from the repo root.

The page compiles the schema, does a JSON ↔ Protobuf round-trip, and prints the result.
Key difference from Node: you must `await init()` once before any call.

```ts
import init, { compileProtoFromSources, jsonToProtobuf } from "../../dist/wasm-web/protoruf.js";
await init();                                    // loads & instantiates the .wasm
const desc = compileProtoFromSources({ "shop.proto": PROTO }, "shop.proto");
const wire = jsonToProtobuf(JSON.stringify({ id: "A-1" }), desc, "shop.Order");
```

---

## Notes

- **Binary types.** Node returns/accepts `Buffer`; WASM uses `Uint8Array`. Both are
  byte arrays — keep app code on `Uint8Array` for portability.
- **Memory.** WASM's `DescriptorCache` owns linear-memory state: call `cache.free()`
  (or `using cache = new DescriptorCache(...)`) when done. The Node addon is GC-managed.
- **64-bit integers.** `int64`/`uint64` are emitted as JSON numbers; values above 2^53
  lose precision under `JSON.parse`. See `dev-docs/javascript-typescript.md` §9.
