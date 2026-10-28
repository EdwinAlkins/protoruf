"""Read a compiled schema: files, messages, fields, enums, services and their comments.

`Schema(descriptor)` turns a descriptor set (from `compile_protos`, `compile_proto`, ...)
into immutable, typed objects: to generate code from `.proto` files, check models
against them, or serve gRPC server reflection::

    from protoruf import Schema, compile_protos

    schema = Schema(compile_protos(["orders/v1/orders.proto"], ["proto"], source_info=True))
    order = schema.message("orders.v1.Order")
    for field in order.fields:
        print(field.number, field.name, field.kind, field.type_name, field.comments)
"""

from __future__ import annotations

import json
from collections.abc import Mapping
from dataclasses import dataclass
from types import MappingProxyType
from typing import Any, Literal

from . import _protoruf

Cardinality = Literal["optional", "required", "repeated"]


@dataclass(frozen=True, slots=True)
class Comments:
    """Comments attached to an element, as written in the `.proto` (needs
    `source_info=True` at compile time)."""

    leading: str | None
    trailing: str | None
    detached: tuple[str, ...]

    @property
    def text(self) -> str | None:
        """The leading comment (else the trailing one) without comment indentation."""
        raw = self.leading if self.leading is not None else self.trailing
        if raw is None:
            return None
        lines = [line[1:] if line.startswith(" ") else line for line in raw.split("\n")]
        return "\n".join(lines).strip() or None


@dataclass(frozen=True, slots=True)
class FieldInfo:
    name: str
    number: int
    json_name: str
    #: Proto type: "int32", "string", ..., "enum" or "message".
    kind: str
    #: Full name of the enum or message type.
    type_name: str | None
    cardinality: Cardinality
    is_map: bool
    map_key_kind: str | None
    map_value_kind: str | None
    map_value_type_name: str | None
    is_packed: bool
    #: Explicit presence: messages, `optional`, oneof members, proto2 singular fields.
    has_presence: bool
    proto3_optional: bool
    #: Name of the real `oneof` holding this field (not the synthetic one of `optional`).
    oneof: str | None
    #: proto2 `[default = ...]`, as written in the `.proto`.
    default_value: str | None
    deprecated: bool
    comments: Comments | None

    @property
    def is_list(self) -> bool:
        """A `repeated` field that is not a map."""
        return self.cardinality == "repeated" and not self.is_map


@dataclass(frozen=True, slots=True)
class OneofInfo:
    name: str
    fields: tuple[str, ...]
    comments: Comments | None


@dataclass(frozen=True, slots=True)
class MessageInfo:
    name: str
    full_name: str
    file: str
    #: Full name of the enclosing message, for nested messages.
    parent: str | None
    #: In declaration order.
    fields: tuple[FieldInfo, ...]
    #: Real `oneof`s only.
    oneofs: tuple[OneofInfo, ...]
    #: Inclusive `(start, end)` pairs, as in `reserved 15 to 20;`.
    reserved_ranges: tuple[tuple[int, int], ...]
    reserved_names: tuple[str, ...]
    is_map_entry: bool
    deprecated: bool
    comments: Comments | None

    def field(self, name: str) -> FieldInfo:
        for field in self.fields:
            if field.name == name:
                return field
        raise KeyError(f"{self.full_name} has no field {name!r}")


@dataclass(frozen=True, slots=True)
class EnumValueInfo:
    name: str
    number: int
    deprecated: bool
    comments: Comments | None


@dataclass(frozen=True, slots=True)
class EnumInfo:
    name: str
    full_name: str
    file: str
    parent: str | None
    values: tuple[EnumValueInfo, ...]
    reserved_ranges: tuple[tuple[int, int], ...]
    reserved_names: tuple[str, ...]
    deprecated: bool
    comments: Comments | None


@dataclass(frozen=True, slots=True)
class MethodInfo:
    name: str
    full_name: str
    input_type: str
    output_type: str
    client_streaming: bool
    server_streaming: bool
    deprecated: bool
    comments: Comments | None


@dataclass(frozen=True, slots=True)
class ServiceInfo:
    name: str
    full_name: str
    file: str
    methods: tuple[MethodInfo, ...]
    deprecated: bool
    comments: Comments | None

    def method(self, name: str) -> MethodInfo:
        for method in self.methods:
            if method.name == name:
                return method
        raise KeyError(f"{self.full_name} has no method {name!r}")


@dataclass(frozen=True, slots=True)
class FileInfo:
    name: str
    package: str
    syntax: Literal["proto2", "proto3"]
    dependencies: tuple[str, ...]
    #: Full names of the top-level messages, enums and services.
    messages: tuple[str, ...]
    enums: tuple[str, ...]
    services: tuple[str, ...]


def _comments(raw: dict[str, Any] | None) -> Comments | None:
    if raw is None:
        return None
    return Comments(raw["leading"], raw["trailing"], tuple(raw["detached"]))


def _ranges(raw: list[list[int]]) -> tuple[tuple[int, int], ...]:
    return tuple((start, end) for start, end in raw)


def _field(raw: dict[str, Any]) -> FieldInfo:
    return FieldInfo(**{**raw, "comments": _comments(raw["comments"])})


