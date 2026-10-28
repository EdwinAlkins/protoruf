"""Native codec: well-known types, enums, and construct mode (no validation)."""

from __future__ import annotations

import datetime as dt
import enum

import pytest
from pydantic import BaseModel, ConfigDict, Field, PrivateAttr, ValidationError

from protoruf import OpenIntEnum, compile_proto_from_sources
from protoruf._protoruf import NativeCodec

SOURCE = """
syntax = "proto3";
package k;
import "google/protobuf/timestamp.proto";
import "google/protobuf/duration.proto";
import "google/protobuf/wrappers.proto";
import "google/protobuf/struct.proto";
import "google/protobuf/field_mask.proto";

enum Color { COLOR_UNSPECIFIED = 0; COLOR_RED = 1; COLOR_BLUE = 2; }

message Times {
  google.protobuf.Timestamp at = 1;
  google.protobuf.Duration took = 2;
  repeated google.protobuf.Timestamp history = 3;
  map<string, google.protobuf.Duration> timings = 4;
}

message Wrapped {
  google.protobuf.StringValue name = 1;
  google.protobuf.Int64Value count = 2;
  google.protobuf.BoolValue flag = 3;
  google.protobuf.DoubleValue ratio = 4;
  google.protobuf.BytesValue blob = 5;
  repeated google.protobuf.UInt32Value ids = 6;
}

message Dynamic {
  google.protobuf.Struct doc = 1;
  google.protobuf.Value any = 2;
  google.protobuf.ListValue items = 3;
  google.protobuf.FieldMask mask = 4;
}

message Painted {
  Color color = 1;
  repeated Color palette = 2;
  map<string, Color> by_name = 3;
}

message Line { string sku = 1; int32 qty = 2; Color color = 3; }

message Order {
  string id = 1;
  Line main = 2;
  repeated Line lines = 3;
  map<string, Line> by_sku = 4;
  google.protobuf.Timestamp at = 5;
  int32 unused_by_model = 6;
}

message Node { string value = 1; Node next = 2; }
"""

DESCRIPTOR = compile_proto_from_sources({"k.proto": SOURCE}, root="k.proto")
UTC = dt.timezone.utc


def codec(message: str, model: type | None = None, **options) -> NativeCodec:
    return NativeCodec(DESCRIPTOR, f"k.{message}", model, **options)


def upb(name: str):
    pytest.importorskip("google.protobuf")
    from google.protobuf import descriptor_pool, message_factory
    from google.protobuf.descriptor_pb2 import FileDescriptorSet

    pool = descriptor_pool.DescriptorPool()
    for file in FileDescriptorSet.FromString(DESCRIPTOR).file:
        pool.Add(file)
    return message_factory.GetMessageClass(pool.FindMessageTypeByName(name))


# --------------------------------------------------------------- well-known types


@pytest.mark.parametrize(
    "moment",
    [
        dt.datetime(2024, 2, 29, 12, 30, 15, 123456, tzinfo=UTC),
        dt.datetime(1969, 12, 31, 23, 59, 59, 500000, tzinfo=UTC),
        dt.datetime(1, 1, 1, tzinfo=UTC),
        dt.datetime(9999, 12, 31, 23, 59, 59, 999999, tzinfo=UTC),
        dt.datetime(1970, 1, 1, tzinfo=UTC),
    ],
)
def test_timestamp_round_trip_and_matches_upb(moment):
    c = codec("Times")
    raw = c.encode({"at": moment})
    assert c.decode_dict(raw)["at"] == moment
    expected = upb("k.Times")()
    expected.at.FromDatetime(moment)
    assert raw == expected.SerializeToString()


def test_timestamp_with_offset_is_stored_in_utc():
    paris = dt.timezone(dt.timedelta(hours=2))
    local = dt.datetime(2024, 6, 1, 14, 0, tzinfo=paris)
    decoded = codec("Times").decode_dict(codec("Times").encode({"at": local}))["at"]
    assert decoded == local and decoded.tzinfo is UTC and decoded.hour == 12


def test_naive_datetime_is_refused_unless_configured():
    naive = dt.datetime(2024, 1, 1, 8, 0)
    with pytest.raises(ValueError, match="naive datetime"):
        codec("Times").encode({"at": naive})
    c = codec("Times", naive_datetime="utc")
    assert c.decode_dict(c.encode({"at": naive}))["at"] == naive.replace(tzinfo=UTC)


def test_timestamp_rejects_wrong_type_and_out_of_range_wire_values():
    with pytest.raises(TypeError, match=r"k\.Times\.at.*datetime"):
        codec("Times").encode({"at": "2024-01-01"})
    # Times{at: Timestamp{seconds: 253402300800}}, i.e. year 10000.
    out_of_range = bytes.fromhex("0a07088083d1ffaf07")
    with pytest.raises(ValueError, match="out of range"):
        codec("Times").decode_dict(out_of_range)


