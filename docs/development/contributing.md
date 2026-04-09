# Contributing Guide

Thank you for your interest in contributing to protoruf! This guide covers setup, development workflow, and best practices.

## Development Setup

### Prerequisites

- **Python 3.12+**
- **Rust** (1.70+) — [Install via rustup](https://rustup.rs/)
- **[uv](https://docs.astral.sh/uv/)** — Fast Python package manager

### 1. Clone the Repository

```bash
git clone https://github.com/EdwinAlkins/protoruf.git
cd protoruf
```

### 2. Install Dependencies

```bash
uv sync --dev
```

This installs all development dependencies including `maturin`, `pytest`, and `mypy`.

### 3. Build the Extension

```bash
uv run maturin develop
```

This compiles the Rust code and installs the Python package in development mode.

## Project Structure

```
protoruf/
├── python/protoruf/           # Python package
│   ├── __init__.py            # Public API wrappers
│   ├── _protoruf.pyi          # Type stubs for Rust extension
│   ├── compiler.py            # Proto compilation utilities
│   └── py.typed               # PEP 561 type marker
├── src/                       # Rust source code
│   ├── lib.rs                 # Python bindings (PyO3)
│   └── core.rs                # Core logic (pure Rust)
├── tests/                     # Python test suite
│   ├── proto/                 # Test proto files
│   ├── test_models.py         # Pydantic test models
│   └── test_rust_json_protobuf.py
├── examples/                  # Usage examples
└── docs/                      # Documentation
```

## Development Workflow

### Make Changes

1. Edit Rust code in `src/`
2. Edit Python code in `python/protoruf/`
3. Rebuild the extension:

```bash
uv run maturin develop
```

### Run Tests

#### Python Tests

```bash
uv run pytest tests/ -v
```

#### Rust Tests

```bash
cargo test --lib
```

#### All Tests

```bash
uv run pytest tests/ -v && cargo test --lib
```

### Type Checking

```bash
uv run mypy python/protoruf/ --ignore-missing-imports
```

### Pre-commit Checklist

Before committing, ensure all checks pass:

```bash
# 1. Python tests
uv run pytest tests/ -v

# 2. Rust tests
cargo test --lib

# 3. Type checking
uv run mypy python/protoruf/ --ignore-missing-imports

# 4. Build verification
uv run maturin develop
```

## Adding Features

### Rust Changes

1. Add logic to `src/core.rs` (pure Rust)
2. Expose via Python bindings in `src/lib.rs`
3. Update type stubs in `python/protoruf/_protoruf.pyi`
4. Write tests in both Rust and Python

### Python Changes

1. Update wrappers in `python/protoruf/__init__.py`
2. Update type stubs if signatures change
3. Add tests to `tests/`

### Documentation

Update relevant docs in `docs/`:
- API changes → `docs/api/reference.md`
- New features → `docs/guide/`
- Usage examples → `docs/examples/`

## Testing Guidelines

### Test Proto Files

Test proto files are in `tests/proto/`. When adding new proto features:

1. Update or create a `.proto` file
2. Add corresponding Python test
3. Test both conversion directions (JSON ↔ Protobuf)

### Example Test

```python
def test_nested_message():
    descriptor = compile_proto("tests/proto/message.proto")
    
    json_data = '''
    {
        "id": "123",
        "nested": {
            "field1": "value1",
            "field2": 42
        }
    }
    '''
    
    protobuf_bytes = json_to_protobuf(json_data, descriptor, "message.Message")
    result = protobuf_to_json(protobuf_bytes, descriptor, "message.Message")
    
    # Verify round-trip
    assert json.loads(json_data) == json.loads(result)
```

## Code Style

### Rust

- Follow standard Rust conventions (rustfmt)
- Use descriptive variable names
- Add comments for complex logic
- Write unit tests for core functions

### Python

- Follow PEP 8
- Use type hints everywhere
- Keep functions focused and small
- Docstrings for all public functions

## Building for Distribution

### Build Wheels

```bash
uv run maturin build
```

This creates distributable wheels in `target/wheels/`.

### Local Install

```bash
uv run maturin develop --release
```

## Debugging

### Enable Logging

Add print statements in Python:

```python
import protoruf
print(f"protoruf version: {protoruf.__version__}")
```

### Rust Debugging

Use `eprintln!` for debugging Rust code:

```rust
eprintln!("Descriptor size: {}", descriptor.len());
```

### Test Individual Functions

```bash
# Test specific Rust module
cargo test core

# Test specific Python module
uv run pytest tests/test_rust_json_protobuf.py -v
```

## Submitting Changes

1. **Fork** the repository
2. **Create a branch** for your feature: `git checkout -b feature/my-feature`
3. **Make changes** and ensure all tests pass
4. **Update documentation** if applicable
5. **Commit** with clear messages: `git commit -m "feat: add support for maps"`
6. **Push** to your fork: `git push origin feature/my-feature`
7. **Open a Pull Request** with a clear description of changes

### Commit Message Convention

Use conventional commits:

- `feat:` — New feature
- `fix:` — Bug fix
- `docs:` — Documentation changes
- `test:` — Test additions or changes
- `refactor:` — Code refactoring
- `chore:` — Maintenance tasks

## Reporting Issues

When reporting bugs:

1. Include **Python version**
2. Include **protoruf version**
3. Provide a **minimal reproducible example**
4. Include **error messages** and stack traces
5. Describe **expected vs actual behavior**

## Performance Considerations

When making changes:

- Avoid unnecessary allocations in Rust
- Use `&str` instead of `String` when possible
- Profile before optimizing
- Benchmark significant changes

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
