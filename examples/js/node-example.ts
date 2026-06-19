/**
 * protoruf — Node.js (native addon, napi-rs) example.
 *
 * Build first, from the repo root:
 *   npm install
 *   npm run build:debug        # generates ../../dist/{index.js, index.d.ts, *.node}
 *
 * Run (Node >= 23 executes .ts directly via type-stripping):
 *   node examples/js/node-example.ts
 *   # older Node:  npx tsx examples/js/node-example.ts
 */

import {
  compileProtoFromSources,
  jsonToProtobuf,
  protobufToJson,
  DescriptorCache,
} from "../../dist/index.js";

// A small schema, compiled entirely in memory (no .proto file on disk).
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

// 1. Compile the schema -> descriptor set (bytes).
const descriptor = compileProtoFromSources({ "shop.proto": PROTO }, "shop.proto");
console.log(`compiled descriptor: ${descriptor.length} bytes`);

// 2. JSON -> Protobuf.
const order = {
  id: "A-1001",
  status: 1, // PAID — enums are encoded by number
  items: [
    { sku: "BOOK-42", qty: 2 },
    { sku: "PEN-07", qty: 5 },
  ],
  labels: { gift: "yes", priority: "high" },
};
const wire = jsonToProtobuf(JSON.stringify(order), descriptor, "shop.Order");
console.log(`protobuf wire size: ${wire.length} bytes`);

// 3. Protobuf -> JSON (round-trip, pretty-printed).
const back = JSON.parse(protobufToJson(wire, descriptor, true, "shop.Order"));
console.log("round-trip:", back);

// 4. High-throughput pattern: decode the pool once, reuse it.
const cache = new DescriptorCache(descriptor);
for (let i = 0; i < 3; i++) {
  const pb = cache.jsonToProtobuf(JSON.stringify({ id: `A-${i}` }), "shop.Order");
  console.log(`cached #${i}:`, cache.protobufToJson(pb, "shop.Order", false));
}

// Node-only bonus: `compileProto("schema.proto")` can also read a .proto from disk
// (not available in the browser/WASM build).

console.log("✔ node example done");
