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


def compile_protos(
    roots: list[str],
    include_paths: list[str] = ...,
    sources: dict[str, str] = ...,
    include_imports: bool = True,
    source_info: bool = False,
    standard_protos: bool = True,
) -> bytes: ...


def describe_descriptor_set(descriptor_bytes: bytes) -> str: ...


def split_descriptor_set(descriptor_bytes: bytes) -> list[tuple[str, bytes]]: ...


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


class Codec:
    """Protobuf bytes <-> Python objects and Pydantic models, without JSON.

    Usually created with `Schema(descriptor).codec("pkg.Message", Model)`.
    """

    def __init__(
        self,
        descriptor_bytes: bytes,
        message_type: str,
        model: type | None = None,
        *,
        mode: Literal["validate", "construct", "dict"] = "validate",
        field_names: dict[str, dict[int, str]] | None = None,
        unknown_enum: Literal["keep", "error"] = "keep",
        naive_datetime: Literal["error", "utc"] = "error",
    ) -> None: ...
    def decode(self, data: bytes) -> Any: ...
    def decode_dict(self, data: bytes) -> dict[str, Any]: ...
    def encode(self, obj: Any) -> bytes: ...

#: Name of `Codec` in protoruf 0.3 and 0.4 (experimental).
NativeCodec = Codec
