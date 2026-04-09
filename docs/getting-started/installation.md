# Installation

This guide covers installing protoruf in your Python project.

## Prerequisites

- **Python 3.12 or higher**
- **pip** or **[uv](https://docs.astral.sh/uv/)** (recommended)
- Rust toolchain is **NOT required** — pre-built wheels are provided

## Install via pip

```bash
pip install protoruf
```

## Install via uv (Recommended)

[uv](https://docs.astral.sh/uv/) is a fast Python package manager. If you don't have it:

```bash
curl -LsSf https://astral.sh/uv/install.sh | sh
```

Then install protoruf:

```bash
uv pip install protoruf
```

## Development Installation

If you want to contribute or build from source:

### 1. Clone the repository

```bash
git clone https://github.com/EdwinAlkins/protoruf.git
cd protoruf
```

### 2. Install dependencies

```bash
uv sync --dev
```

### 3. Build the Rust extension

```bash
uv run maturin develop
```

!!! note "Rust Requirement"
    Building from source requires [Rust](https://rustup.rs/) installed on your system. Pre-built wheels do not require Rust.

## Verify Installation

```python
python -c "import protoruf; print(protoruf.__version__)"
```

You should see the version number printed without errors.

## Platform Support

Pre-built wheels are available for:

| Platform | Architecture |
|----------|--------------|
| Linux | x86_64, aarch64 |
| macOS | x86_64, arm64 |
| Windows | x86_64 |

!!! tip "Missing Wheel?"
    If a pre-built wheel is not available for your platform, pip will attempt to build from source. Ensure Rust is installed in that case.

## Next Steps

- Follow the [Quick Start Guide](quick-start.md) to create your first proto message
- Read the [Basic Usage Guide](../guide/basic-usage.md) for detailed examples
