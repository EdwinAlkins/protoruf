"""Type stubs for the Rust extension module."""

from typing import Any, Literal


def compile_proto(
    proto_path: str,
    include_paths: list[str] | None = None,
) -> bytes: ...


def json_to_protobuf(
    json_str: str,
    descriptor_bytes: bytes,
    message_type: str,
) -> bytes: ...


def protobuf_to_json(
    protobuf_bytes: bytes,
    descriptor_bytes: bytes,
    message_type: str,
    pretty: bool = False,
    include_default_values: bool = False,
) -> str: ...


def compile_proto_from_sources(
    files: dict[str, str],
    root: str,
    include_imports: bool = True,
) -> bytes: ...


class DescriptorCache:
    """A reusable, pre-decoded descriptor pool."""

    def __init__(self, descriptor_bytes: bytes) -> None: ...
    def json_to_protobuf(self, json_str: str, message_type: str) -> bytes: ...
    def protobuf_to_json(
        self,
        protobuf_bytes: bytes,
        message_type: str,
        pretty: bool = False,
        include_default_values: bool = False,
    ) -> str: ...


class NativeCodec:
    """Spike: protobuf bytes <-> Python objects without JSON (API not final)."""

    def __init__(
        self,
        descriptor_bytes: bytes,
        message_type: str,
        model: type | None = None,
        *,
        mode: Literal["validate", "construct"] = "validate",
        unknown_enum: Literal["keep", "error"] = "keep",
        naive_datetime: Literal["error", "utc"] = "error",
    ) -> None: ...
    def decode(self, data: bytes) -> Any: ...
    def decode_dict(self, data: bytes) -> dict[str, Any]: ...
    def encode(self, obj: Any) -> bytes: ...
