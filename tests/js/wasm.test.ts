// WebAssembly binding (wasm-pack `nodejs` target) — run the shared corpus.
// Requires a prior build: `npm run build:wasm` (-> dist/wasm/protoruf.js).
import * as wasmBinding from "../../dist/wasm/protoruf.js";
import { runConversionSuite, type ProtorufApi } from "./conversion.shared";

runConversionSuite(wasmBinding as unknown as ProtorufApi, "wasm");
