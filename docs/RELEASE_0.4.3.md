# kitt-toolbox 0.4.3 — Contained and bounded native symbol queries

Native symbol reads, indexing, references and dependency reads use contained snapshots and reject traversal, absolute paths and symlinks. Each source is bounded to 4 MiB. Scans have file, entry, byte, symbol and elapsed-time budgets and expose partial-index metadata. Repeated queries reuse a scan for one second; KITT-owned writes invalidate changed paths immediately. Direct symbol reads parse the current bounded snapshot to avoid stale offsets. External edits become discoverable on the next scan after the one-second interval.

## Verification

Regression checks cover the concrete bugs fixed by this release. Native changes are validated with Rust formatting, Clippy, workspace tests and a Python 3.14 wheel integration. Live provider accounts and STT model inference are not part of these local checks.
