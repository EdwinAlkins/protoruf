"""Protobuf descriptor compiler using Rust protox."""

from pathlib import Path

from . import _protoruf


def compile_proto(
    proto_path: str | Path,
    include_paths: list[str | Path] | None = None,
    output_path: str | Path | None = None,
) -> bytes:
    """
    Compile a .proto file to a descriptor set using Rust protox.

    Args:
        proto_path: Path to the .proto file
        include_paths: List of include paths for imports
                       (defaults to parent directory of proto_path)
        output_path: Optional path to save the descriptor set

    Returns:
        Compiled descriptor set as bytes

    Raises:
        RuntimeError: If proto compilation fails
    """
    proto_path_str = str(Path(proto_path))

    include_paths_str: list[str] | None = None
    if include_paths is not None:
        include_paths_str = [str(Path(p)) for p in include_paths]

    descriptor_bytes = _protoruf.compile_proto(proto_path_str, include_paths_str)

    # Save to file if requested
    if output_path:
        output_path = Path(output_path)
        output_path.parent.mkdir(parents=True, exist_ok=True)
        output_path.write_bytes(descriptor_bytes)

    return descriptor_bytes


def load_descriptor(descriptor_path: str| Path) -> bytes:
    """
    Load a pre-compiled descriptor set from a file.

    Args:
        descriptor_path: Path to the .desc file

    Returns:
        Descriptor set as bytes

    Raises:
        FileNotFoundError: If the descriptor file does not exist
    """
    return Path(descriptor_path).read_bytes()


def compile_proto_from_sources(
    files: dict[str, str],
    root: str,
    include_imports: bool = True,
    output_path: str | Path | None = None,
) -> bytes:
    """
    Compile .proto sources held in memory (no filesystem access).

    Args:
        files: Mapping filename -> source content (e.g. {"user.proto": "..."})
        root: Entry file name (e.g. "user.proto")
        include_imports: Embed imported protos in the descriptor set
        output_path: Optional path to save the descriptor set

    Returns:
        Compiled descriptor set as bytes
    """
    descriptor_bytes = _protoruf.compile_proto_from_sources(
        files, root, include_imports
    )

    if output_path:
        output_path = Path(output_path)
        output_path.parent.mkdir(parents=True, exist_ok=True)
        output_path.write_bytes(descriptor_bytes)

    return descriptor_bytes
