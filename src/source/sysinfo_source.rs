//! Real process sampling backed by [`sysinfo`].

use std::collections::HashMap;

use anyhow::Result;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind, Users};

use super::{PriorityBoost, ProcInfo, ProcStatus, ProcessSource, Snapshot};

/// Live process source. A fresh `SysinfoSource` performs one refresh on
/// construction; callers should wait at least
/// [`sysinfo::MINIMUM_CPU_UPDATE_INTERVAL`] before the first `snapshot()` so the
/// CPU numbers are meaningful.
pub struct SysinfoSource {
    system: System,
    users: Users,
    /// Number of logical CPUs, for turning summed process CPU into a 0..=1 load.
    ncpu: f32,
    /// Cached container classification keyed by pid, tagged with the start time
    /// it was computed for so a reused PID is re-checked.
    container_cache: HashMap<u32, (u64, bool)>,
}

impl SysinfoSource {
    pub fn new() -> Self {
        let mut system = System::new();
        system.refresh_processes_specifics(ProcessesToUpdate::All, true, refresh_kind());
        let ncpu = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1) as f32;
        Self {
            system,
            users: Users::new_with_refreshed_list(),
            ncpu,
            container_cache: HashMap::new(),
        }
    }
}

impl Default for SysinfoSource {
    fn default() -> Self {
        Self::new()
    }
}

fn refresh_kind() -> ProcessRefreshKind {
    ProcessRefreshKind::nothing()
        .with_memory()
        .with_cpu()
        .with_user(UpdateKind::OnlyIfNotSet)
        .with_exe(UpdateKind::OnlyIfNotSet)
        // `nothing()` still enables `tasks` by default, which on Linux means every
        // thread is reported as a process. A fish is a process, so opt out.
        .without_tasks()
}

fn map_status(status: sysinfo::ProcessStatus) -> ProcStatus {
    match status {
        sysinfo::ProcessStatus::Run => ProcStatus::Running,
        sysinfo::ProcessStatus::Sleep | sysinfo::ProcessStatus::Idle => ProcStatus::Sleeping,
        sysinfo::ProcessStatus::Zombie => ProcStatus::Zombie,
        _ => ProcStatus::Other,
    }
}

/// Best-effort container detection. On Linux a process inside a container has a
/// cgroup path naming the runtime; everywhere else we cannot tell, so we say no.
#[cfg(target_os = "linux")]
fn detect_container(pid: u32) -> bool {
    const HINTS: [&str; 8] = [
        "docker",
        "kubepods",
        "containerd",
        "libpod",
        "podman",
        "lxc",
        "garden",
        "sandbox",
    ];
    match std::fs::read_to_string(format!("/proc/{pid}/cgroup")) {
        Ok(contents) => {
            let lower = contents.to_ascii_lowercase();
            HINTS.iter().any(|hint| lower.contains(hint))
        }
        Err(_) => false,
    }
}

#[cfg(not(target_os = "linux"))]
fn detect_container(_pid: u32) -> bool {
    false
}

impl ProcessSource for SysinfoSource {
    fn snapshot(&mut self) -> Result<Snapshot> {
        self.system
            .refresh_processes_specifics(ProcessesToUpdate::All, true, refresh_kind());

        let mut procs = HashMap::with_capacity(self.system.processes().len());
        let mut cpu_total = 0.0f32;
        for (pid, process) in self.system.processes() {
            let pid = pid.as_u32();
            let parent = process.parent().map(|p| p.as_u32());
            let exe = process.exe();
            let name = process.name().to_string_lossy().into_owned();
            let kernel = exe.is_none() && (parent == Some(2) || pid == 2);
            let start_time = process.start_time();
            let container = match self.container_cache.get(&pid) {
                Some((cached_at, value)) if *cached_at == start_time => *value,
                _ => {
                    let value = detect_container(pid);
                    self.container_cache.insert(pid, (start_time, value));
                    value
                }
            };
            let user = process
                .user_id()
                .and_then(|id| self.users.get_user_by_id(id))
                .map(|u| u.name().to_string());
            let cpu = process.cpu_usage();
            cpu_total += cpu;

            procs.insert(
                pid,
                ProcInfo {
                    pid,
                    parent,
                    name,
                    cpu,
                    memory: process.memory(),
                    status: map_status(process.status()),
                    user,
                    start_time,
                    kernel,
                    container,
                },
            );
        }

        // Drop classifications for processes that are gone.
        self.container_cache
            .retain(|pid, _| procs.contains_key(pid));

        // Summed per-core CPU over every process gives total load on the box.
        let load = (cpu_total / (self.ncpu * 100.0)).clamp(0.0, 1.0);
        Ok(Snapshot::new(procs).with_load(load))
    }

    /// Raise priority by lowering niceness, but never past -20. This normally
    /// needs privileges, so an unprivileged user will see `Denied`.
    #[cfg(unix)]
    fn boost_priority(&mut self, pid: u32) -> PriorityBoost {
        let nice = unsafe { libc::getpriority(libc::PRIO_PROCESS, pid as libc::id_t) };
        let nice = if (-20..=19).contains(&nice) { nice } else { 0 };
        let target = (nice - 1).max(-20);
        if target == nice {
            return PriorityBoost::Applied;
        }
        let rc = unsafe { libc::setpriority(libc::PRIO_PROCESS, pid as libc::id_t, target) };
        if rc == 0 {
            PriorityBoost::Applied
        } else {
            PriorityBoost::Denied
        }
    }

    #[cfg(not(unix))]
    fn boost_priority(&mut self, _pid: u32) -> PriorityBoost {
        PriorityBoost::Unsupported
    }
}
