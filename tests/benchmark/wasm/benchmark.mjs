// WASM · small message · free functions (descriptor re-decoded on every call).
// Run: node tests/benchmark/wasm/benchmark.mjs   (after `npm run build:wasm`)
import {
  protoruf,
  SMALL_PROTO,
  SMALL_ROOT,
  SMALL_TYPE,
  SMALL_PAYLOAD,
  setupProtobufjs,
  runScenario,
} from "./common.mjs";

const ITERATIONS = 50_000;

const descriptor = protoruf.compileProtoFromSources(SMALL_PROTO, SMALL_ROOT);
const encode = (s) => protoruf.jsonToProtobuf(s, descriptor, SMALL_TYPE);
const decode = (b) => protoruf.protobufToJson(b, descriptor, false, SMALL_TYPE);
const comparator = await setupProtobufjs(SMALL_PROTO, SMALL_ROOT, SMALL_TYPE);

await runScenario({
  label: "WASM · small message · free functions (decode per call)",
  jsonStr: SMALL_PAYLOAD,
  encode,
  decode,
  comparator,
  iterations: ITERATIONS,
});
