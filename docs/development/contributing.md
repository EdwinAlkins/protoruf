# Contributing Guide

Thank you for your interest in contributing to protoruf! This guide covers setup, development workflow, and best practices.

## Development Setup

### Prerequisites

- **Python 3.12+**
- **Rust** (1.70+) — [Install via rustup](https://rustup.rs/)
- **[uv](https://docs.astral.sh/uv/)** — Fast Python package manager

For the Node.js and browser (WASM) bindings, you also need:

- **Node.js 20+** — [Install via nvm](https://github.com/nvm-sh/nvm) or [nodejs.org](https://nodejs.org/)
- **[wasm-pack](https://rustwasm.github.io/wasm-pack/)** — builds the WASM package
- The **`wasm32-unknown-unknown`** Rust target:

```bash
rustup target add wasm32-unknown-unknown
```

### 1. Clone the Repository

```bash
git clone https://github.com/EdwinAlkins/protoruf.git
cd protoruf
```

### 2. Install Dependencies

Python:

```bash
uv sync --dev --group benchmark --group docs
```

This installs all development dependencies including `maturin`, `pytest`, and `mypy`.

Node.js / WASM (only if you work on those bindings):

```bash
npm ci
```

This installs the JS dev dependencies (`@napi-rs/cli`, `vitest`, `protobufjs`).

### 3. Build the Bindings

Python extension:

```bash
uv run maturin develop
```

This compiles the Rust code and installs the Python package in development mode.

Node.js addon (napi) and WASM package:

```bash
# Both at once (native addon + WASM)
npm run build:js

# Or individually
npm run build:debug   # native Node addon (napi) -> dist/
npm run build:wasm    # WASM package           -> dist/wasm/
```

`npm run test:js` runs `build:js` automatically (via `pretest:js`), so you rarely
need to build by hand before testing.

## Project Structure

```
protoruf/
├── python/protoruf/           # Python package
│   ├── __init__.py            # Public API wrappers
│   ├── _protoruf.pyi          # Type stubs for Rust extension
│   ├── compiler.py            # Proto compilation utilities
│   └── py.typed               # PEP 561 type marker
├── src/                       # Rust source code
│   ├── lib.rs                 # Crate root + feature-gated binding modules
│   ├── core.rs                # Core logic (pure Rust, no FFI)
│   ├── python.rs              # Python bindings (PyO3, feature "python")
│   ├── node.rs                # Node.js bindings (napi, feature "node")
│   └── wasm.rs                # Browser/WASM bindings (wasm-bindgen, feature "wasm")
├── tests/                     # Test suites
│   ├── proto/                 # Test proto files
│   ├── test_models.py         # Pydantic test models
│   ├── test_*.py              # Python tests
│   └── js/                    # JS tests (node + wasm + parity, vitest)
├── examples/                  # Usage examples (Python + examples/js/)
├── package.json               # Node build/test scripts (napi, wasm-pack, vitest)
└── docs/                      # Documentation
```

Each binding is a thin wrapper over `src/core.rs` and is gated behind its own
Cargo feature (`python`, `node`, `wasm`), so building one never pulls in the
others' dependencies.

## Development Workflow

### Make Changes

1. Edit Rust code in `src/` (shared logic in `core.rs`, bindings in `python.rs` / `node.rs` / `wasm.rs`)
2. Edit Python code in `python/protoruf/`, or JS/TS in `examples/js/` and `tests/js/`
3. Rebuild the affected binding(s):

```bash
uv run maturin develop   # Python
npm run build:js         # Node addon + WASM
```

### Run Tests

#### Python Tests

```bash
uv run pytest tests/ -v
```

#### Rust Tests

Unit tests live in `src/*.rs` under `#[cfg(test)]`. Run them across all targets
(the same as CI):

```bash
cargo test --all-targets
```

#### Node / WASM Tests (JavaScript)

The vitest suite (`tests/js/`) covers the native Node addon, the WASM build, and
node↔wasm parity. `pretest:js` builds both bindings first, so a single command
does everything:

```bash
npm run test:js
```

#### All Tests

```bash
uv run pytest tests/ -v && cargo test --all-targets && npm run test:js
```

### Rust Validation (format & lint)

These are the exact commands CI runs (`.github/workflows/rust-test.yml`) — run
them before pushing:

```bash
# Format the code in place
cargo fmt --all

# Validation (what CI checks): formatting is correct, no clippy warnings, tests pass
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

`cargo clippy` treats every warning as an error (`-D warnings`), so the build
fails on any lint. Run `cargo fmt --all` first to auto-fix formatting.

### Type Checking

```bash
uv run mypy python/protoruf/ --ignore-missing-imports
```

### Pre-commit Checklist

Before committing, ensure all checks pass:

```bash
# 1. Python tests
uv run pytest tests/ -v

# 2. Rust formatting (CI checks this)
cargo fmt --all -- --check

# 3. Rust lints (warnings fail the build)
cargo clippy --all-targets -- -D warnings

# 4. Rust tests
cargo test --all-targets

# 5. Type checking
uv run mypy python/protoruf/ --ignore-missing-imports

# 6. Build verification
uv run maturin develop

# 7. Node / WASM tests (if you touched core.rs or the node/wasm bindings)
npm run test:js
```

## Adding Features

### Rust Changes

1. Add logic to `src/core.rs` (pure Rust, the single source of truth)
2. Expose it in each binding you support: `src/python.rs` (PyO3), `src/node.rs`
   (napi), `src/wasm.rs` (wasm-bindgen) — keep the signatures consistent
3. Update type stubs in `python/protoruf/_protoruf.pyi`
4. Write tests in Rust (`#[cfg(test)]` in `core.rs`), Python (`tests/`), and JS (`tests/js/`)

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

- Follow standard Rust conventions — format with `cargo fmt --all`
- Keep `cargo clippy --all-targets -- -D warnings` clean (no warnings)
- Use descriptive variable names
- Add comments for complex logic
- Write unit tests for core functions

### Python

- Follow PEP 8
- Use type hints everywhere
- Keep functions focused and small
- Docstrings for all public functions

## Building for Distribution

### Build Wheels (Python)

```bash
uv run maturin build
```

This creates distributable wheels in `target/wheels/`.

### Build npm Packages (Node / WASM)

```bash
npm run build              # release native addon (napi)
npm run pack:wasm          # release WASM package for the browser
```

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
