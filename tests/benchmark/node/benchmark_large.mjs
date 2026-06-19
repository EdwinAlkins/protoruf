// Node · ~1 MB message · free functions (descriptor re-decoded on every call).
// Run: node tests/benchmark/node/benchmark_large.mjs   (after `npm run build`)
import {
  protoruf,
  LARGE_PROTO,
  LARGE_ROOT,
  LARGE_TYPE,
  buildLargeDataset,
  setupProtobufjs,
  runScenario,
} from "./common.mjs";

const ITERATIONS = 200;

const descriptor = protoruf.compileProtoFromSources(LARGE_PROTO, LARGE_ROOT);
const jsonStr = JSON.stringify(buildLargeDataset(1_000_000));
const encode = (s) => protoruf.jsonToProtobuf(s, descriptor, LARGE_TYPE);
const decode = (b) => protoruf.protobufToJson(b, descriptor, false, LARGE_TYPE);
const comparator = await setupProtobufjs(LARGE_PROTO, LARGE_ROOT, LARGE_TYPE);

await runScenario({
  label: "Node · ~1 MB message · free functions (decode per call)",
  jsonStr,
  encode,
  decode,
  comparator,
  iterations: ITERATIONS,
});
