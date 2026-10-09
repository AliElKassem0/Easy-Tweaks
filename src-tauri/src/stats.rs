// Live system stats for the Dashboard, using the `sysinfo` crate (works on Windows and macOS).
use std::sync::Mutex;
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};

// Kept alive in Tauri's state: CPU usage is the difference between two refreshes,
// so sysinfo has to remember the previous measurement.
pub struct Stats(Mutex<System>);

impl Stats {
    pub fn new() -> Self {
        // Only load what we show (CPU + memory), not processes, disks, etc.
        let sys = System::new_with_specifics(
            RefreshKind::nothing()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything()),
        );
        Self(Mutex::new(sys))
    }
}

// Changes every second
#[derive(serde::Serialize)]
pub struct LiveStats {
    cpu_usage: f32, // percent, average over all cores since the last call
    ram_used: u64,  // bytes
    ram_total: u64, // bytes
}

// Doesn't change while the app runs
#[derive(serde::Serialize)]
pub struct SystemInfo {
    cpu_name: String,
    cpu_threads: usize,
    os: String,
}

#[tauri::command]
pub fn live_stats(stats: tauri::State<Stats>) -> LiveStats {
    let mut sys = stats.0.lock().unwrap();
    sys.refresh_cpu_usage();
    sys.refresh_memory();
    LiveStats {
        cpu_usage: sys.global_cpu_usage(),
        ram_used: sys.used_memory(),
        ram_total: sys.total_memory(),
    }
}

#[tauri::command]
pub fn system_info(stats: tauri::State<Stats>) -> SystemInfo {
    let sys = stats.0.lock().unwrap();
    SystemInfo {
        cpu_name: sys.cpus().first().map(|c| c.brand().trim().to_string()).unwrap_or_default(),
        cpu_threads: sys.cpus().len(),
        os: System::long_os_version().unwrap_or_default(),
    }
}
