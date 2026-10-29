"""Protobuf descriptor compiler using Rust protox."""

from collections.abc import Mapping, Sequence
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


def compile_protos(
    roots: Sequence[str | Path],
    include_paths: Sequence[str | Path] = (),
    sources: Mapping[str, str] | None = None,
    *,
    include_imports: bool = True,
    source_info: bool = False,
    standard_protos: bool = True,
    output_path: str | Path | None = None,
) -> bytes:
    """
    Compile several root .proto files in one go, from memory, disk, or both.

    Imports are resolved, in this order, from `sources`, from each directory of
    `include_paths`, from the standard protos embedded in protoruf, and from the
    `google/protobuf/*` well-known types.

    Args:
        roots: Files to compile: import names (e.g. "orders/v1/orders.proto", a key of
            `sources` or a path relative to an include path), or paths inside one of
            `include_paths`
        include_paths: Directories to resolve imports from
        sources: In-memory files, name -> source text; they take priority over disk
        include_imports: Embed every imported file so the descriptor set is
            self-contained
        source_info: Keep comments and source positions (needed by `Schema` to expose
            comments; makes the descriptor several times larger)
        standard_protos: Resolve the embedded `google/rpc/*`, `google/type/*`,
            `grpc/health/v1` and `grpc/reflection/{v1,v1alpha}` files
        output_path: Optional path to save the descriptor set

    Returns:
        Compiled descriptor set as bytes

    Raises:
        RuntimeError: If compilation fails; the message starts with the location of the
            error (`file.proto:line:column: ...`)
    """
    descriptor_bytes = _protoruf.compile_protos(
        [str(root) for root in roots],
        [str(Path(path)) for path in include_paths],
        dict(sources or {}),
        include_imports,
        source_info,
        standard_protos,
    )

    if output_path:
        output_path = Path(output_path)
        output_path.parent.mkdir(parents=True, exist_ok=True)
        output_path.write_bytes(descriptor_bytes)

    return descriptor_bytes
