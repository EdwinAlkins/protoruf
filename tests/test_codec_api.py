"""The 0.5 codec API: `Schema.codec`, field names, oneofs, merging, dates, floats."""

import datetime
import math
import timeit

import pytest
from pydantic import BaseModel, Field

import protoruf
from protoruf import Codec, Schema, compile_protos

PROTO = """
syntax = "proto3";
package shop;
import "google/type/date.proto";

message Card {
  string number = 1;
  repeated string tags = 2;
  Card backup = 3;
}

message Order {
  string from = 1;
  oneof payment {
    string iban = 2;
    Card card = 3;
  }
  google.type.Date due = 4;
  Card main = 5;
  repeated google.type.Date holidays = 6;
  double ratio = 7;
  float scale = 8;
}
"""


@pytest.fixture(scope="module")
def schema() -> Schema:
    return Schema(compile_protos(["shop.proto"], sources={"shop.proto": PROTO}))


class Card(BaseModel):
    number: str = ""
    tags: list[str] = []
    backup: "Card | None" = None


class Order(BaseModel):
    from_: str = Field(default="", alias="From")
    iban: str | None = None
    card: Card | None = None
    due: datetime.date | None = None
    main: Card | None = None
    holidays: list[datetime.date] = []
    ratio: float = 0.0
    scale: float = 0.0


NAMES = {"shop.Order": {1: "from_"}}


def test_schema_codec_is_the_public_codec(schema: Schema) -> None:
    codec = schema.codec("shop.Order", Order, field_names=NAMES)
    assert isinstance(codec, Codec)
    assert protoruf._protoruf.NativeCodec is Codec  # the 0.3 / 0.4 name still works
    with pytest.raises(KeyError, match="shop.Missing"):
        schema.codec("shop.Missing")


@pytest.mark.parametrize("mode", ["validate", "construct", "dict"])
def test_python_names_and_aliases(schema: Schema, mode: str) -> None:
    codec = schema.codec("shop.Order", Order, mode=mode, field_names=NAMES)  # type: ignore[arg-type]
    order = Order.model_validate({"From": "Paris", "iban": "FR76"})
    data = codec.encode(order)
    # On the wire, the field keeps its proto number (1 = "from").
    plain = schema.codec("shop.Order", mode="dict").decode(data)
    assert (plain["from"], plain["iban"]) == ("Paris", "FR76")
    decoded = codec.decode(data)
    if mode == "dict":
        assert decoded["from_"] == "Paris"
    else:
        assert decoded == order


def test_dicts_without_names_use_proto_names(schema: Schema) -> None:
    codec = schema.codec("shop.Order")
    decoded = codec.decode(codec.encode({"from": "x", "card": {"number": "1"}}))
    assert decoded["from"] == "x"
    assert decoded["card"] == {"number": "1", "tags": []}
    assert "iban" not in decoded


def test_names_of_nested_messages(schema: Schema) -> None:
    class Renamed(BaseModel):
        id: str = ""
        labels: list[str] = []

    class Holder(BaseModel):
        main: Renamed | None = None

    names = {"shop.Card": {1: "id", 2: "labels"}}
    codec = schema.codec("shop.Order", Holder, field_names=names)
    data = codec.encode(Holder(main=Renamed(id="7", labels=["a"])))
    assert codec.decode(data) == Holder(main=Renamed(id="7", labels=["a"]))
    construct = schema.codec("shop.Order", Holder, mode="construct", field_names=names)
    assert construct.decode(data) == Holder(main=Renamed(id="7", labels=["a"]))


def test_oneof_refuses_two_members_on_encode(schema: Schema) -> None:
    codec = schema.codec("shop.Order", Order, field_names=NAMES)
    with pytest.raises(ValueError, match="oneof payment has several fields set: iban, card"):
        codec.encode(Order(iban="FR76", card=Card(number="4")))
    with pytest.raises(ValueError, match="several fields set"):
        schema.codec("shop.Order").encode({"iban": "FR76", "card": {}})
    codec.encode(Order(iban="FR76", card=None))  # None is unset


def test_oneof_last_member_wins_on_decode(schema: Schema) -> None:
    codec = schema.codec("shop.Order", Order, field_names=NAMES)
    iban = bytes.fromhex("1204") + b"FR76"
    card = bytes.fromhex("1a03") + bytes.fromhex("0a01") + b"4"
    assert codec.decode(iban + card).iban is None
    assert codec.decode(iban + card).card == Card(number="4")
    assert codec.decode(card + iban) == Order(iban="FR76")


