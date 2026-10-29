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


def _spec(annotation: Any, with_bytes: frozenset[type[BaseModel]]) -> Any | None:
    """Where `bytes` values live in a value of type `annotation`, `with_bytes` being the
    models that hold some; None if nowhere."""
    if annotation is bytes:
        return _BYTES
    origin = get_origin(annotation)
    args = get_args(annotation)
    if origin in (Union, types.UnionType):
        non_none = [a for a in args if a is not type(None)]
        return _spec(non_none[0], with_bytes) if len(non_none) == 1 else None
    if origin in (list, tuple, set, frozenset):
        inner = _spec(args[0], with_bytes) if args else None
        return None if inner is None else ("list", inner)
    if origin is dict:
        inner = _spec(args[1], with_bytes) if len(args) == 2 else None
        return None if inner is None else ("dict", inner)
    if annotation in with_bytes:
        return ("model", annotation)
    return None


def _models_in(annotation: Any) -> list[type[BaseModel]]:
    """The models a value of type `annotation` can hold, through the same shapes as
    `_spec`."""
    origin = get_origin(annotation)
    args = get_args(annotation)
    if origin in (Union, types.UnionType):
        non_none = [a for a in args if a is not type(None)]
        return _models_in(non_none[0]) if len(non_none) == 1 else []
    if origin in (list, tuple, set, frozenset):
        return _models_in(args[0]) if args else []
    if origin is dict:
        return _models_in(args[1]) if len(args) == 2 else []
    if isinstance(annotation, type) and issubclass(annotation, BaseModel):
        return [annotation]
    return []


def _input_key(name: str, field: Any) -> str:
    for alias in (field.validation_alias, field.alias):
        if isinstance(alias, str):
            return alias
    return name


def _plan(model_class: type[BaseModel]) -> dict[str, Any]:
    """The plan of `model_class`, computing those of every model it reaches.

    Two passes, so that recursion cannot fool the second: find the models that hold
    `bytes` (directly or through other models, up to a fixed point), then plan each model
    with only those.
    """
    cached = _plans.get(model_class)
    if cached is not None:
        return cached
    reachable: list[type[BaseModel]] = []
    pending = [model_class]
    while pending:
        current = pending.pop()
        if current in reachable:
            continue
        reachable.append(current)
        for field in current.model_fields.values():
            pending += _models_in(field.annotation)
    with_bytes: frozenset[type[BaseModel]] = frozenset()
    changed = True
    while changed:
        found = {
            model
            for model in reachable
            if any(_spec(f.annotation, with_bytes) is not None for f in model.model_fields.values())
        }
        changed = found != with_bytes
        with_bytes = frozenset(found)
    for model in reachable:
        if model not in _plans:
            plan = {}
            for name, field in model.model_fields.items():
                spec = _spec(field.annotation, with_bytes)
                if spec is not None:
                    plan[_input_key(name, field)] = spec
            _plans[model] = plan
    return _plans[model_class]


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
