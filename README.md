# K.I.T.T. Toolbox

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

## What’s included

- `kitt-native-engine`: Rust repository/code-intelligence core.
- `kitt-native-python`: PyO3 extension exported as `kitt_native`.
- Workspace discovery and bounded file traversal.
- Text/repository search primitives.
- Symbol extraction and language-aware parsing.
- Deterministic edit operations.
- Bounded process/output representations.
- Language support built around Tree-sitter grammars for Python, Java, JavaScript, TypeScript, Rust and Go.
- Reproducible native-wheel build helper.

---

## Quick links

- **K.I.T.T. ecosystem:** https://github.com/rfdetoni/kitt
- **Agent CLI:** https://github.com/rfdetoni/kitt-agent-cli
- **Protocol boundary:** [PROTOCOL_BOUNDARY.md](PROTOCOL_BOUNDARY.md)
- **Manual:** [MANUAL.md](MANUAL.md)
- **Security:** [SECURITY.md](SECURITY.md)

---

## Requirements & compatibility

- Rust **1.90+** / edition 2024.
- Python is only required when building the PyO3 wheel.
- `maturin` is used by the native-wheel release helper.

The Python extension uses PyO3’s stable ABI (`abi3`) so the native backend can remain compatible across supported CPython versions without building a separate wheel for every interpreter minor version. Toolbox 0.2.9 explicitly enables PyO3 ABI3 forward compatibility so the existing PyO3 0.23 bridge can build against CPython 3.14 while keeping the stable ABI boundary.

---

## Architecture

```text
kitt-toolbox
│
├── crates/kitt-native-engine/
│   ├── workspace      bounded workspace traversal
│   ├── search         repository/text search
│   ├── symbols        language-aware symbol extraction
│   ├── language       Tree-sitter language support
│   ├── edit           deterministic source edits
│   ├── output         bounded output/data representations
│   └── model          shared native models
│
├── crates/kitt-native-python/
│   └── kitt_native    PyO3 bridge used by KITT Agent
│
└── packaging/
    └── native wheel build tooling
```

The ownership boundary is deliberate: `kitt-agent-cli` owns orchestration and policy; `kitt-toolbox` owns deterministic native execution primitives.

---

## Native Python acceleration

Build the shared `kitt_native` wheel:

```bash
python -m pip install maturin
python packaging/build_native_release.py
```

The helper builds `crates/kitt-native-python` in release mode and writes wheels to `dist-native/` by default.

K.I.T.T. Agent discovers the extension through its native bridge. No model-facing API changes when switching between the native and portable backends.

---

## Performance philosophy

Native code is used only where deterministic CPU/data work benefits from it. The goal is not to move orchestration into Rust; it is to make repository operations fast and predictable while preserving the simpler Python control plane.

Design priorities include bounded traversal/output, minimal serialization across the PyO3 boundary, native parsing/search where it pays off and no resident heavyweight runtime solely for acceleration.

---

## Security

Native operations are still downstream of Agent policy. The extension does not grant authority by itself: workspace containment, approvals and capability checks remain owned by the calling control plane.

Repository operations should remain deterministic, bounded and explicit. See [SECURITY.md](SECURITY.md) and [PROTOCOL_BOUNDARY.md](PROTOCOL_BOUNDARY.md) for component boundaries.

---

## Testing & linting

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Native-wheel behavior is also exercised by the ecosystem’s composed Agent validation.

---

## Contributing

Prefer native implementations for measured hot paths, not as a default rewrite strategy. Changes should preserve deterministic behavior, bounded resource use and compatibility with the Agent’s portable fallback.

---

## K.I.T.T. ecosystem

| Repository | Responsibility |
| --- | --- |
| [`kitt`](https://github.com/rfdetoni/kitt) | installer and ecosystem composition |
| [`kitt-agent-cli`](https://github.com/rfdetoni/kitt-agent-cli) | autonomous agent control plane |
| [`kitt-reverse-proxy`](https://github.com/rfdetoni/kitt-reverse-proxy) | authorized provider gateway |
| [`kitt-protocol`](https://github.com/rfdetoni/kitt-protocol) | shared contracts and SDKs |
| [`kitt-memory`](https://github.com/rfdetoni/kitt-memory) | persistent memory engine |
| [`kitt-ai-workers`](https://github.com/rfdetoni/kitt-ai-workers) | isolated AI/ML workers and evals |
| [`kitt-assistant`](https://github.com/rfdetoni/kitt-assistant) | resident assistant and Control Center |

---

## License

MIT. See [LICENSE](LICENSE).
