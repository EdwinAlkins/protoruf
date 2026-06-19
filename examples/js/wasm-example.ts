/**
 * protoruf — WebAssembly example.
 *
 * Runnable in Node via the `nodejs` wasm-pack target (synchronous init, no fetch).
 * Build first, from the repo root:
 *   rustup target add wasm32-unknown-unknown
 *   cargo install wasm-pack                                  # once
 *   wasm-pack build --target nodejs --out-dir dist/wasm -- --features wasm
 *
 * Run:
 *   node examples/js/wasm-example.ts
 *   # older Node:  npx tsx examples/js/wasm-example.ts
 *
 * For the BROWSER, rebuild with `--target web` and `await init()` before any
 * call (see ./README.md). Note: `compileProto(path)` is intentionally NOT
 * exposed in WASM (no filesystem) — always compile from in-memory sources.
 */

import {
  compileProtoFromSources,
  jsonToProtobuf,
  protobufToJson,
  DescriptorCache,
} from "../../dist/wasm/protoruf.js";

const PROTO = `
syntax = "proto3";
package shop;

enum Status { PENDING = 0; PAID = 1; SHIPPED = 2; }

message Item { string sku = 1; int32 qty = 2; }

message Order {
  string id = 1;
  Status status = 2;
  repeated Item items = 3;
  map<string, string> labels = 4;
}
`;

// 1. Compile from in-memory sources (the only option in WASM).
const descriptor = compileProtoFromSources({ "shop.proto": PROTO }, "shop.proto");
console.log(`compiled descriptor: ${descriptor.length} bytes`);

// 2. JSON -> Protobuf.
const order = {
  id: "A-1001",
  status: 2, // SHIPPED
  items: [{ sku: "BOOK-42", qty: 2 }],
  labels: { gift: "no" },
};
const wire = jsonToProtobuf(JSON.stringify(order), descriptor, "shop.Order");
console.log(`protobuf wire size: ${wire.length} bytes`);

// 3. Protobuf -> JSON (round-trip).
const back = JSON.parse(protobufToJson(wire, descriptor, true, "shop.Order"));
console.log("round-trip:", back);

// 4. Reusable pool. WASM objects own native (linear-memory) state, so free it
//    explicitly when done (or use `using cache = new DescriptorCache(...)`).
const cache = new DescriptorCache(descriptor);
const pb = cache.jsonToProtobuf(JSON.stringify({ id: "A-007" }), "shop.Order");
console.log("cached:", cache.protobufToJson(pb, "shop.Order", false));
cache.free();

console.log("✔ wasm example done");
