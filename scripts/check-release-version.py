"""Reject a release tag if any published package still has another version."""

import ast
import json
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def python_version() -> str:
    tree = ast.parse((ROOT / "python/protoruf/__init__.py").read_text())
    for statement in tree.body:
        if isinstance(statement, ast.Assign) and any(
            isinstance(target, ast.Name) and target.id == "__version__"
            for target in statement.targets
        ):
            if isinstance(statement.value, ast.Constant) and isinstance(
                statement.value.value, str
            ):
                return statement.value.value
    raise ValueError("python/protoruf/__init__.py has no literal __version__")


def main() -> int:
    if len(sys.argv) != 2 or not sys.argv[1].startswith("v"):
        print("usage: check-release-version.py vX.Y.Z", file=sys.stderr)
        return 2

    tag = sys.argv[1]
    version = tag[1:]
    cargo = tomllib.loads((ROOT / "Cargo.toml").read_text())
    lock = tomllib.loads((ROOT / "Cargo.lock").read_text())
    pyproject = tomllib.loads((ROOT / "pyproject.toml").read_text())
    package = json.loads((ROOT / "package.json").read_text())
    package_lock = json.loads((ROOT / "package-lock.json").read_text())
    locked = [
        item["version"]
        for item in lock["package"]
        if item["name"] == cargo["package"]["name"]
    ]
    versions = {
        "Cargo.toml": cargo["package"]["version"],
        "Cargo.lock": locked[0] if len(locked) == 1 else f"{len(locked)} entries",
        "pyproject.toml": pyproject["project"]["version"],
        "package.json": package["version"],
        "package-lock.json": package_lock["version"],
        "package-lock.json (root package)": package_lock["packages"][""]["version"],
        "python/protoruf/__init__.py": python_version(),
    }

    mismatches = {
        filename: actual
        for filename, actual in versions.items()
        if actual != version
    }
    if mismatches:
        print(f"Tag {tag} does not match package versions:", file=sys.stderr)
        for filename, actual in mismatches.items():
            print(f"  {filename}: {actual}", file=sys.stderr)
        return 1

    print(f"Release versions match {tag}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
