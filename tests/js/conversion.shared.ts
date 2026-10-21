/**
 * Shared conversion corpus, run against every binding (Node napi + WASM).
 *
 * Mirrors the parity cases of the Rust core (`src/core.rs`) and the Python suite:
 * round-trip, maps, enums (name <-> number), oneof, repeated, default values,
 * int64 precision, invalid input, unknown message type, in-memory compilation,
 * and the DescriptorCache fast path.
 */
import { describe, test, expect } from "vitest";

export interface ProtorufCache {
  jsonToProtobuf(json: string, messageType: string): Uint8Array;
  protobufToJson(bytes: Uint8Array, messageType: string, pretty?: boolean): string;
  /** WASM objects own linear-memory state and expose `free()`; napi does not. */
  free?(): void;
}

export interface ProtorufApi {
  compileProtoFromSources(files: Record<string, string>, root: string): Uint8Array;
  jsonToProtobuf(json: string, descriptor: Uint8Array, messageType: string): Uint8Array;
  protobufToJson(
    bytes: Uint8Array,
    descriptor: Uint8Array,
    messageType: string,
    pretty: boolean,
  ): string;
  DescriptorCache: new (descriptor: Uint8Array) => ProtorufCache;
}

export const PROTO = `
syntax = "proto3";
package t;

enum Color { RED = 0; GREEN = 1; BLUE = 2; }

message Inner { string note = 1; }

message All {
  string id = 1;
  int32 count = 2;
  int64 big = 3;
  bool flag = 4;
  repeated string tags = 5;
  map<string, string> attrs = 6;
  Color color = 7;
  Inner inner = 8;
  oneof choice {
    string text = 9;
    int32 number = 10;
  }
}
`;

export function runConversionSuite(api: ProtorufApi, label: string): void {
  describe(`protoruf conversion parity [${label}]`, () => {
    const descriptor = api.compileProtoFromSources({ "t.proto": PROTO }, "t.proto");

    const encode = (obj: unknown) =>
      api.jsonToProtobuf(JSON.stringify(obj), descriptor, "t.All");
    const decode = (bytes: Uint8Array) =>
      JSON.parse(api.protobufToJson(bytes, descriptor, "t.All", false));

    test("compiles a non-empty descriptor", () => {
      expect(descriptor.length).toBeGreaterThan(0);
    });

    test("round-trips scalars, repeated and nested fields", () => {
      const j = decode(encode({ id: "123", count: 5, tags: ["a", "b"], inner: { note: "hi" } }));
      expect(j.id).toBe("123");
      expect(j.count).toBe(5);
      expect(j.tags).toEqual(["a", "b"]);
      expect(j.inner.note).toBe("hi");
    });

    test("map fields survive the round-trip", () => {
      const j = decode(encode({ id: "m", attrs: { env: "prod", v: "1" } }));
      expect(j.attrs).toEqual({ env: "prod", v: "1" });
    });

    test("enums: accepts name or number on input, emits number on output", () => {
      expect(decode(encode({ id: "e", color: "BLUE" })).color).toBe(2);
      expect(decode(encode({ id: "e", color: 2 })).color).toBe(2);
    });

    test("oneof: only the set arm is present", () => {
      const jt = decode(encode({ id: "o", text: "hello" }));
      expect(jt.text).toBe("hello");
      expect(jt.number).toBeUndefined();

      const jn = decode(encode({ id: "o", number: 7 }));
      expect(jn.number).toBe(7);
      expect(jn.text).toBeUndefined();
    });

    test("proto3 default values are omitted", () => {
      const j = decode(encode({ id: "d" }));
      expect(j.id).toBe("d");
      expect(j.count).toBeUndefined();
      expect(j.color).toBeUndefined(); // RED = 0
    });

    test("pretty printing adds newlines", () => {
      const pretty = api.protobufToJson(encode({ id: "p" }), descriptor, "t.All", true);
      expect(pretty).toContain("\n");
    });

    test("int64: Rust keeps full precision in JSON text; JS JSON.parse loses it >2^53", () => {
      const exact = "9223372036854775807"; // i64 max
      const bytes = api.jsonToProtobuf(`{"id":"big","big":${exact}}`, descriptor, "t.All");
      const text = api.protobufToJson(bytes, descriptor, "t.All", false);
      expect(text).toContain(exact); // exact on the Rust / JSON-text side
      expect(String(JSON.parse(text).big)).not.toBe(exact); // lost by JS number
    });

    test("invalid JSON throws", () => {
      expect(() => api.jsonToProtobuf("not json", descriptor, "t.All")).toThrow();
    });

    test("unknown message type throws", () => {
      expect(() => api.jsonToProtobuf('{"id":"x"}', descriptor, "t.Nope")).toThrow();
    });

    test("DescriptorCache matches the free functions and is reusable", () => {
      const cache = new api.DescriptorCache(descriptor);
      try {
        const viaCache = Buffer.from(cache.jsonToProtobuf(JSON.stringify({ id: "c", count: 1 }), "t.All"));
        const viaFree = Buffer.from(encode({ id: "c", count: 1 }));
        expect(viaCache.equals(viaFree)).toBe(true);

        for (let i = 0; i < 5; i++) {
          const bytes = cache.jsonToProtobuf(JSON.stringify({ id: `c${i}` }), "t.All");
          expect(JSON.parse(cache.protobufToJson(bytes, "t.All", false)).id).toBe(`c${i}`);
        }
      } finally {
        cache.free?.();
      }
    });

    test("compiles multi-file imports and Google well-known types in memory", () => {
      const common = 'syntax="proto3"; package c; message Id { string value = 1; }';
      const root =
        'syntax="proto3"; package u; import "c.proto"; import "google/protobuf/timestamp.proto";' +
        " message U { c.Id id = 1; google.protobuf.Timestamp at = 2; }";
      const d = api.compileProtoFromSources({ "c.proto": common, "u.proto": root }, "u.proto");
      const bytes = api.jsonToProtobuf('{"id":{"value":"x"}}', d, "u.U");
      expect(JSON.parse(api.protobufToJson(bytes, d, "u.U", false)).id.value).toBe("x");
    });
  });
}
