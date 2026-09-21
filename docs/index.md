# protoruf

<p align="center">
  <strong>High-performance JSON ↔ Protobuf conversion, powered by Rust —<br>for Python, Node.js, the browser (WASM) and Rust.</strong>
</p>

<p align="center">
  <a href="https://pypi.org/project/protoruf/">
    <img src="https://img.shields.io/pypi/v/protoruf?color=blue&logo=pypi" alt="PyPI Version">
  </a>
  <a href="https://github.com/EdwinAlkins/protoruf/releases">
    <img src="https://img.shields.io/github/v/release/EdwinAlkins/protoruf?color=red&logo=github&label=node%20%26%20wasm" alt="GitHub release">
  </a>
  <a href="https://github.com/EdwinAlkins/protoruf/blob/main/LICENSE">
    <img src="https://img.shields.io/github/license/EdwinAlkins/protoruf?color=blue" alt="License">
  </a>
</p>

---

## Overview

**protoruf** converts between JSON and Protobuf **dynamically** — no `protoc`, no generated
classes. You compile a `.proto` to a descriptor once, then convert in both directions at Rust
speed. It is built on [prost-reflect](https://github.com/andrewhickman/prost-reflect) and
[protox](https://github.com/andrewhickman/protox).

A single **pure-Rust core** powers every target through a thin binding layer, so the
conversion logic is shared, never duplicated:

| Target | Package | Install |
|---|---|---|
| **Python** | [`protoruf`](https://pypi.org/project/protoruf/) | `pip install protoruf` |
| **Node.js** | `@protoruf/node` | from [GitHub Releases](https://github.com/EdwinAlkins/protoruf/releases) by URL — npm coming soon ([guide](node/installation.md)) |
| **Browser (WASM)** | `@protoruf/wasm` | from [GitHub Releases](https://github.com/EdwinAlkins/protoruf/releases) by URL — npm coming soon ([guide](browser/installation.md)) |
| **Rust** | core crate | — |

### Why protoruf?

- ⚡ **Blazing fast** — core logic implemented in Rust
- 🔁 **One core, four targets** — identical semantics across Python / Node / Browser / Rust
- 🔒 **Type-safe** — explicit message types prevent serialization errors
- 📦 **No `protoc`** — built-in `.proto` compilation with protox, no external tools or codegen
- 🎯 **Complete Protobuf support** — nested messages, enums, repeated fields, maps, oneof

## Choose your target

<div class="grid cards" markdown>

-   :material-language-python:{ .lg .middle } **Python**

    ---

    `pip install protoruf` — with direct Pydantic integration.

    [Get started →](getting-started/installation.md)

-   :material-nodejs:{ .lg .middle } **Node.js**

    ---

    `@protoruf/node` — native addon, full filesystem access. Install from
    [GitHub Releases](https://github.com/EdwinAlkins/protoruf/releases) (npm coming soon).

    [Get started →](node/installation.md)

-   :material-web:{ .lg .middle } **Browser (WASM)**

    ---

    `@protoruf/wasm` — runs entirely in the browser, sandboxed. Install from
    [GitHub Releases](https://github.com/EdwinAlkins/protoruf/releases) (npm coming soon).

    [Get started →](browser/installation.md)

-   :material-language-rust:{ .lg .middle } **Rust**

    ---

    The shared engine behind every binding.

    [Learn more →](rust/overview.md)

</div>

## Architecture

```
┌───────────────────────────────────────────────────────────────┐
│   Python        Node.js        Browser (WASM)        Rust      │
│   (PyO3)        (napi-rs)      (wasm-bindgen)       (core API)  │
├───────────────────────────────────────────────────────────────┤
│                   core.rs  —  pure-Rust engine                 │
│       .proto compilation · JSON ↔ Protobuf · descriptor pool   │
├───────────────────────────────────────────────────────────────┤
│            protox  │  prost-reflect  │  serde_json             │
└───────────────────────────────────────────────────────────────┘
```

Each binding only translates types & errors around the core; building one target never pulls
in another's dependencies.

## Features

| Feature | Description |
|---------|-------------|
| **JSON ↔ Protobuf** | Bidirectional conversion with full type safety |
| **Proto compilation** | Compile `.proto` files without external tools (`compileProtoFromSources` works in-memory, even in the browser) |
| **Nested messages** | Full support for nested message types |
| **Enums** | Automatic name ↔ number handling |
| **Repeated fields & maps** | Lists, arrays and dictionary structures |
| **Oneof fields** | Support for union types |
| **DescriptorCache** | Pre-decoded pool for high-throughput conversion |
| **Pydantic integration** | Python: direct `pydantic_to_protobuf()` / `protobuf_to_pydantic()` |
