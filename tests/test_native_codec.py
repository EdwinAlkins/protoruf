"""Tests for the native codec (spike S1): protobuf bytes <-> Python objects, no JSON."""

from __future__ import annotations

import pytest
from pydantic import BaseModel

from protoruf import compile_proto_from_sources
from protoruf._protoruf import NativeCodec

SOURCE = """
syntax = "proto3";
package t;

enum Color { COLOR_UNSPECIFIED = 0; COLOR_RED = 1; COLOR_BLUE = 2; }

message Scalars {
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
  Color f_color = 16;
}

message Child { string name = 1; int32 rank = 2; }

message Root {
  Child child = 1;
  optional string note = 2;
  optional int32 count = 3;
  repeated int32 ids = 4;
  repeated string tags = 5;
  repeated Child children = 6;
  map<string, int64> counters = 7;
  map<int32, Child> by_rank = 8;
  repeated Color colors = 9;
}

message Node { string value = 1; Node next = 2; }

message Wide { string name = 1; int64 extra = 2; repeated string more = 3; fixed64 f = 4; float g = 5; }
message Narrow { string name = 1; }
"""

DESCRIPTOR = compile_proto_from_sources({"t.proto": SOURCE}, root="t.proto")

SCALARS = {
    "f_double": -1.5,
    "f_float": 0.25,
    "f_int32": -42,
    "f_int64": -(2**62),
    "f_uint32": 2**32 - 1,
    "f_uint64": 2**64 - 1,
    "f_sint32": -(2**31),
    "f_sint64": -(2**63),
    "f_fixed32": 123456,
    "f_fixed64": 2**63 + 5,
    "f_sfixed32": -7,
    "f_sfixed64": -(2**40),
    "f_bool": True,
    "f_string": "héllo ✓",
    "f_bytes": b"\x00\xff\x10",
    "f_color": 2,
}

ROOT = {
    "child": {"name": "c", "rank": 1},
    "note": "",
    "count": 0,
    "ids": [1, -1, 300, 2**31 - 1],
    "tags": ["a", "", "c"],
    "children": [{"name": "x", "rank": 0}, {"name": "", "rank": 5}],
    "counters": {"a": 1, "b": -(2**40)},
    "by_rank": {3: {"name": "three", "rank": 3}},
    "colors": [1, 2, 0],
}


def codec(message: str, model: type[BaseModel] | None = None) -> NativeCodec:
    return NativeCodec(DESCRIPTOR, f"t.{message}", model)


def test_scalars_round_trip_every_kind():
    c = codec("Scalars")
    assert c.decode_dict(c.encode(SCALARS)) == SCALARS


def test_zero_scalars_are_omitted_then_filled_back():
    c = codec("Scalars")
    zeros = {name: type(value)() for name, value in SCALARS.items()}
    assert c.encode(zeros) == b""
    assert c.decode_dict(b"") == zeros


def test_nested_repeated_maps_and_presence_round_trip():
    c = codec("Root")
    assert c.decode_dict(c.encode(ROOT)) == ROOT


def test_explicit_presence_keeps_zero_values_and_absence():
    c = codec("Root")
    assert c.encode({"note": "", "count": 0}) == bytes.fromhex("1200" "1800")
    decoded = c.decode_dict(b"")
    assert "note" not in decoded and "count" not in decoded and "child" not in decoded
    assert decoded["ids"] == [] and decoded["counters"] == {}


def test_none_is_absent():
    c = codec("Root")
    assert c.encode({"child": None, "note": None}) == b""


def test_recursive_messages():
    c = codec("Node")
    chain = {"value": "a", "next": {"value": "b", "next": {"value": "c"}}}
    assert c.decode_dict(c.encode(chain)) == chain


def test_unknown_fields_are_skipped():
    # Varint, length-delimited, fixed64 and fixed32 unknown fields.
    wide = codec("Wide").encode({"name": "n", "extra": 300, "more": ["x", "y"], "f": 9, "g": 1.5})
    assert codec("Narrow").decode_dict(wide) == {"name": "n"}


def test_pydantic_models_encode_and_validate():
    class Child(BaseModel):
        name: str
        rank: int

    class Root(BaseModel):
        child: Child | None = None
        note: str | None = None
        count: int | None = None
        ids: list[int]
        tags: list[str]
        children: list[Child]
        counters: dict[str, int]
        by_rank: dict[int, Child]
        colors: list[int]

    model = Root.model_validate(ROOT)
    c = codec("Root", Root)
    assert c.decode(c.encode(model)) == model


def test_required_zero_field_survives_round_trip():
    class Child(BaseModel):
        name: str
        rank: int  # required, holds 0: omitted on the wire, must come back as 0

    c = codec("Child", Child)
    assert c.decode(c.encode(Child(name="", rank=0))) == Child(name="", rank=0)


@pytest.mark.parametrize(
    "data",
    [
        b"\x0a",  # truncated length
        b"\x0a\x05ab",  # length beyond the buffer
        b"\x08",  # truncated varint
        b"\x72\x02\xff\xfe",  # invalid UTF-8 in f_string (field 14)
        b"\x08\x01",  # wire type 0 for a double (field 1)
    ],
)
def test_malformed_input_raises_value_error(data):
    with pytest.raises(ValueError, match="protobuf decode error"):
        codec("Scalars").decode_dict(data)


def test_encode_errors_name_the_field():
    # Out of range: ValueError. Wrong type: TypeError.
    with pytest.raises(ValueError, match=r"t\.Scalars\.f_int32"):
        codec("Scalars").encode({"f_int32": 2**40})
    with pytest.raises(TypeError, match=r"t\.Child\.name"):
        codec("Root").encode({"child": {"name": 5}})


def test_unknown_message_type():
    with pytest.raises(ValueError, match="not found"):
        NativeCodec(DESCRIPTOR, "t.Missing")


def test_bytes_match_google_protobuf():
    pb = pytest.importorskip("google.protobuf")
    from google.protobuf import descriptor_pool, message_factory
    from google.protobuf.descriptor_pb2 import FileDescriptorSet

    assert pb.__version__
    pool = descriptor_pool.DescriptorPool()
    for file in FileDescriptorSet.FromString(DESCRIPTOR).file:
        pool.Add(file)

    def upb(name: str):
        return message_factory.GetMessageClass(pool.FindMessageTypeByName(f"t.{name}"))

    scalars = upb("Scalars")(**SCALARS)
    assert codec("Scalars").encode(SCALARS) == scalars.SerializeToString()

    # Maps have no defined order on the wire: compare decoded values both ways.
    root_cls = upb("Root")
    root_raw = root_cls(**ROOT).SerializeToString()
    assert codec("Root").decode_dict(root_raw) == ROOT
    assert root_cls.FromString(codec("Root").encode(ROOT)) == root_cls.FromString(root_raw)

    # Unpacked repeated encoding (proto2 style) must be accepted too.
    unpacked = b"".join(bytes([0x20, v]) for v in (1, 2, 3))
    assert codec("Root").decode_dict(unpacked)["ids"] == [1, 2, 3]
