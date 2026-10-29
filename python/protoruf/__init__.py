"""Rust JSON Protobuf - Python bindings for JSON/Protobuf conversion."""
from typing import Type, TypeVar

from . import _protoruf
from ._protoruf import Codec
from ._native import OpenIntEnum
from ._pydantic import model_from_json, model_to_json
from .compiler import compile_proto, load_descriptor, compile_proto_from_sources, compile_protos
from .schema import Schema

from pydantic import BaseModel


T = TypeVar("T", bound=BaseModel)

__all__ = [
    "json_to_protobuf",
    "protobuf_to_json",
    "compile_proto",
    "load_descriptor",
    "compile_proto_from_sources",
    "compile_protos",
    "protobuf_to_pydantic",
    "pydantic_to_protobuf",
    "Codec",
    "DescriptorCache",
    "OpenIntEnum",
    "Schema",
]
__version__ = "0.5.0"

class DescriptorCache:
    """Pre-decoded descriptor pool.

    Build it once and reuse it across conversions to avoid per-call descriptor
    hashing, the global LRU lock, and repeated message-type lookup. Wraps the
    native Rust cache and adds Pydantic helpers, which are pure-Python
    (de)serialization on top of the JSON conversions.
    """

    def __init__(self, descriptor_bytes: bytes) -> None:
        self._cache = _protoruf.DescriptorCache(descriptor_bytes)

    def json_to_protobuf(self, json_str: str, message_type: str) -> bytes:
        """Convert a JSON string to a Protobuf message (bytes)."""
        return self._cache.json_to_protobuf(json_str, message_type)

    def protobuf_to_json(
        self,
        protobuf_bytes: bytes,
        message_type: str,
        pretty: bool = False,
        include_default_values: bool = False,
    ) -> str:
        """Convert a Protobuf message (bytes) to a JSON string.

        With `include_default_values=True`, fields without presence are emitted
        even at their default value (`0`, `""`, `false`, `[]`, `{}`).
        """
        return self._cache.protobuf_to_json(
            protobuf_bytes, message_type, pretty, include_default_values
        )

    def pydantic_to_protobuf(
        self,
        pydantic_model: BaseModel,
        message_type: str,
    ) -> bytes:
        """Convert a Pydantic model to a Protobuf message (bytes)."""
        return self._cache.json_to_protobuf(model_to_json(pydantic_model), message_type)

    def protobuf_to_pydantic(
        self,
        protobuf_bytes: bytes,
        model_class: Type[T],
        message_type: str,
    ) -> T:
        """Convert a Protobuf message (bytes) to a Pydantic model instance."""
        json_str = self._cache.protobuf_to_json(
            protobuf_bytes, message_type, False, True
        )
        return model_from_json(json_str, model_class)


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


def pydantic_to_protobuf(
    pydantic_model: BaseModel,
    descriptor_bytes: bytes,
    message_type: str,
) -> bytes:
    """
    Convert a Pydantic model to a Protobuf message.

    Args:
        pydantic_model: Pydantic model to convert
        descriptor_bytes: Compiled protobuf descriptor set (from compile_proto or load_descriptor)
        message_type: Full message type name (e.g., "user.User", "ecommerce.Order")

    Returns:
        Protobuf message as bytes

    Raises:
        ValueError: If Pydantic model is invalid or message type is not found in descriptor
    """
    json_str = model_to_json(pydantic_model)
    return _protoruf.json_to_protobuf(json_str, descriptor_bytes, message_type)


def protobuf_to_json(
    protobuf_bytes: bytes,
    descriptor_bytes: bytes,
    message_type: str,
    pretty: bool = False,
    include_default_values: bool = False,
) -> str:
    """
    Convert a Protobuf message to a JSON string.

    Args:
        protobuf_bytes: Protobuf message as bytes
        descriptor_bytes: Compiled protobuf descriptor set (from compile_proto or load_descriptor)
        message_type: Full message type name (e.g., "user.User", "ecommerce.Order")
        pretty: If True, format JSON with indentation (default: False)
        include_default_values: If True, emit fields without presence even at their
            default value (`0`, `""`, `false`, `[]`, `{}`). Fields with presence
            (`optional`, messages) are still emitted only when set (default: False)

    Returns:
        JSON string representation

    Raises:
        RuntimeError: If decoding or JSON serialization fails
    """
    return _protoruf.protobuf_to_json(
        protobuf_bytes,
        descriptor_bytes,
        message_type,
        pretty=pretty,
        include_default_values=include_default_values,
    )


def protobuf_to_pydantic(
    protobuf_bytes: bytes,
    descriptor_bytes: bytes,
    model_class: Type[T],
    message_type: str,
) -> T:
    """
    Convert a Protobuf message to a Pydantic model instance.

    Args:
        protobuf_bytes: Protobuf message as bytes
        descriptor_bytes: Compiled protobuf descriptor set
        model_class: The Pydantic model class expected in return (e.g: User)
        message_type: Full message type name (e.g., "user.User")

    Returns:
        An instance of the Pydantic model class provided (model_class)
    """
    json_str = protobuf_to_json(
        protobuf_bytes, descriptor_bytes, message_type, include_default_values=True
    )
    return model_from_json(json_str, model_class)
