# Manual do K.I.T.T. Toolbox (`kitt-toolbox`)

> Data plane nativo do ecossistema K.I.T.T. para busca, símbolos, edição determinística, leitura limitada do workspace e compressão de saída.

## 1. Escopo

O `kitt-toolbox` contém apenas capacidades nativas efetivamente consumidas pelo ecossistema:

- `kitt-native-engine`: núcleo Rust de code intelligence;
- `kitt-native-python`: bridge PyO3 exportada como `kitt_native`;
- busca textual e por regex;
- descoberta/leitura de símbolos e referências;
- edição de símbolos com validação;
- leitura/listagem limitada do workspace;
- compressão limitada de saída de processos.

Sondagem de CPU/RAM/disco não faz parte deste componente. O antigo executável de snapshot não era instalado nem utilizado pelos demais módulos e foi removido.

## 2. Requisitos

- Rust 1.85+;
- Python 3.12+ para construir/testar o wheel;
- `maturin>=1.8,<2` para empacotamento Python.

## 3. Validação Rust

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## 4. Build do backend Python

```bash
python -m pip install 'maturin>=1.8,<2'
python packaging/build_native_release.py --out dist-native
python -m pip install --no-deps dist-native/*.whl
python -c "import kitt_native; print(kitt_native.ENGINE_VERSION)"
```

O wheel usa ABI estável do PyO3 e é descoberto automaticamente pelo `kitt-agent-cli`. Se a extensão não estiver disponível, o Agent mantém seus fallbacks portáteis onde suportado.

## 5. Responsabilidade arquitetural

O Toolbox não decide autorização, autonomia ou política de ferramentas. Ele fornece operações determinísticas e limitadas; a autoridade permanece no control plane do `kitt-agent-cli`.

Também não depende de `kitt-protocol`: adaptação de IPC/transporte pertence ao processo hospedeiro.
