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
    needs: [wheels, sdist]
    runs-on: ubuntu-latest
    steps:
      - uses: actions/download-artifact@v4
      - name: Publish to PyPI
        uses: pypa/gh-action-pypi-publish@release/v1
        with:
          password: ${{ secrets.PYPI_API_TOKEN }}
```

## Release Process

### 1. Prepare the version

Update `Cargo.toml`, `Cargo.lock`, `pyproject.toml`, `package.json`,
`package-lock.json`, and `python/protoruf/__init__.py` to the same version. Add the release notes to
`CHANGELOG.md`. Before tagging, run:

```bash
python3 scripts/check-release-version.py v0.2.0
cargo test --locked
uv run pytest
```

Replace `v0.2.0` with the version being published.

### 2. Push the tag

```bash
git tag v0.2.0
git push origin v0.2.0
```

A tag push starts both release workflows independently. `release.yml` builds
the Node and WASM tarballs and attaches them to a GitHub Release.
`publish-pypi.yml` builds the Python wheels and sdist, then publishes them
through PyPI trusted publishing. The PyPI workflow no longer waits for a
`release: published` event created by the GitHub workflow's `GITHUB_TOKEN`.

Both workflows reject a tag that disagrees with package versions. Watch their
jobs under GitHub Actions. The PyPI workflow builds CPython 3.12, 3.13, and
3.14 wheels for Linux x86_64, Windows x64, macOS Intel, and macOS arm64 using
the Cargo `dist` profile.

## Building wheels locally

### Install maturin

```bash
uv pip install maturin
```

### Build for your current platform

```bash
maturin build --profile dist
```

Wheels will be in `target/wheels/`.

### Build a source distribution

```bash
maturin sdist
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
| Linux    | x86_64 |
| Windows  | x64 |
| macOS    | x86_64 (Intel), aarch64 (Apple Silicon) |

## Troubleshooting

### Build fails on a specific platform

- Check that all Rust dependencies compile for that target
- Run `maturin build --profile dist --target <target>` locally to reproduce

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