def _message(raw: dict[str, Any]) -> MessageInfo:
    return MessageInfo(
        name=raw["name"],
        full_name=raw["full_name"],
        file=raw["file"],
        parent=raw["parent"],
        fields=tuple(_field(f) for f in raw["fields"]),
        oneofs=tuple(
            OneofInfo(o["name"], tuple(o["fields"]), _comments(o["comments"]))
            for o in raw["oneofs"]
        ),
        reserved_ranges=_ranges(raw["reserved_ranges"]),
        reserved_names=tuple(raw["reserved_names"]),
        is_map_entry=raw["is_map_entry"],
        deprecated=raw["deprecated"],
        comments=_comments(raw["comments"]),
    )


def _enum(raw: dict[str, Any]) -> EnumInfo:
    return EnumInfo(
        name=raw["name"],
        full_name=raw["full_name"],
        file=raw["file"],
        parent=raw["parent"],
        values=tuple(
            EnumValueInfo(v["name"], v["number"], v["deprecated"], _comments(v["comments"]))
            for v in raw["values"]
        ),
        reserved_ranges=_ranges(raw["reserved_ranges"]),
        reserved_names=tuple(raw["reserved_names"]),
        deprecated=raw["deprecated"],
        comments=_comments(raw["comments"]),
    )


def _service(raw: dict[str, Any]) -> ServiceInfo:
    return ServiceInfo(
        name=raw["name"],
        full_name=raw["full_name"],
        file=raw["file"],
        methods=tuple(
            MethodInfo(**{**m, "comments": _comments(m["comments"])}) for m in raw["methods"]
        ),
        deprecated=raw["deprecated"],
        comments=_comments(raw["comments"]),
    )


def _file(raw: dict[str, Any]) -> FileInfo:
    return FileInfo(
        name=raw["name"],
        package=raw["package"],
        syntax=raw["syntax"],
        dependencies=tuple(raw["dependencies"]),
        messages=tuple(raw["messages"]),
        enums=tuple(raw["enums"]),
        services=tuple(raw["services"]),
    )


class Schema:
    """Immutable view of a compiled descriptor set.

    Every file, message (nested and map entries included), enum and service is
    available by name. Comments are filled only when the descriptor was compiled with
    `source_info=True`.
    """

    def __init__(self, descriptor: bytes) -> None:
        raw = json.loads(_protoruf.describe_descriptor_set(descriptor))
        self.descriptor = descriptor
        self.files: Mapping[str, FileInfo] = MappingProxyType(
            {f["name"]: _file(f) for f in raw["files"]}
        )
        self.messages: Mapping[str, MessageInfo] = MappingProxyType(
            {m["full_name"]: _message(m) for m in raw["messages"]}
        )
        self.enums: Mapping[str, EnumInfo] = MappingProxyType(
            {e["full_name"]: _enum(e) for e in raw["enums"]}
        )
        self.services: Mapping[str, ServiceInfo] = MappingProxyType(
            {s["full_name"]: _service(s) for s in raw["services"]}
        )
        self._file_protos: dict[str, bytes] | None = None

    def __repr__(self) -> str:
        return (
            f"Schema({len(self.files)} files, {len(self.messages)} messages, "
            f"{len(self.enums)} enums, {len(self.services)} services)"
        )

    @staticmethod
    def _get(items: Mapping[str, Any], kind: str, name: str) -> Any:
        try:
            return items[name]
        except KeyError:
            raise KeyError(f"no {kind} named {name!r} in the schema") from None

    def file(self, name: str) -> FileInfo:
        info: FileInfo = self._get(self.files, "file", name)
        return info

    def message(self, full_name: str) -> MessageInfo:
        info: MessageInfo = self._get(self.messages, "message", full_name)
        return info

    def enum(self, full_name: str) -> EnumInfo:
        info: EnumInfo = self._get(self.enums, "enum", full_name)
        return info

    def service(self, full_name: str) -> ServiceInfo:
        info: ServiceInfo = self._get(self.services, "service", full_name)
        return info

    # -------------------------------------------------- gRPC server reflection

    def file_descriptor_proto(self, name: str) -> bytes:
        """The serialized `FileDescriptorProto` of one file, as reflection sends it."""
        if self._file_protos is None:
            self._file_protos = dict(_protoruf.split_descriptor_set(self.descriptor))
        protos: dict[str, bytes] = self._file_protos
        result: bytes = self._get(protos, "file", name)
        return result

    def file_containing_symbol(self, symbol: str) -> str | None:
        """Name of the file defining a message, enum, service or method
        (`"pkg.Service.Method"`), or None."""
        for items in (self.messages, self.enums, self.services):
            info = items.get(symbol)
            if info is not None:
                name: str = info.file
                return name
        service_name, _, method = symbol.rpartition(".")
        service = self.services.get(service_name)
        if service is not None and any(m.name == method for m in service.methods):
            return service.file
        return None

    def file_dependencies(self, name: str) -> tuple[str, ...]:
        """Every file `name` depends on, transitively, dependencies first."""
        ordered: list[str] = []

        def visit(current: str) -> None:
            for dependency in self.file(current).dependencies:
                if dependency not in ordered:
                    visit(dependency)
                    ordered.append(dependency)

        visit(name)
        return tuple(ordered)
