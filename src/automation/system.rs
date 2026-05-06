// System-information snapshots: CPU, memory, top processes. Cheap to
// refresh on a timer in the UI thread.

use serde::Serialize;
use sysinfo::{ProcessesToUpdate, System};

#[derive(Debug, Clone, Serialize)]
pub struct SystemSnapshot {
    pub host: String,
    pub os: String,
    pub kernel: String,
    pub cpu_count: usize,
    pub cpu_usage_percent: f32,
    pub mem_used_mb: u64,
    pub mem_total_mb: u64,
    pub uptime_secs: u64,
    pub load_avg_one: f64,
    pub top_processes: Vec<ProcessInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cpu: f32,
    pub mem_mb: u64,
}

pub struct SystemMonitor {
    system: System,
}

impl SystemMonitor {
    pub fn new() -> Self {
        let mut system = System::new_all();
        system.refresh_all();
        Self { system }
    }

    pub fn snapshot(&mut self) -> SystemSnapshot {
        self.system.refresh_cpu_usage();
        self.system.refresh_memory();
        self.system.refresh_processes(ProcessesToUpdate::All);

        let cpu_usage_percent = self.system.global_cpu_usage();
        let mem_used_mb = self.system.used_memory() / 1024 / 1024;
        let mem_total_mb = self.system.total_memory() / 1024 / 1024;

        let mut procs: Vec<ProcessInfo> = self
            .system
            .processes()
            .iter()
            .map(|(pid, p)| ProcessInfo {
                pid: pid.as_u32(),
                name: p.name().to_string_lossy().to_string(),
                cpu: p.cpu_usage(),
                mem_mb: p.memory() / 1024 / 1024,
            })
            .collect();
        procs.sort_by(|a, b| b.cpu.partial_cmp(&a.cpu).unwrap_or(std::cmp::Ordering::Equal));
        procs.truncate(10);

        let load_avg_one = System::load_average().one;

        SystemSnapshot {
            host: System::host_name().unwrap_or_else(|| "unknown".to_string()),
            os: System::long_os_version().unwrap_or_else(|| "unknown".to_string()),
            kernel: System::kernel_version().unwrap_or_else(|| "unknown".to_string()),
            cpu_count: self.system.cpus().len(),
            cpu_usage_percent,
            mem_used_mb,
            mem_total_mb,
            uptime_secs: System::uptime(),
            load_avg_one,
            top_processes: procs,
        }
    }
}
