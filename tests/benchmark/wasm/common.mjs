// Shared helpers for the WASM benchmarks (wasm-bindgen binding, `nodejs` target).
//
// Mirrors the Python benchmarks (and their `benchmark_utils.py` methodology):
// a 2×2 grid of {small, large} message × {free functions, DescriptorCache}.
// Measures the protoruf WASM module and, optionally, compares against protobufjs
// (the JS reference for dynamic JSON ↔ Protobuf), gated like the Python
// google.protobuf comparison.
//
// Methodology (applied by every benchmark script, symmetric with Python):
//   - warmup iterations before any timed run (JIT warmup matters a lot in V8)
//   - multiple measured runs; report median, p95, and stddev
//   - global.gc() invoked between runs when started with `node --expose-gc`
//   - msg/s and MB/s (when the payload is large enough to be meaningful)
//   - RSS and heap deltas sampled on a dedicated run
//   - system info printed at startup (CPU, OS, Node, Rust, protobufjs)
//   - fixed, reproducible dataset (record count, not a variable byte target)
//
// Requires a prior build of the WASM `nodejs` target:
//   npm run build:wasm     # wasm-pack build --target nodejs --out-dir dist/wasm
// (The `nodejs` target initialises synchronously — no `await init()` needed.)

import os from "node:os";
import { execSync } from "node:child_process";
import { createRequire } from "node:module";

import * as protoruf from "../../../dist/wasm/protoruf.js";

export { protoruf };

const require = createRequire(import.meta.url);

// Default timing parameters (overridden per benchmark where needed).
export const DEFAULT_WARMUP_ITERATIONS = 1_000;
export const DEFAULT_MEASURED_RUNS = 20;

// ---------------------------------------------------------------------------
// Schemas (compiled in memory, symmetric with the WASM benchmarks)
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

// Mirror of the Python `make_record` — deterministic, byte-for-byte equivalent.
function makeRecord(index) {
  return {
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
  };
}

/**
 * Build a deterministic `bench.Dataset` with exactly `nRecords` records.
 * Fixed record count (not a variable byte target) for reproducibility — mirror
 * of the Python `build_large_dataset`.
 */
export function buildLargeDataset(nRecords) {
  return {
    dataset_id: "ds-benchmark-1",
    description: "large benchmark dataset",
    records: Array.from({ length: nRecords }, (_, i) => makeRecord(i)),
    metadata: { source: "benchmark", version: "1.0", env: "bench" },
  };
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
    let version = "unknown";
    try {
      version = require("protobufjs/package.json").version;
    } catch {
      /* keep "unknown" */
    }
    return {
      name: "protobufjs",
      version,
      encode: (jsonStr) => Type.encode(Type.fromObject(JSON.parse(jsonStr))).finish(),
      decode: (bytes) => JSON.stringify(Type.toObject(Type.decode(bytes), toObj)),
    };
  } catch (e) {
    console.log(`⚠️  protobufjs comparison unavailable (${e.message}); measuring protoruf only.`);
    return null;
  }
}

// ---------------------------------------------------------------------------
// Stats helpers
// ---------------------------------------------------------------------------
function percentile(values, pct) {
  if (values.length === 0) throw new Error("percentile() requires at least one value");
  if (values.length === 1) return values[0];
  const ordered = [...values].sort((a, b) => a - b);
  const rank = ((ordered.length - 1) * pct) / 100;
  const lower = Math.floor(rank);
  const upper = Math.min(lower + 1, ordered.length - 1);
  const weight = rank - lower;
  return ordered[lower] + (ordered[upper] - ordered[lower]) * weight;
}

function median(values) {
  const ordered = [...values].sort((a, b) => a - b);
  const mid = Math.floor(ordered.length / 2);
  return ordered.length % 2 ? ordered[mid] : (ordered[mid - 1] + ordered[mid]) / 2;
}

function stddev(values) {
  if (values.length < 2) return 0;
  const mean = values.reduce((a, b) => a + b, 0) / values.length;
  const variance = values.reduce((a, b) => a + (b - mean) ** 2, 0) / (values.length - 1);
  return Math.sqrt(variance);
}

const fmtInt = (n) => Math.round(n).toLocaleString("en-US");

function formatBytes(numBytes) {
  if (numBytes >= 1_000_000) return `${(numBytes / 1_000_000).toFixed(1)} MB`;
  if (numBytes >= 1_000) return `${(numBytes / 1_000).toFixed(1)} KB`;
  return `${numBytes} B`;
}

// ---------------------------------------------------------------------------
// GC control (best effort — only available with `node --expose-gc`)
// ---------------------------------------------------------------------------
const HAS_GC = typeof globalThis.gc === "function";
function maybeGc() {
  if (HAS_GC) globalThis.gc();
}

