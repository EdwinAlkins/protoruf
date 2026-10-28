"""JSON bridge between Pydantic models and protoruf's protobuf JSON shape.

protoruf's JSON carries `bytes` as base64 (proto3 JSON mapping), whereas Pydantic
serializes and validates `bytes` as UTF-8 text by default: arbitrary binary data
cannot round-trip through `model_dump_json` / `model_validate_json`.

Models without any `bytes` field (at any depth) keep the direct JSON path. Models
with `bytes` fields go through Python objects instead, with base64 applied
explicitly. Note that this path validates in Python mode, so a model configured
with `strict=True` must then receive Python types (e.g. `datetime`, not strings)
for its other fields.
"""

from __future__ import annotations

import base64
import types
from typing import Any, TypeVar, Union, get_args, get_origin

import pydantic_core
from pydantic import BaseModel

T = TypeVar("T", bound=BaseModel)

# Where `bytes` values live in a model, keyed by input key:
#   "bytes" | ("list", spec) | ("dict", spec) | ("model", model_class)
_BYTES = "bytes"
# A plain dict (not a WeakKeyDictionary): the lookup runs on every conversion and a
# weak lookup costs several times more. Model classes are long-lived in practice.
_plans: dict[type[BaseModel], dict[str, Any]] = {}


def _spec(annotation: Any, stack: tuple[type[BaseModel], ...]) -> Any | None:
    if annotation is bytes:
        return _BYTES
    origin = get_origin(annotation)
    args = get_args(annotation)
    if origin in (Union, types.UnionType):
        non_none = [a for a in args if a is not type(None)]
        return _spec(non_none[0], stack) if len(non_none) == 1 else None
    if origin in (list, tuple, set, frozenset):
        inner = _spec(args[0], stack) if args else None
        return None if inner is None else ("list", inner)
    if origin is dict:
        inner = _spec(args[1], stack) if len(args) == 2 else None
        return None if inner is None else ("dict", inner)
    if isinstance(annotation, type) and issubclass(annotation, BaseModel):
        # A model already being planned (recursive type) is kept: its plan is
        # looked up when the value is decoded.
        if annotation in stack or _plan(annotation, stack):
            return ("model", annotation)
    return None


def _input_key(name: str, field: Any) -> str:
    for alias in (field.validation_alias, field.alias):
        if isinstance(alias, str):
            return alias
    return name


def _plan(model_class: type[BaseModel], stack: tuple[type[BaseModel], ...] = ()) -> dict[str, Any]:
    cached = _plans.get(model_class)
    if cached is not None:
        return cached
    stack = (*stack, model_class)
    plan: dict[str, Any] = {}
    for name, field in model_class.model_fields.items():
        spec = _spec(field.annotation, stack)
        if spec is not None:
            plan[_input_key(name, field)] = spec
    _plans[model_class] = plan
    return plan


def _decode(value: Any, spec: Any) -> Any:
    if value is None:
        return None
    if spec == _BYTES:
        return base64.b64decode(value) if isinstance(value, str) else value
    kind, inner = spec
    if kind == "list" and isinstance(value, list):
        return [_decode(item, inner) for item in value]
    if kind == "dict" and isinstance(value, dict):
        return {key: _decode(item, inner) for key, item in value.items()}
    if kind == "model":
        return _apply(value, _plan(inner))
    return value


def _apply(data: Any, plan: dict[str, Any]) -> Any:
    if isinstance(data, dict):
        for key, spec in plan.items():
            if key in data:
                data[key] = _decode(data[key], spec)
    return data


def model_to_json(model: BaseModel) -> str:
    """Serialize a model to protoruf's JSON shape (`bytes` as base64)."""
    model_class = type(model)
    plan = _plans.get(model_class)
    if plan is None:
        plan = _plan(model_class)
    if not plan:
        return model.model_dump_json()
    return pydantic_core.to_json(model.model_dump(), bytes_mode="base64").decode()


def model_from_json(json_str: str, model_class: type[T]) -> T:
    """Validate protoruf JSON into `model_class` (`bytes` read from base64)."""
    plan = _plans.get(model_class)
    if plan is None:
        plan = _plan(model_class)
    if not plan:
        return model_class.model_validate_json(json_str)
    return model_class.model_validate(_apply(pydantic_core.from_json(json_str), plan))
