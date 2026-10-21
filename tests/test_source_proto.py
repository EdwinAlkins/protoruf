import json

from protoruf import compile_proto_from_sources, json_to_protobuf, protobuf_to_json

MESSAGE_TYPE = "user.User"


def test_compile_proto_from_sources():
    files = {
        "user.proto": """
syntax = "proto3";
package user;
message User { string id = 1; string email = 2; }
""",
    }
    descriptor = compile_proto_from_sources(files, root="user.proto")
    assert len(descriptor) > 0

    json_data = json.dumps({"id": "1", "email": "a@b.c"})
    protobuf_bytes = json_to_protobuf(json_data, descriptor, MESSAGE_TYPE)
    assert isinstance(protobuf_bytes, bytes)
    assert len(protobuf_bytes) > 0

    json_str = protobuf_to_json(
        protobuf_bytes, descriptor, message_type=MESSAGE_TYPE
    )
    assert json.loads(json_str)["id"] == "1"
    assert json.loads(json_str)["email"] == "a@b.c"