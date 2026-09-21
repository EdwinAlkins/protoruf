// Cross-binding parity: the Node and WASM bindings wrap the same Rust core, so
// for identical input they must produce byte-for-byte identical protobuf output.
import { describe, test, expect } from "vitest";
import * as node from "../../dist/index.js";
import * as wasm from "../../dist/wasm/protoruf.js";
import { PROTO } from "./conversion.shared";

describe("node/wasm byte-for-byte parity", () => {
  const files = { "t.proto": PROTO };
  const json = '{"id":"x","count":3,"big":42,"tags":["a","b"],"color":"BLUE","attrs":{"k":"v"},"inner":{"note":"n"}}';

  test("compileProtoFromSources yields identical descriptors", () => {
    const dn = Buffer.from(node.compileProtoFromSources(files, "t.proto"));
    const dw = Buffer.from(wasm.compileProtoFromSources(files, "t.proto"));
    expect(dn.equals(dw)).toBe(true);
  });

  test("jsonToProtobuf yields identical wire bytes", () => {
    const dn = node.compileProtoFromSources(files, "t.proto");
    const dw = wasm.compileProtoFromSources(files, "t.proto");
    const bn = Buffer.from(node.jsonToProtobuf(json, dn, "t.All"));
    const bw = Buffer.from(wasm.jsonToProtobuf(json, dw, "t.All"));
    expect(bn.equals(bw)).toBe(true);
  });
});
