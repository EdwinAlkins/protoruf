"""Tests for protoruf."""

import json
from pathlib import Path

from protoruf import (
    compile_proto,
    json_to_protobuf,
    load_descriptor,
    protobuf_to_json,
    protobuf_to_pydantic,
    pydantic_to_protobuf
)
from tests.test_models import Message

# Compile proto file once for tests
PROTO_PATH = Path(__file__).parent / "proto" / "message.proto"
DESC_PATH = Path(__file__).parent / "proto" / "message.desc"
MESSAGE_TYPE = "message.Message"


def get_descriptor() -> bytes:
    """Get compiled descriptor, compiling if needed."""
    if not DESC_PATH.exists() or DESC_PATH.stat().st_size == 0:
        compile_proto(PROTO_PATH, output_path=DESC_PATH)
    return load_descriptor(DESC_PATH)


def test_json_to_protobuf_to_json():
    """Test round-trip conversion."""
    descriptor = get_descriptor()

    json_data = json.dumps(
        {
            "id": "123",
            "content": "Hello World",
            "priority": 5,
            "tags": ["test", "example"],
            "metadata": {
                "author": "Alice",
                "created_at": 1234567890,
                "attributes": {"env": "prod", "version": "1.0"},
            },
        }
    )

    # JSON -> Protobuf
    protobuf_bytes = json_to_protobuf(json_data, descriptor, MESSAGE_TYPE)
    assert isinstance(protobuf_bytes, bytes)
    assert len(protobuf_bytes) > 0

    # Protobuf -> JSON
    result_json = protobuf_to_json(protobuf_bytes, descriptor, message_type=MESSAGE_TYPE)
    result_data = json.loads(result_json)

    assert result_data["id"] == "123"
    assert result_data["content"] == "Hello World"
    assert result_data["priority"] == 5
    assert result_data["tags"] == ["test", "example"]
    assert result_data["metadata"]["author"] == "Alice"


def test_protobuf_to_json_pretty():
    """Test pretty-printed JSON output."""
    descriptor = get_descriptor()

    json_data = json.dumps({"id": "1", "content": "test"})
    protobuf_bytes = json_to_protobuf(json_data, descriptor, MESSAGE_TYPE)

    pretty_json = protobuf_to_json(
        protobuf_bytes, descriptor, pretty=True, message_type=MESSAGE_TYPE
    )
    assert "\n" in pretty_json


def test_pydantic_integration():
    """Test with Pydantic models."""
    descriptor = get_descriptor()

    msg = Message(
        id="456",
        content="Pydantic test",
        priority=3,
        tags=["pydantic"],
        metadata={"author": "Bob", "created_at": 9876543210},
    )

    json_data = msg.model_dump_json()
    protobuf_bytes = json_to_protobuf(json_data, descriptor, MESSAGE_TYPE)

    result_json = protobuf_to_json(protobuf_bytes, descriptor, message_type=MESSAGE_TYPE)
    result_data = json.loads(result_json)

    assert result_data["id"] == "456"
    assert result_data["content"] == "Pydantic test"


def test_compile_proto():
    """Test proto compilation."""
    descriptor = compile_proto(PROTO_PATH)
    assert isinstance(descriptor, bytes)
    assert len(descriptor) > 0


def test_pydantic_to_protobuf():
    """Test pydantic to protobuf conversion."""
    descriptor = get_descriptor()

    msg = Message(
        id="456",
        content="Pydantic test",
        priority=3,
        tags=["pydantic"],
        metadata={"author": "Bob", "created_at": 9876543210},
    )
    protobuf_bytes = pydantic_to_protobuf(msg, descriptor, MESSAGE_TYPE)
    assert isinstance(protobuf_bytes, bytes)
    assert len(protobuf_bytes) > 0

    result_json = protobuf_to_json(protobuf_bytes, descriptor, message_type=MESSAGE_TYPE)
    result_data = json.loads(result_json)

    assert result_data["id"] == "456"
    assert result_data["content"] == "Pydantic test"
    assert result_data["priority"] == 3
    assert result_data["tags"] == ["pydantic"]
    assert result_data["metadata"]["author"] == "Bob"
    assert result_data["metadata"]["created_at"] == 9876543210


def test_protobuf_to_pydantic():
    """Test protobuf to pydantic conversion."""
    descriptor = get_descriptor()

    msg = Message(
        id="456",
        content="Pydantic test",
        priority=3,
        tags=["pydantic"],
        metadata={"author": "Bob", "created_at": 9876543210},
    )
    json_data = msg.model_dump_json()
    protobuf_bytes = json_to_protobuf(json_data, descriptor, MESSAGE_TYPE)

    result_msg = protobuf_to_pydantic(protobuf_bytes, descriptor, Message, MESSAGE_TYPE)
    assert result_msg.id == "456"
    assert result_msg.content == "Pydantic test"
    assert result_msg.priority == 3
    assert result_msg.tags == ["pydantic"]
    assert result_msg.metadata.author == "Bob"
    assert result_msg.metadata.created_at == 9876543210