// ---------------------------------------------------------------------------
// System info + methodology
// ---------------------------------------------------------------------------
function getRustcVersion() {
  try {
    return execSync("rustc --version", { stdio: ["ignore", "pipe", "ignore"] }).toString().trim();
  } catch {
    return "not found";
  }
}

export function printSystemInfo(runtimeLabel, protobufjsVersion) {
  const cpu = os.cpus()?.[0]?.model ?? "unknown";
  console.log("System");
  console.log(`  CPU:        ${cpu} (${os.arch()})`);
  console.log(`  OS:         ${os.type()} ${os.release()}`);
  console.log(`  Runtime:    ${runtimeLabel} (node ${process.version})`);
  console.log(`  Rust:       ${getRustcVersion()}`);
  console.log(`  protobufjs: ${protobufjsVersion ?? "not installed"}`);
  console.log();
}

export function printMethodology({ warmup, runs, iterations }) {
  console.log("Methodology");
  console.log("  - release build of protoruf (see README)");
  console.log(`  - ${fmtInt(warmup)} warmup iteration(s) per scenario`);
  console.log(`  - ${runs} measured run(s); median, p95, and stddev reported`);
  console.log(
    HAS_GC
      ? "  - global.gc() invoked before each measured run (node --expose-gc)"
      : "  - GC not controlled (re-run with `node --expose-gc` for GC-controlled measurements)",
  );
  console.log(`  - each measured run executes ${fmtInt(iterations)} conversion(s)`);
  console.log(
    "  - measures the full JSON ↔ Protobuf conversion stack " +
      "(JSON parse + encode/decode + JSON emit), not protobuf encode alone",
  );
  console.log();
}

// ---------------------------------------------------------------------------
// Timed harness: warmup, multiple measured runs, memory sampling
// ---------------------------------------------------------------------------
function runLoop(iterations, fn) {
  for (let n = 0; n < iterations; n++) fn();
}

function measureMemory(iterations, fn) {
  maybeGc();
  const before = process.memoryUsage();
  runLoop(iterations, fn);
  const after = process.memoryUsage();
  maybeGc();
  return {
    rssDeltaBytes: Math.max(after.rss - before.rss, 0),
    heapDeltaBytes: Math.max(after.heapUsed - before.heapUsed, 0),
  };
}

/**
 * Warm up, then time `fn` (one conversion per call) over several runs.
 * Prints live progress and returns aggregated stats.
 */
export function runTimedBenchmark(fn, { iterations, payloadBytes, warmupIterations, measuredRuns, label, showMbS }) {
  const prefix = `  [${label}]`;

  process.stdout.write(`${prefix} warmup (${fmtInt(warmupIterations)} pass(es))...`);
  const warmupStart = performance.now();
  runLoop(warmupIterations, fn);
  console.log(` done (${((performance.now() - warmupStart) / 1000).toFixed(2)}s)`);

  const runs = [];
  for (let runIdx = 1; runIdx <= measuredRuns; runIdx++) {
    maybeGc();
    const start = performance.now();
    runLoop(iterations, fn);
    const elapsed = (performance.now() - start) / 1000;
    runs.push(elapsed);
    const instantMsgS = iterations / elapsed;
    let line = `${prefix} run ${String(runIdx).padStart(2)}/${measuredRuns} — ${elapsed.toFixed(4)}s (${fmtInt(instantMsgS)} msg/s)`;
    if (showMbS) line += `   ${fmtInt((payloadBytes / 1_000_000) * instantMsgS)} MB/s`;
    console.log(line);
  }

  const medianS = median(runs);
  const p95S = percentile(runs, 95);
  const stddevS = stddev(runs);
  const msgPerS = iterations / medianS;
  const mbPerS = (payloadBytes / 1_000_000) * msgPerS;

  process.stdout.write(`${prefix} measuring memory...`);
  const { rssDeltaBytes, heapDeltaBytes } = measureMemory(iterations, fn);
  console.log(" done");

  return { medianS, p95S, stddevS, msgPerS, mbPerS, rssDeltaBytes, heapDeltaBytes };
}

// ---------------------------------------------------------------------------
// Reporting
// ---------------------------------------------------------------------------
function statsParts(stats, showMbS) {
  const parts = [
    `median ${stats.medianS.toFixed(4)}s`,
    `p95 ${stats.p95S.toFixed(4)}s`,
    `σ ${stats.stddevS.toFixed(4)}s`,
    `${fmtInt(stats.msgPerS)} msg/s`,
  ];
  if (showMbS) parts.push(`${fmtInt(stats.mbPerS)} MB/s`);
  parts.push(`RSS Δ ${formatBytes(stats.rssDeltaBytes)}`);
  parts.push(`heap Δ ${formatBytes(stats.heapDeltaBytes)}`);
  return parts;
}

