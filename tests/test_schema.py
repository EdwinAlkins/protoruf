"""Tests for compile_protos and Schema (compiled schema introspection)."""

from __future__ import annotations

from pathlib import Path

import pytest

from protoruf import Schema, compile_protos
from protoruf._protoruf import NativeCodec

PROTO_DIR = Path(__file__).parent / "proto"

ORDERS = """syntax = "proto3";
package shop.v1;

import "google/protobuf/timestamp.proto";
import "google/rpc/status.proto";
import "shop/v1/common.proto";

// An order placed by a customer.
message Order {
  // Line of an order.
  message Line {
    string sku = 1;
    uint32 qty = 2;
  }
  enum Kind {
    KIND_UNSPECIFIED = 0;
    KIND_GIFT = 1 [deprecated = true];
  }
  reserved 9, 15 to 20;
  reserved "legacy";

  string id = 1 [json_name = "orderId"]; // Public identifier.
  optional string note = 2;
  oneof payment {
    string card = 3;
    string iban = 4;
  }
  map<string, Line> lines = 5;
  repeated Kind kinds = 6;
  google.protobuf.Timestamp at = 7 [deprecated = true];
  Money total = 8;
  google.rpc.Status last_error = 10;
}

// Order management.
service Orders {
  // Fetch one order.
  rpc GetOrder(Order) returns (Order);
  // Follow an order.
  rpc Watch(Order) returns (stream Order);
}
"""

COMMON = """syntax = "proto3";
package shop.v1;
message Money { int64 cents = 1; string currency = 2; }
"""

SOURCES = {"shop/v1/orders.proto": ORDERS, "shop/v1/common.proto": COMMON}


@pytest.fixture(scope="module")
def schema() -> Schema:
    descriptor = compile_protos(["shop/v1/orders.proto"], sources=SOURCES, source_info=True)
    return Schema(descriptor)


# ------------------------------------------------------------------ compile_protos


def test_compile_protos_mixes_sources_disk_and_standard_protos() -> None:
    generated = """syntax = "proto3"; package gen;
        import "message.proto";
        import "google/rpc/error_details.proto";
        message Envelope { .message.Message payload = 1; google.rpc.BadRequest error = 2; }"""
    schema = Schema(
        compile_protos(["gen.proto"], [PROTO_DIR], {"gen.proto": generated})
    )
    assert {"gen.proto", "message.proto", "google/rpc/error_details.proto"} <= set(schema.files)
    assert schema.message("gen.Envelope").field("error").type_name == "google.rpc.BadRequest"


def test_compile_protos_several_roots_and_disk_paths() -> None:
    schema = Schema(compile_protos([PROTO_DIR / "message.proto"], [PROTO_DIR]))
    assert list(schema.files) == ["message.proto"]
    both = Schema(
        compile_protos(["shop/v1/common.proto", "shop/v1/orders.proto"], sources=SOURCES)
    )
    assert {"shop/v1/common.proto", "shop/v1/orders.proto"} <= set(both.files)


def test_compile_protos_options() -> None:
    plain = compile_protos(["shop/v1/orders.proto"], sources=SOURCES)
    with_info = compile_protos(["shop/v1/orders.proto"], sources=SOURCES, source_info=True)
    assert len(with_info) > len(plain)
    assert Schema(plain).message("shop.v1.Order").comments is None

    roots_only = Schema(
        compile_protos(["shop/v1/common.proto"], sources=SOURCES, include_imports=False)
    )
    assert list(roots_only.files) == ["shop/v1/common.proto"]

    with pytest.raises(RuntimeError, match=r"shop/v1/orders\.proto:5:1: .*google/rpc/status"):
        compile_protos(["shop/v1/orders.proto"], sources=SOURCES, standard_protos=False)


def test_compile_protos_errors_carry_the_location(tmp_path: Path) -> None:
    (tmp_path / "bad.proto").write_text('syntax = "proto3";\nmessage A {\n  Nope x = 1;\n}\n')
    with pytest.raises(RuntimeError, match=r"bad\.proto:3:3: name 'Nope' is not defined"):
        compile_protos(["bad.proto"], [tmp_path])
    with pytest.raises(RuntimeError, match="no root file"):
        compile_protos([])


def test_compile_protos_writes_output(tmp_path: Path) -> None:
    out = tmp_path / "out" / "shop.desc"
    descriptor = compile_protos(["shop/v1/common.proto"], sources=SOURCES, output_path=out)
    assert out.read_bytes() == descriptor


def test_every_standard_proto_compiles() -> None:
    roots = [
        "google/rpc/status.proto",
        "google/rpc/code.proto",
        "google/rpc/error_details.proto",
        "google/type/date.proto",
        "google/type/datetime.proto",
        "google/type/timeofday.proto",
        "google/type/latlng.proto",
        "google/type/money.proto",
        "grpc/health/v1/health.proto",
        "grpc/reflection/v1/reflection.proto",
        "grpc/reflection/v1alpha/reflection.proto",
    ]
    schema = Schema(compile_protos(roots))
    watch = schema.service("grpc.health.v1.Health").method("Watch")
    assert watch.server_streaming and not watch.client_streaming
    assert "google.type.Date" in schema.messages


# ---------------------------------------------------------------------- Schema


