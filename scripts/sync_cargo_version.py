#!/usr/bin/env python3
"""Set Cargo.toml `version` from the repo VERSION file (Cargo has no file indirection)."""
from __future__ import annotations

import re
import sys
from pathlib import Path


def main() -> int:
    root = Path(__file__).resolve().parents[1]
    version = (root / "VERSION").read_text(encoding="utf-8").strip()
    if not version:
        print("VERSION file is empty", file=sys.stderr)
        return 1

    cargo_path = root / "Cargo.toml"
    text = cargo_path.read_text(encoding="utf-8")
    new_text, n = re.subn(
        r"^version\s*=\s*\"[^\"]+\"",
        f'version = "{version}"',
        text,
        count=1,
        flags=re.MULTILINE,
    )
    if n != 1:
        print("Could not find a single [package] version line in Cargo.toml", file=sys.stderr)
        return 1

    if new_text == text:
        return 0

    cargo_path.write_text(new_text, encoding="utf-8")
    print(f"Cargo.toml version set to {version} (from VERSION)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
