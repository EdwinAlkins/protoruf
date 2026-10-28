#!/usr/bin/env python3
"""
Spike S1 — native codec (protobuf bytes <-> Python objects, no JSON) vs alternatives.

Question: is a Rust codec that reads/builds Python objects directly clearly faster
(target: >= 3x) than fastgrpc2's codec (Python loop per field + upb message)?

Compared, for encode (Pydantic model -> bytes) and decode (bytes -> Pydantic model):

  native            protoruf.NativeCodec (decode validates through pydantic-core)
  native (dict)     NativeCodec.decode_dict: no Pydantic at all (lower bound)
  fastgrpc2         fastgrpc2.core.codec serializer/deserializer          [optional]
  upb + pydantic    google.protobuf message <-> model_dump / MessageToDict  [optional]
  protoruf JSON     DescriptorCache.pydantic_to_protobuf / protobuf_to_pydantic
  upb raw           SerializeToString / FromString only, no Pydantic (reference floor)
  validate only     Model.model_validate(dict): the Pydantic share of a decode

Timing: best-of-N repeats of a tight loop, reported in microseconds per operation.
"""

from __future__ import annotations

import enum
import gc
import sys
import time
from pathlib import Path
from typing import Callable

from pydantic import BaseModel

from protoruf import DescriptorCache, compile_proto
from protoruf._protoruf import NativeCodec

sys.path.insert(0, str(Path(__file__).resolve().parent))
from benchmark_utils import HAS_GOOGLE_PROTOBUF, create_google_message_factory  # noqa: E402

try:
    from fastgrpc2.core import codec as fastgrpc2_codec

    HAS_FASTGRPC2 = True
except ImportError:
    HAS_FASTGRPC2 = False

PROTO = Path(__file__).resolve().parent / "proto" / "order.proto"


class Status(enum.IntEnum):
    __proto_package__ = "bench"  # read by fastgrpc2 only
    UNSPECIFIED = 0
    ACTIVE = 1
    CLOSED = 2


class Address(BaseModel):
    __proto_package__ = "bench"
    street: str
    city: str
    zip: str
    country: str


class Line(BaseModel):
    __proto_package__ = "bench"
    sku: str
    qty: int
    price: float


class Order(BaseModel):
    __proto_package__ = "bench"
    id: str
    customer: str
    email: str
    created: int
    updated: int
    total: float
    tax: float
    paid: bool
    shipped: bool
    status: Status
    note: str
    currency: str
    version: int
    signature: bytes
    billing: Address | None = None
    shipping: Address | None = None
    lines: list[Line] = []
    tags: list[str] = []
    priority: int
    channel: str


class Small(BaseModel):
    __proto_package__ = "bench"
    id: str
    value: int


def make_order() -> Order:
    address = Address(street="12 rue de la Paix", city="Paris", zip="75002", country="FR")
    return Order(
        id="ord_01HZX3K9F2",
        customer="cus_8842",
        email="someone@example.com",
        created=1_730_000_000_000,
        updated=1_730_000_360_000,
        total=1234.56,
        tax=205.76,
        paid=True,
        shipped=False,
        status=Status.ACTIVE,
        note="Leave at the door, ring twice",
        currency="EUR",
        version=7,
        signature=b"\x01\x02\x03" * 10,
        billing=address,
        shipping=address,
        lines=[Line(sku=f"SKU-{i:04d}", qty=i + 1, price=9.99 * (i + 1)) for i in range(5)],
        tags=["priority", "gift", "fragile", "eu"],
        priority=3,
        channel="web",
    )


def per_op_us(body: Callable[[], object], *, number: int, repeat: int = 7) -> float:
    """Best-of-`repeat` time per call, in microseconds, GC disabled."""
    for _ in range(min(number, 2_000)):
        body()
    gc.disable()
    try:
        best = float("inf")
        for _ in range(repeat):
            start = time.perf_counter_ns()
            for _ in range(number):
                body()
            best = min(best, (time.perf_counter_ns() - start) / number)
    finally:
        gc.enable()
    return best / 1_000


FAILS = -1.0


def fails_or_us(body: Callable[[], object], *, number: int) -> float:
    """Like `per_op_us`, but report FAILS when the codec cannot handle the message.

    Kept as a guard: protoruf <= 0.2.0's Pydantic helpers failed on `bytes` fields and
    on required fields holding a zero value.
    """
    try:
        body()
    except ValueError as exc:
        print(f"  note: {str(exc).splitlines()[0]} -> {str(exc).splitlines()[1:2]}")
        return FAILS
    return per_op_us(body, number=number)


def upb_pydantic_pair(descriptor: bytes, message_type: str, model: type[BaseModel]):
    from google.protobuf.json_format import MessageToDict

    cls = create_google_message_factory(descriptor, message_type)

    def encode(instance: BaseModel) -> bytes:
        return cls(**instance.model_dump()).SerializeToString()

    def decode(raw: bytes) -> BaseModel:
        data = MessageToDict(
            cls.FromString(raw),
            preserving_proto_field_name=True,
            always_print_fields_with_no_presence=True,
            use_integers_for_enums=True,
        )
        return model.model_validate(data)

    return cls, encode, decode


