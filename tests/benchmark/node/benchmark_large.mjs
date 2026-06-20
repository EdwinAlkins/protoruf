// Node · large message · free functions (descriptor re-decoded on every call).
// Run: node --expose-gc tests/benchmark/node/benchmark_large.mjs   (after `npm run build`)
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
const jsonStr = JSON.stringify(buildLargeDataset(N_RECORDS));
const encode = (s) => protoruf.jsonToProtobuf(s, descriptor, LARGE_TYPE);
const decode = (b) => protoruf.protobufToJson(b, descriptor, false, LARGE_TYPE);
const comparator = await setupProtobufjs(LARGE_PROTO, LARGE_ROOT, LARGE_TYPE);

await runScenario({
  runtimeLabel: "protoruf (Node addon)",
  title: `Large-message results — free functions (Node, ${N_RECORDS.toLocaleString()} records, ${ITERATIONS.toLocaleString()} conversions per run)`,
  jsonStr,
  encode,
  decode,
  comparator,
  iterations: ITERATIONS,
  warmupIterations: WARMUP_ITERATIONS,
  measuredRuns: MEASURED_RUNS,
  showMbS: true,
  protorufLabel: "protoruf (free)",
});
