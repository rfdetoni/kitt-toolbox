#!/usr/bin/env python3
"""Build the shared KITT native Python wheel from kitt-toolbox."""
from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "crates" / "kitt-native-python" / "Cargo.toml"


def build_environment() -> dict[str, str]:
    """Return an environment suitable for PyO3's stable-ABI extension build.

    kitt-native-python is compiled with PyO3's ``abi3`` feature. PyO3 releases
    intentionally reject CPython versions newer than the release knew about
    unless stable-ABI forward compatibility is explicitly opted into. This is
    safe for this crate because its Cargo features already restrict it to the
    stable Python ABI.
    """
    env = os.environ.copy()
    if sys.implementation.name == "cpython" and sys.version_info[:2] >= (3, 14):
        env.setdefault("PYO3_USE_ABI3_FORWARD_COMPATIBILITY", "1")
    return env


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", type=Path, default=ROOT / "dist-native")
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)

    maturin_bin = shutil.which("maturin")
    cmd = [maturin_bin] if maturin_bin else [sys.executable, "-m", "maturin"]
    subprocess.run(
        [
            *cmd,
            "build",
            "--release",
            "--manifest-path",
            str(MANIFEST),
            "--out",
            str(args.out.resolve()),
        ],
        cwd=ROOT,
        env=build_environment(),
        check=True,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
