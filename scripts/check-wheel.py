"""Smoke-test an installed wheel, including its Python typing files."""

import json
from importlib import resources

import protoruf

package = resources.files("protoruf")
for name in ("py.typed", "_protoruf.pyi"):
    if not package.joinpath(name).is_file():
        raise SystemExit(f"wheel is missing protoruf/{name}")

descriptor = protoruf.compile_proto_from_sources(
    {"smoke.proto": 'syntax = "proto3"; package smoke; message Item { string id = 1; }'},
    "smoke.proto",
)
wire = protoruf.json_to_protobuf('{"id":"wheel"}', descriptor, "smoke.Item")
restored = json.loads(protoruf.protobuf_to_json(wire, descriptor, "smoke.Item"))
if restored["id"] != "wheel":
    raise SystemExit("wheel round-trip failed")

print(f"Wheel smoke test passed: protoruf {protoruf.__version__}")