function printComparisonTable(title, rows, { showMbS, protorufLabel, comparatorName }) {
  const bar = "=".repeat(90);
  console.log(`\n${bar}`);
  console.log(title);
  console.log(bar);

  for (const { label, comparatorStats, protorufStats } of rows) {
    console.log(`\n${label}`);
    if (comparatorStats) {
      console.log(`  ${comparatorName}: ${statsParts(comparatorStats, showMbS).join("   ")}`);
      console.log(`  ${protorufLabel}: ${statsParts(protorufStats, showMbS).join("   ")}`);
      const ratio = comparatorStats.medianS / protorufStats.medianS;
      const verdict =
        ratio >= 1 ? `protoruf ${ratio.toFixed(1)}× faster` : `${comparatorName} ${(1 / ratio).toFixed(1)}× faster`;
      console.log(`  speedup (median): ${verdict} on the JSON ↔ Protobuf conversion stack`);
    } else {
      console.log(`  → ${statsParts(protorufStats, showMbS).join("   ")}`);
    }
  }

  console.log(
    "\nNote: speedup compares the full JSON ↔ Protobuf conversion path " +
      "(including JSON parsing/formatting), not raw protobuf encoding alone.",
  );
}

// ---------------------------------------------------------------------------
// Scenario orchestrator (system info + methodology + correctness + table)
// ---------------------------------------------------------------------------
/**
 * @param {object}   o
 * @param {string}   o.runtimeLabel  e.g. "protoruf (Node addon)"
 * @param {string}   o.title         table title
 * @param {string}   o.jsonStr       the message as a JSON string
 * @param {Function} o.encode        (jsonStr) => bytes  (Buffer/Uint8Array)
 * @param {Function} o.decode        (bytes)   => jsonStr
 * @param {?object}  o.comparator    optional { name, version, encode, decode }
 * @param {number}   o.iterations
 * @param {number}   [o.warmupIterations]
 * @param {number}   [o.measuredRuns]
 * @param {boolean}  o.showMbS        show MB/s (true for large messages)
 * @param {string}   o.protorufLabel  e.g. "protoruf (free)" / "protoruf (cache)"
 */
export async function runScenario({
  runtimeLabel,
  title,
  jsonStr,
  encode,
  decode,
  comparator,
  iterations,
  warmupIterations = DEFAULT_WARMUP_ITERATIONS,
  measuredRuns = DEFAULT_MEASURED_RUNS,
  showMbS,
  protorufLabel,
}) {
  printSystemInfo(runtimeLabel, comparator?.version);
  printMethodology({ warmup: warmupIterations, runs: measuredRuns, iterations });

  const jsonMB = Buffer.byteLength(jsonStr) / 1_000_000;
  const payloadBytes = Buffer.byteLength(jsonStr);
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

  console.log(`Message size:  JSON = ${jsonMB.toFixed(3)} MB   Protobuf = ${protoMB.toFixed(3)} MB`);
  console.log("✅ round-trip correctness verified\n");

  console.log(`protoruf write (${measuredRuns} runs × ${fmtInt(iterations)} iterations)...`);
  const protorufWrite = runTimedBenchmark(() => encode(jsonStr), {
    iterations,
    payloadBytes,
    warmupIterations,
    measuredRuns,
    label: "protoruf write",
    showMbS,
  });

  console.log(`\nprotoruf read (${measuredRuns} runs × ${fmtInt(iterations)} iterations)...`);
  const protorufRead = runTimedBenchmark(() => decode(protoBytes), {
    iterations,
    payloadBytes,
    warmupIterations,
    measuredRuns,
    label: "protoruf read",
    showMbS,
  });

  let comparatorWrite = null;
  let comparatorRead = null;
  if (comparator) {
    const cBytes = comparator.encode(jsonStr);
    console.log(`\n${comparator.name} write (${measuredRuns} runs × ${fmtInt(iterations)} iterations)...`);
    comparatorWrite = runTimedBenchmark(() => comparator.encode(jsonStr), {
      iterations,
      payloadBytes,
      warmupIterations,
      measuredRuns,
      label: `${comparator.name} write`,
      showMbS,
    });
    console.log(`\n${comparator.name} read (${measuredRuns} runs × ${fmtInt(iterations)} iterations)...`);
    comparatorRead = runTimedBenchmark(() => comparator.decode(cBytes), {
      iterations,
      payloadBytes,
      warmupIterations,
      measuredRuns,
      label: `${comparator.name} read`,
      showMbS,
    });
  }

  printComparisonTable(
    title,
    [
      { label: "Serialization (JSON→Proto)", comparatorStats: comparatorWrite, protorufStats: protorufWrite },
      { label: "Parsing (Proto→JSON)", comparatorStats: comparatorRead, protorufStats: protorufRead },
    ],
    { showMbS, protorufLabel, comparatorName: comparator?.name ?? "protobufjs" },
  );
}
