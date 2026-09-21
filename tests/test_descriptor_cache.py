"""Tests for the DescriptorCache class (cached descriptor pool)."""

import json
from pathlib import Path

import pytest

from protoruf import (
    DescriptorCache,
    compile_proto,
    json_to_protobuf,
    load_descriptor,
    protobuf_to_json,
)

from tests.test_models import Message

PROTO_PATH = Path(__file__).parent / "proto" / "message.proto"
DESC_PATH = Path(__file__).parent / "proto" / "message.desc"
MESSAGE_TYPE = "message.Message"


def get_descriptor() -> bytes:
    """Get compiled descriptor, compiling if needed."""
    if not DESC_PATH.exists() or DESC_PATH.stat().st_size == 0:
        compile_proto(PROTO_PATH, output_path=DESC_PATH)
    return load_descriptor(DESC_PATH)


SAMPLE = {
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


def test_cache_roundtrip():
    """A cache instance round-trips JSON <-> protobuf correctly."""
    cache = DescriptorCache(get_descriptor())
    json_data = json.dumps(SAMPLE)

    protobuf_bytes = cache.json_to_protobuf(json_data, MESSAGE_TYPE)
    assert isinstance(protobuf_bytes, bytes)
    assert len(protobuf_bytes) > 0

    result = json.loads(cache.protobuf_to_json(protobuf_bytes, MESSAGE_TYPE))
    assert result == SAMPLE


def test_cache_matches_free_functions():
    """The cache produces output equivalent to the free functions.

    Compared semantically (decoded JSON), since protobuf map field encoding has
    no defined ordering and raw bytes may differ between two encodings.
    """
    descriptor = get_descriptor()
    cache = DescriptorCache(descriptor)
    json_data = json.dumps(SAMPLE)

    pb_cache = cache.json_to_protobuf(json_data, MESSAGE_TYPE)
    pb_free = json_to_protobuf(json_data, descriptor, MESSAGE_TYPE)

    assert json.loads(cache.protobuf_to_json(pb_cache, MESSAGE_TYPE)) == json.loads(
        protobuf_to_json(pb_free, descriptor, message_type=MESSAGE_TYPE)
    )


def test_cache_reuse_across_many_calls():
    """The same cache instance can be reused for many conversions."""
    cache = DescriptorCache(get_descriptor())
    json_data = json.dumps(SAMPLE)

    for _ in range(100):
        protobuf_bytes = cache.json_to_protobuf(json_data, MESSAGE_TYPE)
        result = json.loads(cache.protobuf_to_json(protobuf_bytes, MESSAGE_TYPE))
        assert result == SAMPLE


def test_cache_pretty():
    """Pretty output adds newlines."""
    cache = DescriptorCache(get_descriptor())
    protobuf_bytes = cache.json_to_protobuf(json.dumps(SAMPLE), MESSAGE_TYPE)

    compact = cache.protobuf_to_json(protobuf_bytes, MESSAGE_TYPE)
    pretty = cache.protobuf_to_json(protobuf_bytes, MESSAGE_TYPE, pretty=True)
    assert "\n" not in compact
    assert "\n" in pretty
    assert json.loads(compact) == json.loads(pretty)


def test_cache_map_field_roundtrip():
    """Map fields survive the round-trip (regression for the map bug)."""
    cache = DescriptorCache(get_descriptor())
    data = {"id": "m", "metadata": {"attributes": {"a": "1", "b": "2"}}}

    protobuf_bytes = cache.json_to_protobuf(json.dumps(data), MESSAGE_TYPE)
    result = json.loads(cache.protobuf_to_json(protobuf_bytes, MESSAGE_TYPE))
    assert result["metadata"]["attributes"] == {"a": "1", "b": "2"}


def test_cache_invalid_descriptor():
    """Building a cache from invalid descriptor bytes raises."""
    with pytest.raises(ValueError):
        DescriptorCache(b"not a descriptor")


def test_cache_unknown_message_type():
    """An unknown message type raises when used."""
    cache = DescriptorCache(get_descriptor())
    with pytest.raises(ValueError):
        cache.json_to_protobuf(json.dumps(SAMPLE), "message.DoesNotExist")


def test_cache_invalid_json():
    """Invalid JSON raises a ValueError."""
    cache = DescriptorCache(get_descriptor())
    with pytest.raises(ValueError):
        cache.json_to_protobuf("not valid json", MESSAGE_TYPE)


def test_cache_pydantic_to_protobuf():
    """A cache converts a Pydantic model to protobuf bytes."""
    cache = DescriptorCache(get_descriptor())
    msg = Message(
        id="456",
        content="Pydantic test",
        priority=3,
        tags=["pydantic"],
        metadata={"author": "Bob", "created_at": 9876543210},
    )

    protobuf_bytes = cache.pydantic_to_protobuf(msg, MESSAGE_TYPE)
    assert isinstance(protobuf_bytes, bytes)
    assert len(protobuf_bytes) > 0

    result = json.loads(cache.protobuf_to_json(protobuf_bytes, MESSAGE_TYPE))
    assert result["id"] == "456"
    assert result["content"] == "Pydantic test"
    assert result["priority"] == 3
    assert result["tags"] == ["pydantic"]
    assert result["metadata"]["author"] == "Bob"
    assert result["metadata"]["created_at"] == 9876543210


def test_cache_protobuf_to_pydantic():
    """A cache converts protobuf bytes back into a Pydantic model instance."""
    cache = DescriptorCache(get_descriptor())
    msg = Message(
        id="456",
        content="Pydantic test",
        priority=3,
        tags=["pydantic"],
        metadata={"author": "Bob", "created_at": 9876543210},
    )
    protobuf_bytes = cache.pydantic_to_protobuf(msg, MESSAGE_TYPE)

    result = cache.protobuf_to_pydantic(protobuf_bytes, Message, MESSAGE_TYPE)
    assert isinstance(result, Message)
    assert result.id == "456"
    assert result.content == "Pydantic test"
    assert result.priority == 3
    assert result.tags == ["pydantic"]
    assert result.metadata.author == "Bob"
    assert result.metadata.created_at == 9876543210


def test_cache_pydantic_roundtrip():
    """A Pydantic model survives a full round-trip through the cache."""
    cache = DescriptorCache(get_descriptor())
    msg = Message(**SAMPLE)

    protobuf_bytes = cache.pydantic_to_protobuf(msg, MESSAGE_TYPE)
    result = cache.protobuf_to_pydantic(protobuf_bytes, Message, MESSAGE_TYPE)
    assert result == msg
