#!/usr/bin/env python3
"""Assert VERSION and Cargo.toml version are in sync."""
from __future__ import annotations

import re
import sys
from pathlib import Path


def extract_cargo_version(content: str) -> str:
    match = re.search(r"^version\s*=\s*\"([^\"]+)\"", content, re.MULTILINE)
    if not match:
        raise RuntimeError("Could not find version in Cargo.toml")
    return match.group(1).strip()


def main() -> int:
    root = Path(__file__).resolve().parents[1]

    version_path = root / "VERSION"
    if not version_path.is_file():
        print("VERSION file not found", file=sys.stderr)
        return 1

    canonical = version_path.read_text(encoding="utf-8").strip()
    if not canonical:
        print("VERSION file is empty", file=sys.stderr)
        return 1

    cargo_text = (root / "Cargo.toml").read_text(encoding="utf-8")
    cargo_version = extract_cargo_version(cargo_text)

    if cargo_version != canonical:
        print("Version mismatch detected:", file=sys.stderr)
        print(f"  VERSION:    {canonical}", file=sys.stderr)
        print(f"  Cargo.toml: {cargo_version}", file=sys.stderr)
        print("Run: python3 scripts/sync_cargo_version.py", file=sys.stderr)
        return 1

    print(f"Versions are consistent: {canonical}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
