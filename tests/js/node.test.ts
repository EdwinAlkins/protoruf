// Node.js native addon (napi) — run the shared conversion corpus.
// Requires a prior build: `npm run build:debug` (-> dist/index.js).
import * as nodeBinding from "../../dist/index.js";
import { runConversionSuite, type ProtorufApi } from "./conversion.shared";

runConversionSuite(nodeBinding as unknown as ProtorufApi, "node");