@pytest.mark.parametrize(
    "delta",
    [
        dt.timedelta(0),
        dt.timedelta(seconds=90, microseconds=250),
        dt.timedelta(microseconds=-500_000),
        dt.timedelta(days=-3, hours=5),
        dt.timedelta(days=3_650_000),
    ],
)
def test_duration_round_trip_and_matches_upb(delta):
    c = codec("Times")
    raw = c.encode({"took": delta})
    assert c.decode_dict(raw)["took"] == delta
    expected = upb("k.Times")()
    expected.took.FromTimedelta(delta)
    assert raw == expected.SerializeToString()


def test_repeated_and_map_well_known_values():
    c = codec("Times")
    value = {
        "history": [dt.datetime(2020, 1, 1, tzinfo=UTC), dt.datetime(2021, 1, 1, tzinfo=UTC)],
        "timings": {"db": dt.timedelta(milliseconds=12), "zero": dt.timedelta(0)},
    }
    decoded = c.decode_dict(c.encode(value))
    assert decoded["history"] == value["history"]
    assert decoded["timings"] == value["timings"]


def test_wrappers_keep_none_apart_from_zero():
    c = codec("Wrapped")
    assert c.encode({"name": None, "count": None}) == b""
    zeros = {"name": "", "count": 0, "flag": False, "ratio": 0.0, "blob": b""}
    raw = c.encode(zeros)
    assert raw == upb("k.Wrapped")(
        name={"value": ""}, count={}, flag={}, ratio={}, blob={}
    ).SerializeToString()
    assert {k: v for k, v in c.decode_dict(raw).items() if k in zeros} == zeros
    full = {"name": "n", "count": -5, "flag": True, "ratio": 0.5, "blob": b"\x00\xff", "ids": [0, 7]}
    assert c.decode_dict(c.encode(full)) == full
    absent = c.decode_dict(b"")
    assert "name" not in absent and absent["ids"] == []


def test_struct_value_list_and_field_mask():
    c = codec("Dynamic")
    doc = {"a": 1, "b": [True, None, "x", {"c": 2.5}], "d": {}, "e": []}
    value = {"doc": doc, "any": [1, "two", None], "items": [{"k": "v"}, 3], "mask": ["a.b", "c"]}
    decoded = c.decode_dict(c.encode(value))
    # Numbers are doubles in google.protobuf.Value: 1 comes back as 1.0 (== 1).
    assert decoded == value
    assert isinstance(decoded["doc"]["a"], float)

    dynamic = upb("k.Dynamic")  # one class: messages of distinct pools never compare equal
    expected = dynamic()
    expected.doc.update(doc)
    expected.any.list_value.extend([1, "two", None])
    expected.mask.paths.extend(["a.b", "c"])
    ours = c.encode({"doc": doc, "any": [1, "two", None], "mask": ["a.b", "c"]})
    assert dynamic.FromString(ours) == expected
    assert c.decode_dict(expected.SerializeToString())["doc"] == doc


def test_value_rejects_unsupported_types():
    with pytest.raises(TypeError, match="google.protobuf.Value"):
        codec("Dynamic").encode({"any": {1, 2}})


# --------------------------------------------------------------------------- enums


class Color(OpenIntEnum):
    UNSPECIFIED = 0
    RED = 1
    BLUE = 2


class ClosedColor(enum.IntEnum):
    UNSPECIFIED = 0
    RED = 1
    BLUE = 2


class Painted(BaseModel):
    color: Color
    palette: list[Color] = []
    by_name: dict[str, Color] = {}


UNKNOWN = b"\x08\x07"  # color = 7, not declared


def test_open_enum_keeps_unknown_values_in_both_modes():
    for mode in ("validate", "construct"):
        c = codec("Painted", Painted, mode=mode)
        decoded = c.decode(UNKNOWN)
        assert decoded.color == 7 and decoded.color.name == "UNRECOGNIZED_7"
        assert isinstance(decoded.color, Color)
        assert c.encode(decoded) == UNKNOWN


def test_unknown_enum_error_option():
    with pytest.raises(ValueError, match="unknown value 7 for enum k.Color"):
        codec("Painted", unknown_enum="error").decode_dict(UNKNOWN)


def test_closed_enum_rejects_unknown_on_validation_but_keeps_int_on_construct():
    class Closed(BaseModel):
        color: ClosedColor

    with pytest.raises(ValidationError):
        codec("Painted", Closed).decode(UNKNOWN)
    assert codec("Painted", Closed, mode="construct").decode(UNKNOWN).color == 7


