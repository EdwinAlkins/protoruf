// Node · small message · DescriptorCache (pool decoded once, outside the loop).
// Run: node tests/benchmark/node/benchmark_hot_loop.mjs   (after `npm run build`)
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
const cache = new protoruf.DescriptorCache(descriptor);
const encode = (s) => cache.jsonToProtobuf(s, SMALL_TYPE);
const decode = (b) => cache.protobufToJson(b, SMALL_TYPE, false);
const comparator = await setupProtobufjs(SMALL_PROTO, SMALL_ROOT, SMALL_TYPE);

await runScenario({
  label: "Node · small message · DescriptorCache (decoded once)",
  jsonStr: SMALL_PAYLOAD,
  encode,
  decode,
  comparator,
  iterations: ITERATIONS,
});
