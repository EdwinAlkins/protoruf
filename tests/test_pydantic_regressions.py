"""Regression tests for the Pydantic helpers.

1. `bytes` fields: Pydantic dumps/validates them as UTF-8 text by default, while the
   protobuf JSON mapping uses base64 -> arbitrary binary data used to fail or be
   silently corrupted.
2. Default values: protobuf JSON output omits zero values, so a required Pydantic field
   holding a zero (`False`, `0`, `""`) came back missing.
"""

from __future__ import annotations

import datetime as dt
import json

import pytest
from pydantic import BaseModel, ConfigDict

from protoruf import (
    DescriptorCache,
    compile_proto_from_sources,
    json_to_protobuf,
    protobuf_to_json,
    protobuf_to_pydantic,
    pydantic_to_protobuf,
)
from protoruf._pydantic import _plan

SOURCE = """
syntax = "proto3";
package p;
import "google/protobuf/timestamp.proto";

message Blob { bytes data = 1; }

message Doc {
  string id = 1;
  bool flag = 2;
  int64 count = 3;
  bytes raw = 4;
  Blob blob = 5;
  repeated bytes chunks = 6;
  map<string, bytes> files = 7;
  optional int32 maybe = 8;
  optional bytes maybe_raw = 9;
  google.protobuf.Timestamp at = 10;
  repeated Blob blobs = 11;
}

message Node { bytes data = 1; Node next = 2; }

message Plain { string id = 1; bool flag = 2; int64 count = 3; google.protobuf.Timestamp at = 4; }

message Tree { google.protobuf.Timestamp when = 1; Tree child = 2; }

message Outer { Inner inner = 1; }
message Inner { Outer back = 1; bytes data = 2; }
"""

DESCRIPTOR = compile_proto_from_sources({"p.proto": SOURCE}, root="p.proto")
BINARY = b"\x00\xff\xfe\x80 binary"


class Blob(BaseModel):
    data: bytes


class Doc(BaseModel):
    id: str
    flag: bool
    count: int
    raw: bytes
    blob: Blob | None = None
    chunks: list[bytes] = []
    files: dict[str, bytes] = {}
    maybe: int | None = None
    maybe_raw: bytes | None = None
    at: dt.datetime | None = None
    blobs: list[Blob] = []


class Node(BaseModel):
    data: bytes
    next: Node | None = None


class Plain(BaseModel):
    id: str
    flag: bool
    count: int
    at: dt.datetime | None = None


class Tree(BaseModel):
    """Recursive, strict, without `bytes`: must keep the JSON path (strict validation of
    a datetime from Python data would refuse the string)."""

    model_config = ConfigDict(strict=True)

    when: dt.datetime
    child: Tree | None = None


class Outer(BaseModel):
    inner: Inner | None = None


class Inner(BaseModel):
    back: Outer | None = None
    data: bytes = b""


Outer.model_rebuild()  # `Inner` is defined after `Outer`


def both_paths(model: BaseModel, message_type: str) -> list[BaseModel]:
    """Round-trip through the free functions and through DescriptorCache."""
    cache = DescriptorCache(DESCRIPTOR)
    free = protobuf_to_pydantic(
        pydantic_to_protobuf(model, DESCRIPTOR, message_type), DESCRIPTOR, type(model), message_type
    )
    cached = cache.protobuf_to_pydantic(
        cache.pydantic_to_protobuf(model, message_type), type(model), message_type
    )
    return [free, cached]


def test_binary_bytes_round_trip_at_every_depth():
    doc = Doc(
        id="d",
        flag=True,
        count=1,
        raw=BINARY,
        blob=Blob(data=BINARY),
        chunks=[BINARY, b""],
        files={"a": BINARY},
        maybe_raw=b"\x01",
        at=dt.datetime(2024, 1, 2, 3, 4, 5, tzinfo=dt.timezone.utc),
        blobs=[Blob(data=b"\xff")],
    )
    for result in both_paths(doc, "p.Doc"):
        assert result == doc


def test_bytes_are_base64_on_the_json_side():
    raw = pydantic_to_protobuf(Blob(data=BINARY), DESCRIPTOR, "p.Blob")
    assert json_to_protobuf('{"data": "AP/+gCBiaW5hcnk="}', DESCRIPTOR, "p.Blob") == raw


def test_required_zero_values_survive():
    doc = Doc(id="", flag=False, count=0, raw=b"")
    for result in both_paths(doc, "p.Doc"):
        assert result == doc


def test_presence_fields_keep_none_and_zero_apart():
    unset = Doc(id="x", flag=True, count=1, raw=b"r")
    zero = unset.model_copy(update={"maybe": 0, "maybe_raw": b""})
    for result in both_paths(unset, "p.Doc"):
        assert result.maybe is None and result.maybe_raw is None and result.blob is None
    for result in both_paths(zero, "p.Doc"):
        assert result.maybe == 0 and result.maybe_raw == b""


def test_recursive_model_with_bytes():
    chain = Node(data=b"\x01", next=Node(data=b"\xff", next=Node(data=b"")))
    for result in both_paths(chain, "p.Node"):
        assert result == chain


def test_model_without_bytes_keeps_json_path():
    plain = Plain(id="", flag=False, count=0, at=dt.datetime(2024, 1, 1, tzinfo=dt.timezone.utc))
    for result in both_paths(plain, "p.Plain"):
        assert result == plain


def test_recursive_model_without_bytes_keeps_json_path():
    at = dt.datetime(2026, 10, 29, tzinfo=dt.timezone.utc)
    tree = Tree(when=at, child=Tree(when=at))
    assert _plan(Tree) == {}
    for result in both_paths(tree, "p.Tree"):
        assert result == tree


def test_mutual_recursion_plans_only_models_with_bytes():
    assert _plan(Outer) == {"inner": ("model", Inner)}
    assert _plan(Inner) == {"back": ("model", Outer), "data": "bytes"}
    outer = Outer(inner=Inner(data=BINARY, back=Outer(inner=Inner(data=b"\xff"))))
    for result in both_paths(outer, "p.Outer"):
        assert result == outer


def test_protobuf_to_json_still_omits_defaults_by_default():
    raw = json_to_protobuf('{"id": "", "flag": false}', DESCRIPTOR, "p.Plain")
    assert json.loads(protobuf_to_json(raw, DESCRIPTOR, "p.Plain")) == {}
    with_defaults = protobuf_to_json(raw, DESCRIPTOR, "p.Plain", include_default_values=True)
    assert json.loads(with_defaults) == {"id": "", "flag": False, "count": 0}
    cache = DescriptorCache(DESCRIPTOR)
    assert json.loads(cache.protobuf_to_json(raw, "p.Plain", include_default_values=True)) == {
        "id": "",
        "flag": False,
        "count": 0,
    }


@pytest.mark.parametrize("value", [b"", b"\x00", bytes(range(256))])
def test_every_byte_value(value):
    for result in both_paths(Blob(data=value), "p.Blob"):
        assert result.data == value