def test_repeated_singular_messages_are_merged(schema: Schema) -> None:
    """A message field met twice is the merge of both: scalars from the last,
    repeated fields concatenated, sub-messages merged recursively."""
    codec = schema.codec("shop.Card", Card)
    first = codec.encode(Card(number="1", tags=["a"], backup=Card(number="b1", tags=["x"])))
    second = codec.encode(Card(number="2", tags=["b"], backup=Card(tags=["y"])))
    order = schema.codec("shop.Order", Order, field_names=NAMES)
    main = lambda payload: bytes.fromhex("2a") + bytes([len(payload)]) + payload  # noqa: E731
    merged = order.decode(main(first) + main(second)).main
    assert merged == Card(number="2", tags=["a", "b"], backup=Card(number="b1", tags=["x", "y"]))


def test_merging_is_linear_in_the_number_of_occurrences(schema: Schema) -> None:
    """Merging used to decode the concatenation again at each occurrence (quadratic)."""
    codec = schema.codec("shop.Order", Order, field_names=NAMES)
    occurrence = bytes.fromhex("2a03") + bytes.fromhex("1201") + b"t"  # main.tags += ["t"]

    def best_time(n: int) -> float:
        data = occurrence * n
        assert len(codec.decode(data).main.tags) == n
        return min(timeit.repeat(lambda: codec.decode(data), number=1, repeat=3))

    # Linear: about 8x; quadratic was about 64x.
    assert best_time(16_000) < 24 * best_time(2_000)


def test_merging_with_oneof_keeps_only_the_last_member(schema: Schema) -> None:
    codec = schema.codec("shop.Order", Order, field_names=NAMES)
    card = lambda payload: bytes.fromhex("1a") + bytes([len(payload)]) + payload  # noqa: E731
    iban = bytes.fromhex("1204") + b"FR76"
    first, second = card(bytes.fromhex("0a01") + b"1"), card(bytes.fromhex("1201") + b"a")
    assert codec.decode(first + second).card == Card(number="1", tags=["a"])
    assert codec.decode(first + iban + second) == Order(card=Card(tags=["a"]))
    assert codec.decode(first + second + iban) == Order(iban="FR76")


@pytest.mark.parametrize(
    "data",
    [
        b"\x00\x01",  # field number 0
        bytes.fromhex("888080808001") + b"\x05",  # 2**32 + 1, used to be read as field 1
        bytes.fromhex("8080808010") + b"\x01",  # 2**29: above the maximum
        b"\x2b\x2c",  # group (wire types 3 and 4)
        b"\x0e",  # wire type 6
    ],
)
def test_invalid_tags_are_refused(schema: Schema, data: bytes) -> None:
    codec = schema.codec("shop.Order", Order, field_names=NAMES)
    with pytest.raises(ValueError, match="invalid field number|unsupported wire type"):
        codec.decode(data)


def test_dates(schema: Schema) -> None:
    codec = schema.codec("shop.Order", Order, field_names=NAMES)
    order = Order(due=datetime.date(2024, 2, 29), holidays=[datetime.date(1, 1, 1)])
    data = codec.encode(order)
    assert bytes.fromhex("220708e80f1002181d") in data  # year 2024, month 2, day 29
    assert codec.decode(data) == order
    plain = schema.codec("shop.Order")
    with pytest.raises(TypeError, match="expected datetime.date, got datetime"):
        plain.encode({"due": datetime.datetime(2024, 1, 1)})
    # The dict form of protoruf 0.4 is still accepted on encode.
    as_dict = {"due": {"year": 2024, "month": 2, "day": 29}}
    assert plain.encode(as_dict) == plain.encode({"due": datetime.date(2024, 2, 29)})
    with pytest.raises(ValueError, match="invalid or partial date 2024-0-0"):
        plain.decode(bytes.fromhex("22030" "8e80f"))  # year only


@pytest.mark.parametrize("value", [math.inf, -math.inf, -0.0, 1e-310])
def test_special_floats_round_trip(schema: Schema, value: float) -> None:
    codec = schema.codec("shop.Order")
    decoded = codec.decode(codec.encode({"ratio": value}))["ratio"]
    assert decoded == value
    assert math.copysign(1.0, decoded) == math.copysign(1.0, value)


def test_nan_and_negative_zero_are_written(schema: Schema) -> None:
    codec = schema.codec("shop.Order")
    assert math.isnan(codec.decode(codec.encode({"ratio": math.nan}))["ratio"])
    # -0.0 is not the default value: it is sent, as upb does.
    assert codec.encode({"ratio": -0.0}) == bytes.fromhex("390000000000000080")
    assert codec.encode({"ratio": 0.0}) == b""
    assert math.isnan(codec.decode(codec.encode({"scale": math.nan}))["scale"])


def test_unknown_mode(schema: Schema) -> None:
    with pytest.raises(ValueError, match="'validate', 'construct' or 'dict'"):
        schema.codec("shop.Order", mode="fast")  # type: ignore[arg-type]