def check_correctness(descriptor: bytes, instance: BaseModel, message_type: str, codec: NativeCodec) -> None:
    raw = codec.encode(instance)
    assert codec.decode(raw) == instance, "native round-trip changed the value"
    if HAS_GOOGLE_PROTOBUF:
        cls, upb_encode, _ = upb_pydantic_pair(descriptor, message_type, type(instance))
        assert raw == upb_encode(instance), "native bytes differ from google.protobuf (upb)"
    if HAS_FASTGRPC2:
        legacy = fastgrpc2_codec.serializer(type(instance))(instance)
        assert codec.decode(legacy) == instance, "native cannot read fastgrpc2 bytes"
        # The reverse is expected to fail when a required field holds a zero value:
        # proto3 omits it on the wire, and fastgrpc2 (explicit presence everywhere)
        # then reports it as missing. Reported, not asserted.
        try:
            fastgrpc2_codec.deserializer(type(instance))(raw)
        except ValueError as exc:
            first = str(exc).splitlines()[1:2]
            print(f"  note: fastgrpc2 cannot read proto3 bytes of {type(instance).__name__}: {first}")


def bench_message(descriptor: bytes, message_type: str, instance: BaseModel, number: int) -> None:
    model = type(instance)
    codec = NativeCodec(descriptor, message_type, model)
    check_correctness(descriptor, instance, message_type, codec)

    raw = codec.encode(instance)
    data = codec.decode_dict(raw)
    cache = DescriptorCache(descriptor)
    construct = NativeCodec(descriptor, message_type, model, mode="construct")
    assert construct.decode(raw) == instance, "construct mode changed the value"

    rows: list[tuple[str, float | None, float | None]] = [
        ("native", per_op_us(lambda: codec.encode(instance), number=number),
         per_op_us(lambda: codec.decode(raw), number=number)),
        ("native construct (no valid.)", None, per_op_us(lambda: construct.decode(raw), number=number)),
        ("native (dict, no pydantic)", None, per_op_us(lambda: codec.decode_dict(raw), number=number)),
    ]
    if HAS_FASTGRPC2:
        ser = fastgrpc2_codec.serializer(model)
        de = fastgrpc2_codec.deserializer(model)
        legacy = ser(instance)  # fastgrpc2 decodes its own (explicit-presence) bytes
        rows.append(("fastgrpc2", per_op_us(lambda: ser(instance), number=number),
                     per_op_us(lambda: de(legacy), number=number)))
    if HAS_GOOGLE_PROTOBUF:
        cls, upb_encode, upb_decode = upb_pydantic_pair(descriptor, message_type, model)
        rows.append(("upb + pydantic", per_op_us(lambda: upb_encode(instance), number=number),
                     per_op_us(lambda: upb_decode(raw), number=number)))
    rows.append(("protoruf JSON", per_op_us(lambda: cache.pydantic_to_protobuf(instance, message_type), number=number),
                 fails_or_us(lambda: cache.protobuf_to_pydantic(raw, model, message_type), number=number)))
    if HAS_GOOGLE_PROTOBUF:
        msg = cls.FromString(raw)
        rows.append(("upb raw (no pydantic)", per_op_us(msg.SerializeToString, number=number),
                     per_op_us(lambda: cls.FromString(raw), number=number)))
    rows.append(("validate only", None, per_op_us(lambda: model.model_validate(data), number=number)))

    native_enc, native_dec = rows[0][1], rows[0][2]
    print(f"\n{message_type} — {len(raw)} bytes on the wire, {number:,} ops x 7")
    print(f"  {'codec':<28}{'encode µs':>11}{'vs native':>11}{'decode µs':>11}{'vs native':>11}")
    for name, enc, dec in rows:
        def cell(value: float | None, ref: float | None) -> str:
            if value is None:
                return f"{'—':>11}{'':>11}"
            if value == FAILS:
                return f"{'fails':>11}{'':>11}"
            return f"{value:>11.2f}{value / ref:>10.1f}x"
        print(f"  {name:<28}{cell(enc, native_enc)}{cell(dec, native_dec)}")


def main() -> None:
    descriptor = compile_proto(PROTO)
    print(f"Python {sys.version.split()[0]} — fastgrpc2: {HAS_FASTGRPC2} — google.protobuf: {HAS_GOOGLE_PROTOBUF}")
    print("'vs native' > 1x means slower than the native codec.")
    bench_message(descriptor, "bench.Small", Small(id="abc", value=42), number=200_000)
    bench_message(descriptor, "bench.Order", make_order(), number=50_000)


if __name__ == "__main__":
    main()
