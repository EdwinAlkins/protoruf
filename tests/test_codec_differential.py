"""Differential test against `google.protobuf` (upb), the reference implementation.

Random messages (seeded, so failures reproduce) are encoded by the native codec; upb must
parse them and serialize the very same bytes, and the codec must decode upb's bytes back
to the expected values. Map entries are compared as sets: their order is unspecified (upb's
"deterministic" order puts zero keys last).
"""

import datetime
import random
import struct
from typing import Any

import pytest

from protoruf import Schema, compile_protos

json_format = pytest.importorskip("google.protobuf.json_format")
from google.protobuf import descriptor_pb2, descriptor_pool, message_factory  # noqa: E402

PROTO = """
syntax = "proto3";
package diff;
import "google/protobuf/duration.proto";
import "google/protobuf/timestamp.proto";
import "google/protobuf/wrappers.proto";

enum Color { COLOR_UNSPECIFIED = 0; COLOR_RED = 1; COLOR_BLUE = 2; }

message Inner {
  int32 a = 1;
  string s = 2;
  repeated sint64 zs = 3;
  Inner child = 4;
}

message All {
  double f_double = 1;
  float f_float = 2;
  int32 f_int32 = 3;
  int64 f_int64 = 4;
  uint32 f_uint32 = 5;
  uint64 f_uint64 = 6;
  sint32 f_sint32 = 7;
  sint64 f_sint64 = 8;
  fixed32 f_fixed32 = 9;
  fixed64 f_fixed64 = 10;
  sfixed32 f_sfixed32 = 11;
  sfixed64 f_sfixed64 = 12;
  bool f_bool = 13;
  string f_string = 14;
  bytes f_bytes = 15;
  Color f_enum = 16;
  optional int32 o_int32 = 17;
  optional string o_string = 18;
  repeated int32 r_int32 = 19;
  repeated double r_double = 20;
  repeated string r_string = 21;
  repeated bytes r_bytes = 22;
  repeated Color r_enum = 23;
  repeated Inner r_inner = 24;
  repeated int64 r_unpacked = 25 [packed = false];
  map<string, int32> m_str_int = 26;
  map<int64, string> m_int_str = 27;
  map<bool, Inner> m_bool_inner = 28;
  map<uint32, Color> m_enum = 29;
  Inner inner = 30;
  oneof choice {
    string c_string = 31;
    Inner c_inner = 32;
    int64 c_int = 33;
  }
  google.protobuf.Timestamp ts = 34;
  google.protobuf.Duration dur = 35;
  google.protobuf.Int32Value w_int = 36;
  google.protobuf.StringValue w_str = 37;
  int32 big_number = 536870911;
}
"""

I32 = (-(2**31), 2**31 - 1)
I64 = (-(2**63), 2**63 - 1)
U32 = (0, 2**32 - 1)
U64 = (0, 2**64 - 1)
INTEGERS = {
    "int32": I32, "int64": I64, "uint32": U32, "uint64": U64, "sint32": I32,
    "sint64": I64, "fixed32": U32, "fixed64": U64, "sfixed32": I32, "sfixed64": I64,
}  # fmt: skip
ENUM_VALUES = (0, 1, 2, 7, -3)  # 7 and -3 are not declared: open enum


@pytest.fixture(scope="module")
def setup() -> tuple[Schema, Any]:
    descriptor = compile_protos(["diff.proto"], sources={"diff.proto": PROTO})
    pool = descriptor_pool.DescriptorPool()
    for file in descriptor_pb2.FileDescriptorSet.FromString(descriptor).file:
        pool.Add(file)
    upb_class = message_factory.GetMessageClass(pool.FindMessageTypeByName("diff.All"))
    return Schema(descriptor), upb_class


