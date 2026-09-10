#!/usr/bin/env python3
"""Build the shared KITT native Python wheel from kitt-toolbox."""
from __future__ import annotations

import argparse
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "crates" / "kitt-native-python" / "Cargo.toml"


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
        check=True,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
