// WASM · small message · free functions (descriptor re-decoded on every call).
// Run: node --expose-gc tests/benchmark/wasm/benchmark.mjs   (after `npm run build:wasm`)
import {
  protoruf,
  SMALL_PROTO,
  SMALL_ROOT,
  SMALL_TYPE,
  SMALL_PAYLOAD,
  setupProtobufjs,
  runScenario,
} from "./common.mjs";

const ITERATIONS = 10_000;
const WARMUP_ITERATIONS = 1_000;
const MEASURED_RUNS = 20;

const descriptor = protoruf.compileProtoFromSources(SMALL_PROTO, SMALL_ROOT);
const encode = (s) => protoruf.jsonToProtobuf(s, descriptor, SMALL_TYPE);
const decode = (b) => protoruf.protobufToJson(b, descriptor, false, SMALL_TYPE);
const comparator = await setupProtobufjs(SMALL_PROTO, SMALL_ROOT, SMALL_TYPE);

await runScenario({
  runtimeLabel: "protoruf (WASM)",
  title: `Small-message results — free functions (WASM, ${ITERATIONS.toLocaleString()} conversions per run)`,
  jsonStr: SMALL_PAYLOAD,
  encode,
  decode,
  comparator,
  iterations: ITERATIONS,
  warmupIterations: WARMUP_ITERATIONS,
  measuredRuns: MEASURED_RUNS,
  showMbS: false,
  protorufLabel: "protoruf (free)",
});