def test_files(schema: Schema) -> None:
    orders = schema.file("shop/v1/orders.proto")
    assert orders.package == "shop.v1"
    assert orders.syntax == "proto3"
    assert orders.dependencies == (
        "google/protobuf/timestamp.proto",
        "google/rpc/status.proto",
        "shop/v1/common.proto",
    )
    assert orders.messages == ("shop.v1.Order",)
    assert orders.services == ("shop.v1.Orders",)


def test_messages_and_fields(schema: Schema) -> None:
    order = schema.message("shop.v1.Order")
    assert [f.name for f in order.fields] == [
        "id", "note", "card", "iban", "lines", "kinds", "at", "total", "last_error",
    ]
    assert order.reserved_ranges == ((9, 9), (15, 20))
    assert order.reserved_names == ("legacy",)
    assert [(o.name, o.fields) for o in order.oneofs] == [("payment", ("card", "iban"))]
    assert order.comments is not None and order.comments.text == "An order placed by a customer."

    field_id = order.field("id")
    assert (field_id.number, field_id.kind, field_id.json_name) == (1, "string", "orderId")
    assert field_id.comments is not None and field_id.comments.text == "Public identifier."
    note = order.field("note")
    assert note.has_presence and note.proto3_optional and note.oneof is None
    assert order.field("card").oneof == "payment"
    assert not order.field("id").has_presence

    lines = order.field("lines")
    assert lines.is_map and not lines.is_list
    assert (lines.map_key_kind, lines.map_value_kind) == ("string", "message")
    assert lines.map_value_type_name == "shop.v1.Order.Line"
    kinds = order.field("kinds")
    assert kinds.is_list and kinds.is_packed and kinds.type_name == "shop.v1.Order.Kind"
    assert order.field("at").deprecated
    assert order.field("total").type_name == "shop.v1.Money"

    line = schema.message("shop.v1.Order.Line")
    assert line.parent == "shop.v1.Order" and line.field("qty").kind == "uint32"
    assert schema.message("shop.v1.Order.LinesEntry").is_map_entry


def test_enums_and_services(schema: Schema) -> None:
    kind = schema.enum("shop.v1.Order.Kind")
    assert kind.parent == "shop.v1.Order"
    assert [(v.name, v.number, v.deprecated) for v in kind.values] == [
        ("KIND_UNSPECIFIED", 0, False),
        ("KIND_GIFT", 1, True),
    ]
    orders = schema.service("shop.v1.Orders")
    assert orders.comments is not None and orders.comments.text == "Order management."
    watch = orders.method("Watch")
    assert watch.full_name == "shop.v1.Orders.Watch"
    assert (watch.input_type, watch.output_type) == ("shop.v1.Order", "shop.v1.Order")
    assert watch.server_streaming and not watch.client_streaming
    assert watch.comments is not None and watch.comments.text == "Follow an order."


def test_lookups_fail_with_clear_errors(schema: Schema) -> None:
    with pytest.raises(KeyError, match="no message named 'shop.v1.Nope'"):
        schema.message("shop.v1.Nope")
    with pytest.raises(KeyError, match="has no field 'nope'"):
        schema.message("shop.v1.Order").field("nope")
    with pytest.raises(KeyError, match="has no method 'Nope'"):
        schema.service("shop.v1.Orders").method("Nope")


def test_reflection_helpers(schema: Schema) -> None:
    for symbol in ("shop.v1.Order", "shop.v1.Order.Line", "shop.v1.Order.Kind",
                   "shop.v1.Orders", "shop.v1.Orders.Watch"):
        assert schema.file_containing_symbol(symbol) == "shop/v1/orders.proto", symbol
    assert schema.file_containing_symbol("shop.v1.Money") == "shop/v1/common.proto"
    assert schema.file_containing_symbol("shop.v1.Nope") is None

    assert schema.file_dependencies("shop/v1/orders.proto") == (
        "google/protobuf/timestamp.proto",
        "google/protobuf/any.proto",
        "google/rpc/status.proto",
        "shop/v1/common.proto",
    )
    with pytest.raises(KeyError, match="no file named"):
        schema.file_descriptor_proto("nope.proto")


def test_file_descriptor_protos_match_the_set(schema: Schema) -> None:
    pytest.importorskip("google.protobuf")
    from google.protobuf.descriptor_pb2 import FileDescriptorProto, FileDescriptorSet

    expected = {f.name: f for f in FileDescriptorSet.FromString(schema.descriptor).file}
    for name in schema.files:
        proto = FileDescriptorProto.FromString(schema.file_descriptor_proto(name))
        assert proto == expected[name]


def test_schema_works_with_native_codec(schema: Schema) -> None:
    codec = NativeCodec(schema.descriptor, "shop.v1.Money")
    assert codec.decode_dict(codec.encode({"cents": 150, "currency": "EUR"})) == {
        "cents": 150,
        "currency": "EUR",
    }


def test_comments_text() -> None:
    from protoruf.schema import Comments

    assert Comments(" Line one.\n Line two.\n", None, ()).text == "Line one.\nLine two."
    assert Comments(None, " Trailing.\n", ()).text == "Trailing."
    assert Comments(None, None, (" Detached.\n",)).text is None
