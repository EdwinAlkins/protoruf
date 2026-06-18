"""Type stubs for the Rust extension module."""


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
    pretty: bool = False,
    message_type: str = ...,
) -> str: ...


class DescriptorCache:
    """A reusable, pre-decoded descriptor pool."""

    def __init__(self, descriptor_bytes: bytes) -> None: ...
    def json_to_protobuf(self, json_str: str, message_type: str) -> bytes: ...
    def protobuf_to_json(
        self,
        protobuf_bytes: bytes,
        message_type: str,
        pretty: bool = False,
    ) -> str: ...
