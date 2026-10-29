"""The decoder never crashes on hostile input: it decodes, or raises `ValueError`.

A light fuzzer (seeded): random bytes, then valid encodings with flipped bits, truncated,
or with bytes inserted, through every decoding mode. A Rust panic would surface as
`pyo3_runtime.PanicException`, which is not a `ValueError`. And known attack shapes
(fields repeated many times) decode in time linear in their size.
"""

import datetime
import enum
import random
import timeit
from collections.abc import Callable

import pytest
from pydantic import BaseModel, ValidationError

from protoruf import Schema, compile_protos

PROTO = """
syntax = "proto3";
package fuzz;
import "google/protobuf/struct.proto";
import "google/protobuf/timestamp.proto";
import "google/protobuf/duration.proto";
import "google/type/date.proto";

enum Kind { KIND_UNSPECIFIED = 0; KIND_A = 1; }

message Node {
  string name = 1;
  repeated int64 values = 2 [packed = true];
  map<string, Node> children = 3;
  Node next = 4;
  Kind kind = 5;
  oneof pick { string text = 6; Node other = 7; }
  google.protobuf.Struct meta = 8;
  google.protobuf.Timestamp at = 9;
  google.protobuf.Duration span = 10;
  google.type.Date day = 11;
  bytes blob = 12;
  double ratio = 13;
}
"""


class Kind(enum.IntEnum):
    KIND_UNSPECIFIED = 0
    KIND_A = 1


class Node(BaseModel):
    name: str = ""
    values: list[int] = []
    children: dict[str, "Node"] = {}
    next: "Node | None" = None
    kind: int = 0
    text: str | None = None
    other: "Node | None" = None
    meta: dict[str, object] | None = None
    at: datetime.datetime | None = None
    span: datetime.timedelta | None = None
    day: datetime.date | None = None
    blob: bytes = b""
    ratio: float = 0.0


@pytest.fixture(scope="module")
def schema() -> Schema:
    return Schema(compile_protos(["fuzz.proto"], sources={"fuzz.proto": PROTO}))


def sample() -> dict[str, object]:
    leaf = {"name": "leaf", "values": [1, -2, 2**40], "kind": 1}
    return {
        "name": "root",
        "values": [5, 6],
        "children": {"a": leaf, "b": {"text": "x"}},
        "next": {"next": leaf},
        "other": leaf,
        "meta": {"k": [1.0, "s", None, {"n": True}]},
        "at": datetime.datetime(2024, 1, 1, tzinfo=datetime.UTC),
        "span": datetime.timedelta(seconds=-3),
        "day": datetime.date(2024, 2, 29),
        "blob": b"\x00\xff",
        "ratio": 1.5,
    }


def mutations(seed: int, valid: bytes) -> list[bytes]:
    rng = random.Random(seed)
    inputs = [bytes(rng.randrange(256) for _ in range(rng.randint(0, 64)))]
    data = bytearray(valid)
    for _ in range(rng.randint(1, 4)):
        data[rng.randrange(len(data))] ^= 1 << rng.randrange(8)
    inputs.append(bytes(data))
    inputs.append(valid[: rng.randrange(len(valid))])
    position = rng.randrange(len(valid))
    inputs.append(valid[:position] + bytes([rng.randrange(256)]) + valid[position:])
    return inputs


@pytest.mark.parametrize("seed", range(300))
def test_hostile_input_raises_value_error_only(schema: Schema, seed: int) -> None:
    codecs = [
        schema.codec("fuzz.Node"),
        schema.codec("fuzz.Node", Node),
        schema.codec("fuzz.Node", Node, mode="construct"),
        schema.codec("fuzz.Node", unknown_enum="error"),
    ]
    valid = codecs[0].encode(sample())
    for data in mutations(seed, valid):
        for codec in codecs:
            try:
                codec.decode(data)
            except (ValueError, ValidationError):
                pass  # refused cleanly


_ENTRY = bytes.fromhex("0a070a016b12020800")  # Struct.fields entry: "k" -> null

# Each shape repeats one unit n times, in one message.
SHAPES: dict[str, Callable[[int], bytes]] = {
    "merged sub-message": lambda n: bytes.fromhex("22030a0178") * n,
    "merged at two depths": lambda n: bytes.fromhex("220522030a0178") * n,
    "oneof switching": lambda n: bytes.fromhex("3a030a0178320179") * n,
    "unknown fields": lambda n: bytes.fromhex("a00101") * n,
    "duplicate map keys": lambda n: bytes.fromhex("1a050a016b1200") * n,
    "packed varints": lambda n: b"\x12" + _varint(n) + b"\x01" * n,
    "struct entries": lambda n: b"\x42" + _varint(len(_ENTRY) * n) + _ENTRY * n,
    "merged structs": lambda n: (b"\x42" + _varint(len(_ENTRY)) + _ENTRY) * n,
}


@pytest.mark.parametrize("shape", SHAPES)
def test_attack_shapes_decode_in_linear_time(schema: Schema, shape: str) -> None:
    codec = schema.codec("fuzz.Node")
    build = SHAPES[shape]

    def best_time(n: int) -> float:
        data = build(n)
        return min(timeit.repeat(lambda: codec.decode(data), number=3, repeat=5))

    best_time(100)  # warm up
    # Linear: about 8x. protoruf 0.5.0 (quadratic merging) fails, or hangs on nested merges.
    assert best_time(2_000) < 24 * best_time(250)


def test_deep_nesting_is_refused(schema: Schema) -> None:
    codec = schema.codec("fuzz.Node")
    data = b""
    for _ in range(200):  # `next` (field 4) nested 200 times
        data = bytes([0x22]) + _varint(len(data)) + data
    with pytest.raises(ValueError, match="nesting too deep"):
        codec.decode(data)


def _varint(value: int) -> bytes:
    out = bytearray()
    while value > 0x7F:
        out.append(value & 0x7F | 0x80)
        value >>= 7
    out.append(value)
    return bytes(out)
