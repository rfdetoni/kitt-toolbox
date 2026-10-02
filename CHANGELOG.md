# Changelog

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
