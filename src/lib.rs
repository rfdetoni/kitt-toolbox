use serde::Serialize;
use sysinfo::{Disks, System};

#[derive(Debug, Serialize)]
pub struct ResourceSnapshot {
    pub total_memory_bytes: u64,
    pub used_memory_bytes: u64,
    pub available_memory_bytes: u64,
    pub cpu_usage_percent: f32,
    pub disks: Vec<DiskSnapshot>,
}
#[derive(Debug, Serialize)]
pub struct DiskSnapshot {
    pub mount: String,
    pub total_bytes: u64,
    pub available_bytes: u64,
}

pub fn snapshot() -> ResourceSnapshot {
    let mut sys = System::new_all();
    sys.refresh_all();
    let cpu = if sys.cpus().is_empty() {
        0.0
    } else {
        sys.cpus().iter().map(|c| c.cpu_usage()).sum::<f32>() / sys.cpus().len() as f32
    };
    let disks = Disks::new_with_refreshed_list()
        .iter()
        .map(|d| DiskSnapshot {
            mount: d.mount_point().to_string_lossy().into_owned(),
            total_bytes: d.total_space(),
            available_bytes: d.available_space(),
        })
        .collect();
    ResourceSnapshot {
        total_memory_bytes: sys.total_memory(),
        used_memory_bytes: sys.used_memory(),
        available_memory_bytes: sys.available_memory(),
        cpu_usage_percent: cpu,
        disks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snapshot_returns_valid_data() {
        let snap = snapshot();
        assert!(snap.total_memory_bytes > 0);
        assert!(!snap.disks.is_empty());
    }
}
