// WASM · large message · DescriptorCache (pool decoded once, outside the loop).
// Run: node --expose-gc tests/benchmark/wasm/benchmark_large_hot_loop.mjs   (after `npm run build:wasm`)
import {
  protoruf,
  LARGE_PROTO,
  LARGE_ROOT,
  LARGE_TYPE,
  buildLargeDataset,
  setupProtobufjs,
  runScenario,
} from "./common.mjs";

const N_RECORDS = 5_000;
const ITERATIONS = 200;
const WARMUP_ITERATIONS = 10;
const MEASURED_RUNS = 10;

const descriptor = protoruf.compileProtoFromSources(LARGE_PROTO, LARGE_ROOT);
const cache = new protoruf.DescriptorCache(descriptor);
const jsonStr = JSON.stringify(buildLargeDataset(N_RECORDS));
const encode = (s) => cache.jsonToProtobuf(s, LARGE_TYPE);
const decode = (b) => cache.protobufToJson(b, LARGE_TYPE, false);
const comparator = await setupProtobufjs(LARGE_PROTO, LARGE_ROOT, LARGE_TYPE);

await runScenario({
  runtimeLabel: "protoruf (WASM)",
  title: `Large-message results — DescriptorCache (WASM, ${N_RECORDS.toLocaleString()} records, ${ITERATIONS.toLocaleString()} conversions per run)`,
  jsonStr,
  encode,
  decode,
  comparator,
  iterations: ITERATIONS,
  warmupIterations: WARMUP_ITERATIONS,
  measuredRuns: MEASURED_RUNS,
  showMbS: true,
  protorufLabel: "protoruf (cache)",
});

cache.free(); // WASM objects own linear-memory state — release it
