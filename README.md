# KITT Toolbox

> Shared low-overhead system inspection and resource probe utilities for KITT.

Minimal, efficient Rust utility for capturing point-in-time system snapshots (CPU, memory, mount points, and disk space) without bloat.

---

## 🚀 CLI Usage

```bash
# Output JSON snapshot of host system resources
cargo run --release -- snapshot
```

Sample output:
```json
{
  "total_memory_bytes": 10352185344,
  "used_memory_bytes": 4653096960,
  "available_memory_bytes": 5699088384,
  "cpu_usage_percent": 1.5,
  "disks": [
    {
      "mount": "/",
      "total_bytes": 249792131072,
      "available_bytes": 159918665728
    }
  ]
}
```

---

## 🛠️ Library Usage

```rust
use kitt_toolbox::snapshot;

let res = snapshot();
println!("CPU: {:.1}% | Available RAM: {} MB", res.cpu_usage_percent, res.available_memory_bytes / 1024 / 1024);
```

---

## 🧪 Testing

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

---

## 📄 License

MIT License. See [LICENSE](LICENSE).
