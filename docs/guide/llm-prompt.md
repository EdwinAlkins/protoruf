# protoruf — LLM Usage Prompt (Python)

<div class="llm-copy-mount"></div>

> Feed this file to an LLM (or paste it into a system prompt) so it can use the
> **Python** target of `protoruf` correctly with no other context. It is a
> complete, self-contained reference for version **0.2.0**.

## What protoruf is

`protoruf` converts between **JSON and Protobuf** *dynamically* — no `protoc`, no
generated classes. You compile a `.proto` to a **descriptor set** once, then
convert JSON ↔ Protobuf wire bytes in both directions. The engine is pure Rust
(exposed to Python via PyO3), so conversions run at Rust speed.

Core idea: `descriptor` (compiled schema, `bytes`) + `message_type` (fully
qualified name like `"user.User"`) are required for every conversion.

## Install

```bash
pip install protoruf
```

Requires Python ≥ 3.12. Pydantic (≥ 2) is a required runtime dependency — it is
imported at package load and powers the Pydantic helper functions.

## The mental model (3 steps)

1. **Compile** a `.proto` schema to a descriptor (`bytes`) — do this once.
2. **Convert** `json_to_protobuf` / `protobuf_to_json` using that descriptor and a
   `message_type`.
3. For throughput, wrap the descriptor in a **`DescriptorCache`** and reuse it.

## Complete public API

All names below are importable from the top-level `protoruf` package:

```python
from protoruf import (
    compile_proto,
    compile_proto_from_sources,
    load_descriptor,
    json_to_protobuf,
    protobuf_to_json,
    pydantic_to_protobuf,
    protobuf_to_pydantic,
    DescriptorCache,
)
```

### Compilation

```python
def compile_proto(
    proto_path: str | Path,
    include_paths: list[str | Path] | None = None,
    output_path: str | Path | None = None,
) -> bytes
```
Compile a `.proto` file **from disk**. `include_paths` defaults to the parent
directory of `proto_path`. If `output_path` is given, the descriptor is also
written there (as a `.desc` file). Raises `RuntimeError` on compile failure.

```python
def compile_proto_from_sources(
    files: dict[str, str],
    root: str,
    include_imports: bool = True,
    output_path: str | Path | None = None,
) -> bytes
```
Compile `.proto` sources held **in memory** (no filesystem). `files` maps a
logical filename → its source text; `import` statements resolve by name within
this map. Google well-known types are provided automatically. `root` is the entry
file (must be a key of `files`). `include_imports=True` embeds imported files so
the descriptor is self-contained; `False` produces a smaller, faster-decoding
descriptor when you don't need well-known types.

```python
def load_descriptor(descriptor_path: str | Path) -> bytes
```
Read a pre-compiled `.desc` file back into `bytes`. Raises `FileNotFoundError` if
missing.

### Conversion (one-shot)

```python
def json_to_protobuf(json_str: str, descriptor_bytes: bytes, message_type: str) -> bytes
def protobuf_to_json(protobuf_bytes: bytes, descriptor_bytes: bytes, message_type: str, pretty: bool = False) -> str
```
`message_type` is the **fully qualified** name: `"<package>.<Message>"`, e.g.
`"user.User"`. `pretty=True` indents the JSON output. These re-resolve the message
type on every call; for hot loops use `DescriptorCache`.

### Pydantic helpers (free functions)

```python
def pydantic_to_protobuf(pydantic_model: BaseModel, descriptor_bytes: bytes, message_type: str) -> bytes
def protobuf_to_pydantic(protobuf_bytes: bytes, descriptor_bytes: bytes, model_class: Type[T], message_type: str) -> T
```
Thin wrappers: `pydantic_to_protobuf` calls `model.model_dump_json()` then
`json_to_protobuf`; `protobuf_to_pydantic` calls `protobuf_to_json` then
`model_class.model_validate_json()`. Note the argument order:
`protobuf_to_pydantic(bytes, descriptor, ModelClass, message_type)`.

### `DescriptorCache` (high-throughput)

Decoding the descriptor set is the dominant cost. `DescriptorCache` decodes the
pool **once** and memoizes resolved message descriptors — roughly a **7–14×
speedup** in loops. One instance handles every message type in the descriptor and
is safe to share across threads.

```python
class DescriptorCache:
    def __init__(self, descriptor_bytes: bytes) -> None: ...
    def json_to_protobuf(self, json_str: str, message_type: str) -> bytes: ...
    def protobuf_to_json(self, protobuf_bytes: bytes, message_type: str, pretty: bool = False) -> str: ...
    def pydantic_to_protobuf(self, pydantic_model: BaseModel, message_type: str) -> bytes: ...
    def protobuf_to_pydantic(self, protobuf_bytes: bytes, model_class: Type[T], message_type: str) -> T: ...
```
The cache methods take **no** `descriptor_bytes` argument (the pool is already
decoded). Output format is identical to the free functions.

