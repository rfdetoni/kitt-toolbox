# Changelog — K.I.T.T. Toolbox

## Histórico migrado do README

## Release 0.4.3 — Contained and bounded native symbol queries

Native symbol reads, indexing, references and dependency reads use contained snapshots and reject traversal, absolute paths and symlinks. Each source is bounded to 4 MiB. Scans have file, entry, byte, symbol and elapsed-time budgets and expose partial-index metadata. Repeated queries reuse a scan for one second; KITT-owned writes invalidate changed paths immediately. Direct symbol reads parse the current bounded snapshot to avoid stale offsets. External edits become discoverable on the next scan after the one-second interval.

See [release notes](docs/RELEASE_0.4.3.md).

## Release 0.4.2 — same-line symbol ordering

Break top-k symbol ties by UTF-8 source offset and kind so duplicate names on one line retain source order. Keep the bounded borrowed-symbol optimization from 0.4.1. See [release notes](docs/RELEASE_0.4.2.md).

## Release 0.4.1 — execution boundary hardening

Filter borrowed symbols before cloning; select bounded top-k results before sorting. Keep deterministic exact-match priority and tie ordering, while preserving filesystem refresh and external-edit detection.

See [release notes](docs/RELEASE_0.4.1.md).

<p align="center">
  <strong>Shared native data plane for K.I.T.T.</strong><br>
  Rust code intelligence · bounded repository operations · PyO3 acceleration
</p>

<p align="center">
  <a href="https://github.com/rfdetoni/kitt-toolbox/blob/main/LICENSE"><img alt="License MIT" src="https://img.shields.io/badge/license-MIT-blue.svg"></a>
  <img alt="Rust 1.90+" src="https://img.shields.io/badge/Rust-1.90%2B-000000?logo=rust&logoColor=white">
  <img alt="Python native" src="https://img.shields.io/badge/Python-PyO3-3776AB?logo=python&logoColor=white">
</p>

K.I.T.T. Toolbox owns the deterministic native capabilities shared by the K.I.T.T. ecosystem. It provides the Rust `kitt-native-engine` used to accelerate repository search, symbol analysis, edits and bounded output handling behind the Agent’s stable runtime surface.

The Agent does not depend on native code semantically: when the `kitt_native` extension is unavailable it can use its portable Python fallback. This repository exists to make the hot data path faster without coupling model-facing behavior to Rust.

---

---

## Histórico anterior do CHANGELOG

## 0.4.3 — 2026-10-07

Native symbol reads, indexing, references and dependency reads use contained snapshots and reject traversal, absolute paths and symlinks. Each source is bounded to 4 MiB. Scans have file, entry, byte, symbol and elapsed-time budgets and expose partial-index metadata. Repeated queries reuse a scan for one second; KITT-owned writes invalidate changed paths immediately. Direct symbol reads parse the current bounded snapshot to avoid stale offsets. External edits become discoverable on the next scan after the one-second interval.

## 0.4.2 — 2026-10-06

- Preserve source order for duplicate symbol names on the same line at top-k cutoffs; revalidate bounded borrowed-symbol selection.

## 0.4.1 — 2026-10-06

- Filter borrowed symbols before cloning; select bounded top-k results before sorting. Keep deterministic exact-match priority and tie ordering, while preserving filesystem refresh and external-edit detection.

## 0.4.0 — 2026-10-02

- Remove unused lease-coordination types from the public native model surface; workspace coordination remains Agent-owned.

## 0.3.0 — 2026-10-02

- Add exact UTF-8 byte pagination with original line endings and explicit partial-line cursors.
- Parse and edit a symbol from one bounded file snapshot; serialize native edits and recheck the file before atomic persistence.
- Keep existing read_file positional arguments compatible; start_byte is optional.


## 0.2.9 - 2026-09-25

- Enable PyO3 ABI3 forward compatibility for CPython 3.14 builds.
- Preserve the existing stable-ABI bridge without introducing a PyO3/pythonize upgrade in the hot native boundary.


## 0.1.0 - Unreleased

- Initial KITT ecosystem foundation.
