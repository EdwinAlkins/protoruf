// Shared helpers for the WASM benchmarks (wasm-bindgen binding, `nodejs` target).
//
// Mirrors the Python/Node benchmarks: a 2×2 grid of {small, ~1 MB} message ×
// {free functions, DescriptorCache}. Measures the protoruf WASM module and,
// optionally, compares against protobufjs, gated like the Python google.protobuf
// comparison.
//
// Requires a prior build of the WASM `nodejs` target:
//   npm run build:wasm     # wasm-pack build --target nodejs --out-dir dist/wasm
// (The `nodejs` target initialises synchronously — no `await init()` needed.)

import * as protoruf from "../../../dist/wasm/protoruf.js";

export { protoruf };

// ---------------------------------------------------------------------------
// Schemas (compiled in memory — the only option in WASM; no filesystem)
// ---------------------------------------------------------------------------
export const SMALL_PROTO = {
  "message.proto":
    'syntax = "proto3"; package message;' +
    " message Message { string id = 1; string content = 2; int32 priority = 3;" +
    " repeated string tags = 4; Metadata metadata = 5; }" +
    " message Metadata { string author = 1; int64 created_at = 2;" +
    " map<string, string> attributes = 3; }",
};
export const SMALL_ROOT = "message.proto";
export const SMALL_TYPE = "message.Message";

export const SMALL_PAYLOAD = JSON.stringify({
  id: "123",
  content: "Hello World! This is a benchmark.",
  priority: 5,
  tags: ["test", "example", "bench"],
  metadata: {
    author: "Alice",
    created_at: 1234567890,
    attributes: { env: "prod", version: "1.0" },
  },
});

export const LARGE_PROTO = {
  "large.proto":
    'syntax = "proto3"; package bench;' +
    " enum Status { STATUS_UNKNOWN = 0; STATUS_ACTIVE = 1; STATUS_ARCHIVED = 2; }" +
    " message Vector { repeated double values = 1; }" +
    " message Record { string id = 1; string name = 2; Status status = 3; int64 timestamp = 4;" +
    " repeated string tags = 5; map<string, string> attributes = 6; Vector embedding = 7; }" +
    " message Dataset { string dataset_id = 1; string description = 2;" +
    " repeated Record records = 3; map<string, string> metadata = 4; }",
};
export const LARGE_ROOT = "large.proto";
export const LARGE_TYPE = "bench.Dataset";

/** Build a ~`targetBytes` bench.Dataset (mirror of the Python generator). */
export function buildLargeDataset(targetBytes) {
  const makeRecord = (index) => ({
    id: `rec-${String(index).padStart(8, "0")}`,
    name: `Record number ${index}`,
    // STATUS_ACTIVE / STATUS_ARCHIVED — non-zero on purpose (proto3 omits 0).
    status: (index % 2) + 1,
    timestamp: 1_700_000_000 + index,
    tags: [`tag-${index % 50}`, `group-${index % 10}`],
    attributes: { region: "eu-west", tier: String(index % 5), note: "x".repeat(16) },
    embedding: {
      values: Array.from({ length: 8 }, (_, k) => Math.round(((index % 7) * 0.125 + k) * 1e4) / 1e4),
    },
  });

  const records = [];
  const obj = {
    dataset_id: "ds-benchmark-1",
    description: "large benchmark dataset",
    records,
    metadata: { source: "benchmark", version: "1.0", env: "bench" },
  };

  const batchSize = 200;
  let index = 0;
  for (let n = 0; n < batchSize; n++) {
    records.push(makeRecord(index));
    index++;
  }

  let currentSize = JSON.stringify(obj).length;
  if (currentSize < targetBytes) {
    const batchJson = JSON.stringify(records.slice(-batchSize));
    const bytesPerRecord = batchJson.length / batchSize;
    const extraRecords = Math.floor((targetBytes - currentSize) / bytesPerRecord);
    for (let n = 0; n < extraRecords; n++) {
      records.push(makeRecord(index));
      index++;
    }
  }

  while (JSON.stringify(obj).length < targetBytes) {
    for (let n = 0; n < Math.min(batchSize, 50); n++) {
      records.push(makeRecord(index));
      index++;
    }
  }
  return obj;
}