def test_construct_mode_builds_enum_members_everywhere():
    c = codec("Painted", Painted, mode="construct")
    raw = c.encode({"color": 2, "palette": [1, 0], "by_name": {"x": 1}})
    decoded = c.decode(raw)
    assert decoded.color is Color.BLUE
    assert decoded.palette == [Color.RED, Color.UNSPECIFIED]
    assert all(type(v) is Color for v in [*decoded.palette, *decoded.by_name.values()])
    # An absent enum field is the zero member, not the int 0.
    assert c.decode(b"").color is Color.UNSPECIFIED


def test_enum_encodes_from_ints_members_and_proto_names():
    class StrColor(enum.StrEnum):
        RED = "COLOR_RED"

    class NamedColor(enum.Enum):
        COLOR_BLUE = "blue"

    c = codec("Painted")
    expected = c.encode({"color": 2, "palette": [1, 1, 2]})
    assert c.encode({"color": "COLOR_BLUE", "palette": [Color.RED, StrColor.RED, NamedColor.COLOR_BLUE]}) == expected
    with pytest.raises(ValueError, match=r"'BLUE' is not a value of enum k\.Color"):
        c.encode({"color": "BLUE"})


# ---------------------------------------------------------------- construct mode


class Line(BaseModel):
    sku: str = Field(min_length=3)
    qty: int
    color: Color = Color.UNSPECIFIED


class Order(BaseModel):
    id: str
    main: Line | None = None
    lines: list[Line] = []
    by_sku: dict[str, Line] = {}
    at: dt.datetime | None = None
    note: str = "not on the wire"
    tags: list[str] = Field(default_factory=list)


ORDER = Order(
    id="o1",
    main=Line(sku="AAA", qty=1, color=Color.RED),
    lines=[Line(sku="BBB", qty=2), Line(sku="CCC", qty=0)],
    by_sku={"DDD": Line(sku="DDD", qty=4, color=Color.BLUE)},
    at=dt.datetime(2024, 5, 1, tzinfo=UTC),
)


def test_construct_equals_validate_for_valid_data():
    raw = codec("Order", Order).encode(ORDER)
    constructed = codec("Order", Order, mode="construct").decode(raw)
    assert constructed == codec("Order", Order).decode(raw) == ORDER
    assert type(constructed.main) is Line and type(constructed.lines[0]) is Line
    assert type(constructed.by_sku["DDD"]) is Line
    assert constructed.model_dump() == ORDER.model_dump()
    assert constructed.model_dump_json() == ORDER.model_dump_json()


def test_construct_skips_validation():
    raw = codec("Line").encode({"sku": "x", "qty": 1})
    with pytest.raises(ValidationError):
        codec("Line", Line).decode(raw)
    assert codec("Line", Line, mode="construct").decode(raw).sku == "x"


def test_construct_defaults_fields_set_and_dropped_fields():
    c = codec("Order", Order, mode="construct")
    first = c.decode(codec("Order").encode({"id": "o", "unused_by_model": 9}))
    second = c.decode(b"")
    assert first.note == "not on the wire"
    assert first.tags == [] and first.tags is not second.tags  # factory called each time
    assert "unused_by_model" not in first.__dict__
    assert first.main is None and first.at is None
    assert "id" in first.model_fields_set and "note" not in first.model_fields_set


def test_construct_extra_allow_private_attrs_and_post_init():
    class Extra(BaseModel):
        model_config = ConfigDict(extra="allow")
        value: str

    class WithPrivate(BaseModel):
        value: str
        _cache: dict = PrivateAttr(default_factory=dict)

        def model_post_init(self, context) -> None:
            self._cache["seen"] = True

    raw = codec("Node").encode({"value": "v"})
    extra = codec("Node", Extra, mode="construct").decode(raw)
    assert extra.value == "v" and extra.__pydantic_extra__ == {}
    private = codec("Node", WithPrivate, mode="construct").decode(raw)
    assert private.value == "v" and private._cache == {"seen": True}


def test_construct_recursive_models():
    class Node(BaseModel):
        value: str
        next: Node | None = None

    chain = Node(value="a", next=Node(value="b", next=Node(value="c")))
    c = codec("Node", Node, mode="construct")
    decoded = c.decode(c.encode(chain))
    assert decoded == chain and type(decoded.next.next) is Node


def test_invalid_options():
    with pytest.raises(ValueError, match="requires a model"):
        codec("Order", mode="construct")
    for option in ({"mode": "fast"}, {"unknown_enum": "drop"}, {"naive_datetime": "local"}):
        with pytest.raises(ValueError):
            codec("Order", Order, **option)
