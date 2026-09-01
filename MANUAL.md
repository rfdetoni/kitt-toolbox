# Manual do K.I.T.T. Toolbox (`kitt-toolbox`)

> Ferramentas de sistema nativas, diagnósticos de baixo overhead, coleta de métricas e execução segura de sondas para o ecossistema K.I.T.T.

---

## 1. Visão Geral

O **`kitt-toolbox`** fornece utilitários e sondas de diagnóstico escritas em Rust com consumo mínimo de recursos de CPU e memória.
Ele permite que o agente e o assistente obtenham instantâneos (*snapshots*) do estado da máquina, processos em execução, uso de RAM/Swap, disco e status do repositório Git sem a necessidade de comandos bash custosos.

---

## 2. Requisitos de Sistema

- **Rust**: 1.80+ (com `cargo`)
- Compatível nativamente com **Linux**, **macOS** e **Windows**.

---

## 3. Instalação e Compilação por Sistema Operacional

### 🐧 A. LINUX
```bash
cargo build --release
cargo test
sudo cp target/release/kitt-toolbox /usr/local/bin/ # Opcional
```

### 🍏 B. macOS
```bash
cargo build --release
cargo test
cp target/release/kitt-toolbox /usr/local/bin/ # Opcional
```

### 🪟 C. WINDOWS (PowerShell)
```powershell
cargo build --release
cargo test
# O executável estará em target\release\kitt-toolbox.exe
```

---

## 4. Guia de Uso e Comandos da CLI

### 1. Obter Snapshot Completo do Sistema em JSON:
```bash
cargo run --release -- snapshot
```
*Saída de exemplo:*
```json
{
  "timestamp": 1787330000,
  "system": {
    "os": "linux",
    "cpu_count": 8,
    "memory_total_bytes": 17179869184,
    "memory_available_bytes": 10737418240
  },
  "git": {
    "branch": "main",
    "clean": true
  }
}
```

### 2. Verificar Recursos de Memória e Carga de CPU:
```bash
cargo run --release -- probe system
```

### 3. Inspecionar Estado de Processos Locais do KITT:
```bash
cargo run --release -- probe processes
```

---

## 5. Validação e Testes
```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```
