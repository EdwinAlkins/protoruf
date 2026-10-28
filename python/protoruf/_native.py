"""Python-side helpers of the native codec: open enums and model introspection.

The Rust codec (`NativeCodec`) calls `model_links` and `construct_info` once, when it
is created with a model, to learn which nested model or enum class each field holds
and how to build instances without validation (`mode="construct"`).
"""

from __future__ import annotations

import enum
import types
from typing import Any, Union, get_args, get_origin

from pydantic import BaseModel
from pydantic_core import PydanticUndefined


class OpenIntEnum(enum.IntEnum):
    """An `IntEnum` that accepts values it does not declare.

    Proto3 enums are open: a peer built from a newer schema may send a number this
    side does not know. Instead of failing, the unknown number becomes a cached
    pseudo-member named `UNRECOGNIZED_<n>`, which compares equal to the int, is
    re-encoded unchanged, and is not listed when iterating the enum.

    Works with Pydantic validation (Python and JSON modes)::

        class Status(OpenIntEnum):
            UNSPECIFIED = 0
            ACTIVE = 1

        Status(7)  # <Status.UNRECOGNIZED_7: 7>
    """

    @classmethod
    def _missing_(cls, value: object) -> OpenIntEnum | None:
        if not isinstance(value, int) or isinstance(value, bool):
            return None
        member = int.__new__(cls, value)
        member._name_ = f"UNRECOGNIZED_{value}"
        member._value_ = value
        return cls._value2member_map_.setdefault(value, member)


def _target(annotation: Any) -> type | None:
    """The model or enum class a field holds, through `Optional`, lists and dicts."""
    origin = get_origin(annotation)
    args = get_args(annotation)
    if origin in (Union, types.UnionType):
        non_none = [a for a in args if a is not type(None)]
        return _target(non_none[0]) if len(non_none) == 1 else None
    if origin in (list, tuple, set, frozenset):
        return _target(args[0]) if args else None
    if origin is dict:
        return _target(args[1]) if len(args) == 2 else None
    if isinstance(annotation, type) and issubclass(annotation, (BaseModel, enum.Enum)):
        return annotation
    return None


def model_links(model_class: type[BaseModel]) -> dict[type, dict[str, type]]:
    """For each model reachable from `model_class`: field name -> model/enum class."""
    links: dict[type, dict[str, type]] = {}
    pending = [model_class]
    while pending:
        current = pending.pop()
        if current in links:
            continue
        fields: dict[str, type] = {}
        for name, field in current.model_fields.items():
            target = _target(field.annotation)
            if target is None:
                continue
            fields[name] = target
            if issubclass(target, BaseModel):
                pending.append(target)
        links[current] = fields
    return links


_SHARED, _COPIED, _FACTORY, _REQUIRED = range(4)
_IMMUTABLE = (type(None), bool, int, float, str, bytes, tuple, frozenset, enum.Enum)


def construct_info(model_class: type[BaseModel]) -> tuple[bool, bool, list[tuple[str, int, Any]]]:
    """How to build `model_class` instances without validation.

    Returns `(fast, extra_allow, defaults)`:

    - `fast`: instances can be assembled directly (`__dict__`, fields set, extra,
      private), as `BaseModel.model_construct` does. False for models with
      `model_post_init`, private attributes, root models, or default factories that
      read validated data: those go through `model_construct` itself.
    - `extra_allow`: the model is configured with `extra="allow"`.
    - `defaults`: `(field name, kind, value)` for every field, kind being shared value,
      deep-copied value, default factory, or required (left unset, like
      `model_construct`).
    """
    fast = not (
        model_class.__pydantic_post_init__
        or model_class.__private_attributes__
        or getattr(model_class, "__pydantic_root_model__", False)
    )
    defaults: list[tuple[str, int, Any]] = []
    for name, field in model_class.model_fields.items():
        if field.default_factory is not None:
            if getattr(field, "default_factory_takes_data", False):
                fast = False
            defaults.append((name, _FACTORY, field.default_factory))
        elif field.default is PydanticUndefined:
            defaults.append((name, _REQUIRED, None))
        elif isinstance(field.default, _IMMUTABLE):
            defaults.append((name, _SHARED, field.default))
        else:
            defaults.append((name, _COPIED, field.default))
    extra_allow = model_class.model_config.get("extra") == "allow"
    return fast, extra_allow, defaults