// ---------------------------------------------------------------------------
// Optional comparison: protobufjs (skipped gracefully if unavailable)
// ---------------------------------------------------------------------------
export async function setupProtobufjs(files, root, messageType) {
  try {
    const protobuf = (await import("protobufjs")).default;
    // Parse the .proto source with protobufjs's own parser (it handles maps
    // natively, unlike Root.fromDescriptor on a foreign descriptor). `keepCase`
    // keeps snake_case field names, matching protoruf's JSON output.
    const parsed = protobuf.parse(files[root], { keepCase: true });
    const Type = parsed.root.lookupType(messageType);
    const toObj = { longs: Number, enums: Number, bytes: String };
    return {
      name: "protobufjs",
      encode: (jsonStr) => Type.encode(Type.fromObject(JSON.parse(jsonStr))).finish(),
      decode: (bytes) => JSON.stringify(Type.toObject(Type.decode(bytes), toObj)),
    };
  } catch (e) {
    console.log(`⚠️  protobufjs comparison unavailable (${e.message}); measuring protoruf only.`);
    return null;
  }
}

// ---------------------------------------------------------------------------
// Scenario runner: correctness check + timed write/read loops + table
// ---------------------------------------------------------------------------
function timeLoop(iterations, fn) {
  const start = performance.now();
  for (let n = 0; n < iterations; n++) fn();
  return (performance.now() - start) / 1000;
}

/**
 * @param {object}   o
 * @param {string}   o.label
 * @param {string}   o.jsonStr     the message as a JSON string
 * @param {Function} o.encode      (jsonStr) => bytes  (Uint8Array)
 * @param {Function} o.decode      (bytes)   => jsonStr
 * @param {?object}  o.comparator  optional { name, encode, decode }
 * @param {number}   o.iterations
 */
export async function runScenario({ label, jsonStr, encode, decode, comparator, iterations }) {
  const jsonMB = Buffer.byteLength(jsonStr) / 1_000_000;
  const protoBytes = encode(jsonStr);
  const protoMB = Buffer.from(protoBytes).length / 1_000_000;

  // Schema-agnostic correctness: a JSON -> proto -> JSON -> proto -> JSON round-trip
  // must preserve the data. Compared with sorted keys so that map field ordering
  // (which has no canonical protobuf wire order) is ignored.
  const canon = (v) =>
    Array.isArray(v)
      ? v.map(canon)
      : v && typeof v === "object"
        ? Object.fromEntries(Object.keys(v).sort().map((k) => [k, canon(v[k])]))
        : v;
  const decoded = decode(protoBytes);
  const recoded = decode(encode(decoded));
  if (JSON.stringify(canon(JSON.parse(decoded))) !== JSON.stringify(canon(JSON.parse(recoded)))) {
    throw new Error("round-trip did not preserve the message");
  }

  const mbps = (sec) => (jsonMB * iterations) / sec;
  const fmt = (n) => Math.round(n).toLocaleString();

  console.log(`\n=== ${label} ===`);
  console.log(
    `Message: JSON ${jsonMB.toFixed(3)} MB  Protobuf ${protoMB.toFixed(3)} MB` +
      `   (${iterations.toLocaleString()} iterations)`,
  );
  console.log("✅ round-trip stable\n");

  const pw = timeLoop(iterations, () => encode(jsonStr));
  console.log(`protoruf  write: ${pw.toFixed(4)}s  ${((pw / iterations) * 1000).toFixed(4)} ms/msg  ${fmt(mbps(pw))} MB/s`);
  const pr = timeLoop(iterations, () => decode(protoBytes));
  console.log(`protoruf  read : ${pr.toFixed(4)}s  ${((pr / iterations) * 1000).toFixed(4)} ms/msg  ${fmt(mbps(pr))} MB/s`);

  if (comparator) {
    // r = comparator_time / protoruf_time  (>1 => protoruf is faster)
    const speed = (cmpTime, pTime) => {
      const r = cmpTime / pTime;
      return r >= 1 ? `protoruf ${r.toFixed(1)}× faster` : `${comparator.name} ${(1 / r).toFixed(1)}× faster`;
    };
    const cBytes = comparator.encode(jsonStr);
    const cw = timeLoop(iterations, () => comparator.encode(jsonStr));
    const cr = timeLoop(iterations, () => comparator.decode(cBytes));
    console.log(`\n${comparator.name} write: ${cw.toFixed(4)}s  ${fmt(mbps(cw))} MB/s   (${speed(cw, pw)})`);
    console.log(`${comparator.name} read : ${cr.toFixed(4)}s  ${fmt(mbps(cr))} MB/s   (${speed(cr, pr)})`);
  }
}
