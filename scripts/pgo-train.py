"""Profile-guided optimization (PGO) workload: the native codec in every mode, and the
JSON path, on messages of every shape.

    python scripts/pgo-train.py          # run the workload (on an instrumented build)
    python scripts/pgo-train.py --bench  # time encode / decode, µs per message

`scripts/pgo-prepare.sh` runs it to build the Linux wheels with PGO.
"""

import datetime
import random
import sys
import timeit

from pydantic import BaseModel

from protoruf import (
    OpenIntEnum,
    Schema,
    compile_proto_from_sources,
    json_to_protobuf,
    protobuf_to_json,
)

PROTO = """
syntax = "proto3";
package train;
import "google/protobuf/struct.proto";
import "google/protobuf/timestamp.proto";
import "google/protobuf/duration.proto";
import "google/protobuf/wrappers.proto";

enum State { STATE_UNSPECIFIED = 0; STATE_ON = 1; STATE_OFF = 2; }

message Point { double x = 1; double y = 2; float z = 3; }
message Reading {
  string sensor = 1;
  int64 at_ms = 2;
  sint32 delta = 3;
  uint64 counter = 4;
  bool ok = 5;
  State state = 6;
  repeated double samples = 7;
  repeated Point path = 8;
  map<string, string> tags = 9;
  map<int32, Point> marks = 10;
  optional string note = 11;
  bytes raw = 12;
  google.protobuf.Timestamp when = 13;
  google.protobuf.Duration span = 14;
  google.protobuf.Struct extra = 15;
  google.protobuf.Int64Value limit = 16;
  oneof source { string device = 17; Point origin = 18; }
}
message Batch { repeated Reading readings = 1; string id = 2; }
"""


class State(OpenIntEnum):
    UNSPECIFIED = 0
    ON = 1
    OFF = 2


class Point(BaseModel):
    x: float = 0.0
    y: float = 0.0
    z: float = 0.0


class Reading(BaseModel):
    sensor: str = ""
    at_ms: int = 0
    delta: int = 0
    counter: int = 0
    ok: bool = False
    state: State = State.UNSPECIFIED
    samples: list[float] = []
    path: list[Point] = []
    tags: dict[str, str] = {}
    marks: dict[int, Point] = {}
    note: str | None = None
    raw: bytes = b""
    when: datetime.datetime | None = None
    span: datetime.timedelta | None = None
    extra: dict[str, object] | None = None
    limit: int | None = None
    device: str | None = None
    origin: Point | None = None


class Batch(BaseModel):
    readings: list[Reading] = []
    id: str = ""


def reading(rng: random.Random, size: int) -> Reading:
    return Reading(
        sensor=f"s-{rng.randrange(10**6)}",
        at_ms=rng.randrange(2**40),
        delta=rng.randrange(-1000, 1000),
        counter=rng.randrange(2**63),
        ok=rng.random() < 0.5,
        state=State(rng.randrange(3)),
        samples=[rng.uniform(-1e3, 1e3) for _ in range(size)],
        path=[Point(x=rng.random(), y=rng.random(), z=1.5) for _ in range(size // 2)],
        tags={f"k{i}": f"v{i}" for i in range(size // 4)},
        marks={i: Point(x=float(i)) for i in range(size // 8)},
        note="n" if rng.random() < 0.5 else None,
        raw=rng.randbytes(size),
        when=datetime.datetime(2026, 1, 1, tzinfo=datetime.UTC),
        span=datetime.timedelta(seconds=rng.randrange(100)),
        extra={"a": [1, "b", None, {"c": True}]} if size > 4 else None,
        limit=rng.randrange(100) if rng.random() < 0.5 else None,
        device="d" if rng.random() < 0.5 else None,
    )


def bench(schema: Schema, batch: Batch, single: Reading) -> None:
    def per_call(function: object) -> float:
        timer = timeit.Timer(function)  # type: ignore[arg-type]
        number, _ = timer.autorange()
        return min(timer.repeat(5, number)) / number * 1e6

    for name, model, message in (("Reading", Reading, single), ("Batch", Batch, batch)):
        for mode in ("validate", "construct"):
            codec = schema.codec(f"train.{name}", model, mode=mode)
            data = codec.encode(message)
            encode = per_call(lambda: codec.encode(message))
            decode = per_call(lambda: codec.decode(data))
            print(f"{name:8} {mode:9} encode {encode:8.2f} µs  decode {decode:8.2f} µs")


def main() -> None:
    rng = random.Random(7)
    descriptor = compile_proto_from_sources({"train.proto": PROTO}, root="train.proto")
    schema = Schema(descriptor)
    batches = [Batch(id=str(i), readings=[reading(rng, s) for s in (2, 8, 32)]) for i in range(20)]
    singles = [reading(rng, s) for s in (0, 1, 4, 16, 64) for _ in range(20)]
    if "--bench" in sys.argv:
        bench(schema, batches[0], singles[60])
        return
    for mode in ("validate", "construct", "dict"):
        batch_codec = schema.codec("train.Batch", Batch, mode=mode)
        reading_codec = schema.codec("train.Reading", Reading, mode=mode)
        for _ in range(30):
            for batch in batches:
                batch_codec.decode(batch_codec.encode(batch))
            for single in singles:
                reading_codec.decode(reading_codec.encode(single))
    for _ in range(5):
        for single in singles[:40]:
            data = schema.codec("train.Reading", Reading).encode(single)
            text = protobuf_to_json(data, descriptor, message_type="train.Reading")
            json_to_protobuf(text, descriptor, "train.Reading")


if __name__ == "__main__":
    main()
