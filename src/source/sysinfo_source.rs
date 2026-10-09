//! Real process sampling backed by [`sysinfo`].

use std::collections::HashMap;

use anyhow::Result;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind, Users};

use super::{ProcInfo, ProcStatus, ProcessSource, Snapshot};

/// Live process source. A fresh `SysinfoSource` performs one refresh on
/// construction; callers should wait at least
/// [`sysinfo::MINIMUM_CPU_UPDATE_INTERVAL`] before the first `snapshot()` so the
/// CPU numbers are meaningful.
pub struct SysinfoSource {
    system: System,
    users: Users,
    /// Number of logical CPUs, for turning summed process CPU into a 0..=1 load.
    ncpu: f32,
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
                    start_time: process.start_time(),
                    kernel,
                },
            );
        }

        // Summed per-core CPU over every process gives total load on the box.
        let load = (cpu_total / (self.ncpu * 100.0)).clamp(0.0, 1.0);
        Ok(Snapshot::new(procs).with_load(load))
    }
}
