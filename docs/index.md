# protoruf

<p align="center">
  <strong>High-performance JSON ↔ Protobuf conversion for Python, powered by Rust</strong>
</p>

<p align="center">
  <a href="https://github.com/EdwinAlkins/protoruf/actions">
    <img src="https://img.shields.io/github/actions/workflow/status/EdwinAlkins/protoruf/ci.yml?branch=main&label=CI&logo=github" alt="CI Status">
  </a>
  <a href="https://pypi.org/project/protoruf/">
    <img src="https://img.shields.io/pypi/v/protoruf?color=blue&logo=pypi" alt="PyPI Version">
  </a>
  <a href="https://github.com/EdwinAlkins/protoruf/blob/main/LICENSE">
    <img src="https://img.shields.io/github/license/EdwinAlkins/protoruf?color=blue" alt="License">
  </a>
  <a href="https://github.com/EdwinAlkins/protoruf">
    <img src="https://img.shields.io/badge/python-3.12+-blue?logo=python" alt="Python Version">
  </a>
</p>

---

## Overview

**protoruf** is a Python library written in Rust that provides blazing-fast conversion between JSON and Protobuf messages. Built on top of [prost-reflect](https://github.com/devicehive/prost-reflect) and [protox](https://github.com/andrewhick/protox), it offers type-safe serialization with explicit message type specification.

### Why protoruf?

- ⚡ **Blazing Fast** — Core logic implemented in Rust for maximum performance
- 🔒 **Type-Safe** — Explicit message types prevent serialization errors
- 📦 **Zero External Dependencies** — Built-in proto compilation with Rust protox
- 🎯 **Complete Protobuf Support** — Nested messages, enums, repeated fields, maps, oneof
- 🐍 **Pythonic API** — Clean, intuitive interface with full type hints

### Quick Example

```python
from protoruf import compile_proto, json_to_protobuf, protobuf_to_json

# Compile your .proto file
descriptor = compile_proto("message.proto")

# Convert JSON to Protobuf
json_data = '{"id": "123", "content": "Hello", "priority": 1}'
protobuf_bytes = json_to_protobuf(json_data, descriptor, message_type="message.Message")

# Convert Protobuf back to JSON
result = protobuf_to_json(protobuf_bytes, descriptor, message_type="message.Message", pretty=True)
print(result)
```

## Getting Started

<div class="grid cards" markdown>

-   :material-download:{ .lg .middle } **Installation**
    
    ---
    
    Install protoruf via pip and verify your setup
    
    [Get Started →](getting-started/installation.md)

-   :material-rocket-launch:{ .lg .middle } **Quick Start**
    
    ---
    
    Build your first proto message in 5 minutes
    
    [Quick Start →](getting-started/quick-start.md)

-   :material-book-open:{ .lg .middle } **User Guide**
    
    ---
    
    Learn basic usage, proto files, and advanced features
    
    [Read Guide →](guide/basic-usage.md)

-   :material-code-braces:{ .lg .middle } **API Reference**
    
    ---
    
    Complete API documentation with examples
    
    [View API →](api/reference.md)

</div>

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     Python Application                      │
│              compile_proto()  │  json_to_protobuf()         │
│              protobuf_to_json()  │  load_descriptor()       │
├─────────────────────────────────────────────────────────────┤
│                   PyO3 Rust Bindings                        │
├─────────────────────────────────────────────────────────────┤
│         protox  │  prost-reflect  │  serde_json             │
└─────────────────────────────────────────────────────────────┘
```

## Features

| Feature | Description |
|---------|-------------|
| **JSON ↔ Protobuf** | Bidirectional conversion with full type safety |
| **Proto Compilation** | Compile `.proto` files without external tools |
| **Nested Messages** | Full support for nested message types |
| **Enums** | Automatic string ↔ number conversion |
| **Repeated Fields** | Handle lists and arrays seamlessly |
| **Maps** | Convert dictionary structures |
| **Oneof Fields** | Support for union types |
| **Pydantic Integration** | Works seamlessly with Pydantic models |

## Next Steps

- [**Install protoruf**](getting-started/installation.md) and get up and running
- Follow the [**Quick Start**](getting-started/quick-start.md) to build your first proto message
- Explore the [**User Guide**](guide/basic-usage.md) for detailed usage examples
- Check the [**API Reference**](api/reference.md) for complete documentation
