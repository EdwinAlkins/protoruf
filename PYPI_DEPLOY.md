# PyPI Deployment Guide

This guide explains how to build and publish `protoruf` to PyPI using GitHub Actions.

## Prerequisites

1. A PyPI account at [pypi.org](https://pypi.org)
2. A GitHub repository with the code
3. Python 3.12+ and `uv` installed locally for testing

## Setup PyPI Publishing

### Option 1: Trusted Publishing (Recommended)

This is the most secure method and doesn't require storing long-lived tokens.

1. Go to [PyPI Account Settings](https://pypi.org/manage/account/publishing/)
2. Under **Trusted Publishers**, click **Add a new pending publisher**
3. Fill in:
   - **Project name**: `protoruf`
   - **Owner**: your GitHub username or organization
   - **Repository name**: `protoruf`
   - **Workflow name**: `.github/workflows/publish-pypi.yml`
   - **Environment name**: `pypi`
4. Submit and confirm

### Option 2: API Token

1. Go to [PyPI API Tokens](https://pypi.org/manage/account/token/)
2. Click **Add API token**
3. Set scope to `Entire account` or specific to `protoruf`
4. Copy the token (you'll only see it once!)
5. Go to your GitHub repo → Settings → Secrets and variables → Actions
6. Add a new secret:
   - **Name**: `PYPI_API_TOKEN`
   - **Value**: the token you copied

If using this method, update the `publish` job in `.github/workflows/publish-pypi.yml`:

```yaml
  publish:
    needs: [linux, windows, macos, sdist]
    runs-on: ubuntu-latest
    steps:
      - uses: actions/download-artifact@v4
      - name: Publish to PyPI
        uses: pypa/gh-action-pypi-publish@release/v1
        with:
          password: ${{ secrets.PYPI_API_TOKEN }}
```

## Release Process

### 1. Update the version

Update the version in both files:

- `Cargo.toml`:
  ```toml
  [package]
  name = "protoruf"
  version = "0.1.2"  # ← update here
  ```

- `pyproject.toml`:
  ```toml
  [project]
  name = "protoruf"
  version = "0.1.2"  # ← update here
  ```

### 2. Commit and push

```bash
git add Cargo.toml pyproject.toml
git commit -m "bump version to 0.1.2"
git push
```

### 3. Run checks locally

```bash
# Python tests
uv run pytest tests/ -v

# Rust tests
cargo test --lib

# Type checking
uv run mypy python/protoruf/ --ignore-missing-imports

# Build verification
uv run maturin develop
```

### 4. Create a Git tag and release

```bash
git tag v0.1.2
git push origin v0.1.2
```

Then go to GitHub → your repo → **Releases** → **Draft a new release**:

- **Tag**: `v0.1.2`
- **Title**: `v0.1.2`
- **Description**: changelog/release notes
- Click **Publish release**

This triggers the GitHub Actions workflow automatically.

### 5. Monitor the workflow

Go to **Actions** tab in your GitHub repo → click on the running workflow → check each job completes successfully.

Once done, the package will be available on PyPI at:
```
https://pypi.org/project/protoruf/
```

## Building wheels locally

### Install maturin

```bash
uv pip install maturin
```

### Build for your current platform

```bash
maturin build --release
```

Wheels will be in `target/wheels/`.

### Build a source distribution

```bash
maturin build --release --sdist
```

### Test a wheel locally

```bash
uv pip install target/wheels/protoruf-*.whl
python -c "from protoruf import json_to_protobuf; print('OK')"
```

## Supported platforms

The workflow builds wheels for:

| Platform | Architectures |
|----------|---------------|
| Linux    | x86_64, aarch64 |
| Windows  | x64 |
| macOS    | x86_64 (Intel), aarch64 (Apple Silicon) |

## Troubleshooting

### Build fails on a specific platform

- Check that all Rust dependencies compile for that target
- Run `maturin build --release --target <target>` locally to reproduce

### PyPI rejects the upload

- Version must be unique (can't re-upload same version)
- Bump the version and try again

### Trusted publishing doesn't work

- Verify the repo name and owner match exactly
- Check the environment name is `pypi`
- Ensure the workflow file path is correct

### Module not found after install

- Verify `[tool.maturin]` config in `pyproject.toml` is correct
- Check `python-source = "python"` points to the right directory
