# Protocol boundary

`kitt-toolbox` intentionally does **not** depend on `kitt-protocol`.

The toolbox exposes native resource and tooling primitives. The process hosting or invoking those primitives owns IPC adaptation. Keeping transport concerns out of this low-level crate preserves reuse, startup simplicity and a smaller dependency graph.

Required validation:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```
