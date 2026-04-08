"""Rust JSON Protobuf - Python bindings for JSON/Protobuf conversion."""

from typing import overload

from . import _protoruf
from .compiler import compile_proto, load_descriptor

__all__ = [
    "json_to_protobuf",
    "protobuf_to_json",
    "compile_proto",
    "load_descriptor",
]
__version__ = "0.1.0"


def json_to_protobuf(
    json_str: str,
    descriptor_bytes: bytes,
    message_type: str,
) -> bytes:
    """
    Convert a JSON string to a Protobuf message.

    Args:
        json_str: JSON string to convert
        descriptor_bytes: Compiled protobuf descriptor set (from compile_proto or load_descriptor)
        message_type: Full message type name (e.g., "user.User", "ecommerce.Order")

    Returns:
        Protobuf message as bytes

    Raises:
        ValueError: If JSON is invalid or message type is not found in descriptor
    """
    return _protoruf.json_to_protobuf(json_str, descriptor_bytes, message_type)


def protobuf_to_json(
    protobuf_bytes: bytes,
    descriptor_bytes: bytes,
    pretty: bool = False,
    message_type: str = None,  # type: ignore[assignment]
) -> str:
    """
    Convert a Protobuf message to a JSON string.

    Args:
        protobuf_bytes: Protobuf message as bytes
        descriptor_bytes: Compiled protobuf descriptor set (from compile_proto or load_descriptor)
        pretty: If True, format JSON with indentation
        message_type: Full message type name (e.g., "user.User", "ecommerce.Order")

    Returns:
        JSON string representation

    Raises:
        RuntimeError: If decoding or JSON serialization fails
    """
    return _protoruf.protobuf_to_json(
        protobuf_bytes, descriptor_bytes, pretty, message_type
    )
