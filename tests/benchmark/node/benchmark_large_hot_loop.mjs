// Node · ~1 MB message · DescriptorCache (pool decoded once, outside the loop).
// Run: node tests/benchmark/node/benchmark_large_hot_loop.mjs   (after `npm run build`)
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
const cache = new protoruf.DescriptorCache(descriptor);
const jsonStr = JSON.stringify(buildLargeDataset(1_000_000));
const encode = (s) => cache.jsonToProtobuf(s, LARGE_TYPE);
const decode = (b) => cache.protobufToJson(b, LARGE_TYPE, false);
const comparator = await setupProtobufjs(LARGE_PROTO, LARGE_ROOT, LARGE_TYPE);

await runScenario({
  label: "Node · ~1 MB message · DescriptorCache (decoded once)",
  jsonStr,
  encode,
  decode,
  comparator,
  iterations: ITERATIONS,
});