class Generator:
    def __init__(self, seed: int) -> None:
        self.random = random.Random(seed)

    def chance(self, p: float = 0.5) -> bool:
        return self.random.random() < p

    def integer(self, kind: str) -> int:
        low, high = INTEGERS[kind]
        choice = self.random.random()
        if choice < 0.2:
            return self.random.choice((low, high, 0, 1, -1 if low < 0 else 2))
        if choice < 0.5:
            return self.random.randint(max(low, -300), min(high, 300))
        return self.random.randint(low, high)

    def double(self) -> float:
        specials = (0.0, -0.0, 1.5, -2.25, 1e308, -5e-324, float("inf"), float("-inf"))
        return self.random.choice(specials) if self.chance(0.3) else self.random.uniform(-1e9, 1e9)

    def float32(self) -> float:
        value = self.double()
        return struct.unpack("<f", struct.pack("<f", value if abs(value) < 3e38 else 1.0))[0]

    def text(self) -> str:
        alphabet = "abcXYZ019 éàü€漢字🙂\n\t\"\\"
        return "".join(self.random.choice(alphabet) for _ in range(self.random.randint(0, 12)))

    def blob(self) -> bytes:
        return bytes(self.random.randrange(256) for _ in range(self.random.randint(0, 12)))

    def enum(self) -> int:
        return self.random.choice(ENUM_VALUES)

    def inner(self, depth: int = 0) -> dict[str, Any]:
        value: dict[str, Any] = {}
        if self.chance():
            value["a"] = self.integer("int32")
        if self.chance():
            value["s"] = self.text()
        if self.chance():
            value["zs"] = [self.integer("sint64") for _ in range(self.random.randint(0, 3))]
        if depth < 2 and self.chance(0.3):
            value["child"] = self.inner(depth + 1)
        return value

    def message(self) -> dict[str, Any]:
        g: dict[str, Any] = {}
        maybe = self.chance
        if maybe():
            g["f_double"] = self.double()
        if maybe():
            g["f_float"] = self.float32()
        for kind in INTEGERS:
            if maybe():
                g[f"f_{kind}"] = self.integer(kind)
        if maybe():
            g["f_bool"] = self.chance()
        if maybe():
            g["f_string"] = self.text()
        if maybe():
            g["f_bytes"] = self.blob()
        if maybe():
            g["f_enum"] = self.enum()
        if maybe():
            g["o_int32"] = self.random.choice((0, self.integer("int32")))
        if maybe():
            g["o_string"] = self.random.choice(("", self.text()))
        lists = {
            "r_int32": lambda: self.integer("int32"),
            "r_double": self.double,
            "r_string": self.text,
            "r_bytes": self.blob,
            "r_enum": self.enum,
            "r_inner": self.inner,
            "r_unpacked": lambda: self.integer("int64"),
        }
        for name, make in lists.items():
            if maybe():
                g[name] = [make() for _ in range(self.random.randint(0, 4))]
        maps = {
            "m_str_int": (self.text, lambda: self.integer("int32")),
            "m_int_str": (lambda: self.integer("int64"), self.text),
            "m_bool_inner": (self.chance, self.inner),
            "m_enum": (lambda: self.integer("uint32"), self.enum),
        }
        for name, (make_key, make_value) in maps.items():
            if maybe():
                g[name] = {make_key(): make_value() for _ in range(self.random.randint(0, 3))}
        if maybe():
            g["inner"] = self.inner()
        choice = self.random.choice((None, "c_string", "c_inner", "c_int"))
        if choice == "c_string":
            g["c_string"] = self.text()
        elif choice == "c_inner":
            g["c_inner"] = self.inner()
        elif choice == "c_int":
            g["c_int"] = self.integer("int64")
        if maybe():
            seconds = self.random.randint(-62135596800, 253402300799)
            micros = self.random.randint(0, 999_999)
            g["ts"] = datetime.datetime.fromtimestamp(seconds, datetime.UTC).replace(
                microsecond=micros
            )
        if maybe():
            g["dur"] = datetime.timedelta(microseconds=self.random.randint(-10**15, 10**15))
        if maybe():
            g["w_int"] = self.random.choice((0, self.integer("int32")))
        if maybe():
            g["w_str"] = self.random.choice(("", self.text()))
        if maybe():
            g["big_number"] = self.integer("int32")
        return g


INNER_DEFAULTS = {"a": 0, "s": "", "zs": []}
ALL_DEFAULTS: dict[str, Any] = {
    "f_double": 0.0, "f_float": 0.0, "f_bool": False, "f_string": "", "f_bytes": b"",
    "f_enum": 0, "big_number": 0,
    **{f"f_{kind}": 0 for kind in INTEGERS},
    **{name: [] for name in (
        "r_int32", "r_double", "r_string", "r_bytes", "r_enum", "r_inner", "r_unpacked"
    )},
    **{name: {} for name in ("m_str_int", "m_int_str", "m_bool_inner", "m_enum")},
}  # fmt: skip


def expected_inner(value: dict[str, Any]) -> dict[str, Any]:
    result = {**INNER_DEFAULTS, **value}
    if "child" in value:
        result["child"] = expected_inner(value["child"])
    return result


def expected(generated: dict[str, Any]) -> dict[str, Any]:
    """What decoding gives: implicit fields filled with their zero value."""
    result = {**ALL_DEFAULTS, **generated}
    result["r_inner"] = [expected_inner(v) for v in generated.get("r_inner", [])]
    result["m_bool_inner"] = {
        k: expected_inner(v) for k, v in generated.get("m_bool_inner", {}).items()
    }
    for name in ("inner", "c_inner"):
        if name in generated:
            result[name] = expected_inner(generated[name])
    return result


MAP_FIELDS = frozenset({26, 27, 28, 29})


def top_level_fields(data: bytes) -> list[tuple[int, bytes]]:
    """`(field number, encoded field)` in wire order."""
    fields, pos = [], 0

    def varint() -> int:
        nonlocal pos
        result = shift = 0
        while True:
            byte = data[pos]
            pos += 1
            result |= (byte & 0x7F) << shift
            shift += 7
            if byte < 0x80:
                return result

    while pos < len(data):
        start = pos
        tag = varint()
        wire_type = tag & 7
        if wire_type == 0:
            varint()
        elif wire_type == 1:
            pos += 8
        elif wire_type == 5:
            pos += 4
        else:
            length = varint()  # not `pos += varint()`: `pos` would be read first
            pos += length
        fields.append((tag >> 3, data[start:pos]))
    return fields


def canonical(data: bytes) -> tuple[list[tuple[int, bytes]], list[tuple[int, bytes]]]:
    """Fields in order, except map entries, sorted (their order is unspecified)."""
    fields = top_level_fields(data)
    ordered = [f for f in fields if f[0] not in MAP_FIELDS]
    entries = sorted(f for f in fields if f[0] in MAP_FIELDS)
    return ordered, entries


@pytest.mark.parametrize("seed", range(400))
def test_codec_matches_upb(setup: tuple[Schema, Any], seed: int) -> None:
    schema, upb_class = setup
    codec = schema.codec("diff.All")
    generated = Generator(seed).message()

    ours = codec.encode(generated)
    upb_message = upb_class.FromString(ours)
    theirs = upb_message.SerializeToString(deterministic=True)

    assert canonical(ours) == canonical(theirs)
    assert codec.decode(theirs) == expected(generated)
    assert codec.decode(ours) == expected(generated)