## Canonical examples

### Minimal round-trip (from disk)

```python
from protoruf import compile_proto, json_to_protobuf, protobuf_to_json

descriptor = compile_proto("message.proto")
pb = json_to_protobuf('{"id": "123"}', descriptor, message_type="message.Message")
print(protobuf_to_json(pb, descriptor, message_type="message.Message", pretty=True))
```

### Compile from in-memory sources

```python
from protoruf import compile_proto_from_sources, json_to_protobuf

files = {
    "user.proto": '''
        syntax = "proto3";
        package user;
        message User { string id = 1; string email = 2; }
    ''',
}
descriptor = compile_proto_from_sources(files, root="user.proto")
pb = json_to_protobuf('{"id":"1","email":"a@b.com"}', descriptor, "user.User")
```

### High-throughput loop with `DescriptorCache`

```python
from protoruf import compile_proto, DescriptorCache

cache = DescriptorCache(compile_proto("schema.proto"))  # decode pool once
for json_data in json_stream:
    pb = cache.json_to_protobuf(json_data, "message.Message")
    process(pb)
restored = cache.protobuf_to_json(pb, "message.Message", pretty=True)
```

### Pydantic integration

```python
from pydantic import BaseModel
from protoruf import compile_proto, DescriptorCache

class Message(BaseModel):
    id: str = ""
    content: str = ""

cache = DescriptorCache(compile_proto("schema.proto"))
pb = cache.pydantic_to_protobuf(Message(id="123", content="hi"), "message.Message")
msg = cache.protobuf_to_pydantic(pb, Message, "message.Message")
```

## JSON shape — READ THIS (non-standard)

protoruf uses a **specific, non-default** JSON mapping. When generating or
validating JSON for it, follow these rules exactly:

- **Field names are proto names (`snake_case`)**, NOT the camelCase of canonical
  protobuf JSON. Use `created_at`, not `createdAt`.
- **Enums are emitted as numbers** (their integer value), not names. On input,
  numbers are expected; enum names may also be accepted on input but output is
  always numeric.
- **64-bit integers (`int64`/`uint64`/`sint64`/`fixed64`) are JSON numbers, not
  strings.** ⚠️ Precision caveat: values above 2^53 lose precision in a standard
  JSON reader. Consumers needing exact large 64-bit values must use a
  big-integer-aware JSON parser.
- **`bytes` fields** map to standard base64-encoded strings.
- **Fields at their proto3 default (0, `""`, `false`, empty repeated/map) are
  omitted** from the output JSON. Don't rely on them being present after a
  round-trip; re-parsing yields the default anyway.
- Standard proto3 JSON applies otherwise (nested messages → objects, repeated →
  arrays, map → object, `oneof` → the single set field, well-known types like
  `Timestamp`/`Duration`/`Struct` use their canonical JSON forms).

## Errors

| Exception | When |
|---|---|
| `RuntimeError` | proto compilation fails; **any** failure in `protobuf_to_json` including an unknown `message_type` in that direction; protobuf decode/serialize failure |
| `ValueError` | invalid JSON input, or `message_type` not found — in the `json_to_protobuf` direction; also raised by `DescriptorCache(...)` if the descriptor bytes can't be decoded |
| `FileNotFoundError` | `load_descriptor` path does not exist |
| `pydantic.ValidationError` | decoded JSON doesn't match the target Pydantic model |

> Note the asymmetry: a wrong `message_type` raises `ValueError` in
> `json_to_protobuf` but `RuntimeError` in `protobuf_to_json`.

## Rules for the LLM

1. Always compile (or load) a descriptor **before** converting; every conversion
   needs both `descriptor_bytes` and a fully-qualified `message_type`.
2. `message_type` must be `"<package>.<Message>"` matching the `.proto`'s
   `package` and message name — a wrong name raises `ValueError` in
   `json_to_protobuf` and `RuntimeError` in `protobuf_to_json`.
3. In the browser there is no filesystem — but this is the **Python** target, so
   `compile_proto(path)` is available and fine here.
4. For more than a few conversions, build one `DescriptorCache` and reuse it;
   don't call the free functions in a tight loop.
5. Emit JSON in protoruf's shape (snake_case fields, numeric enums, numeric
   64-bit ints) — do not assume canonical camelCase protobuf JSON.
