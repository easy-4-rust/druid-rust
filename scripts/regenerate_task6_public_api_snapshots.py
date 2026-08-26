#!/usr/bin/env python3
"""Regenerate all three Task 6 public-API snapshots with the pinned toolchain."""

from __future__ import annotations

import hashlib
from pathlib import Path

import verify_no_physical_standard as no_physical
import verify_task6_public_contract as public_contract


SNAPSHOTS = {
    "druid-rdbc": Path("api/druid-rdbc.single-standard.after.txt"),
    "druid": Path("api/druid.single-standard.after.txt"),
    "druid-wrapper": Path("api/druid-wrapper.single-standard.after.txt"),
}


def main() -> int:
    """Generate in memory, validate retired identities, then replace all snapshots."""

    root = Path(__file__).resolve().parent.parent
    generated = {
        "druid-rdbc": no_physical.generate_public_api(root),
        "druid": public_contract.generate_api(root, "druid"),
        "druid-wrapper": public_contract.generate_api(root, "druid-wrapper"),
    }
    for package, text in generated.items():
        no_physical.verify_text_artifact(f"{package} public API", text)
        if not text.strip():
            raise RuntimeError(f"{package} public API is empty")
    for package, text in generated.items():
        target = root / SNAPSHOTS[package]
        target.write_text(text, encoding="utf-8", newline="\n")
        print(f"{package.upper().replace('-', '_')}_PUBLIC_API_LINES={len(text.splitlines())}")
        print(
            f"{package.upper().replace('-', '_')}_PUBLIC_API_SHA256="
            f"{hashlib.sha256(text.encode('utf-8')).hexdigest()}"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
