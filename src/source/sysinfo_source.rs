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
}

impl SysinfoSource {
    pub fn new() -> Self {
        let mut system = System::new();
        system.refresh_processes_specifics(ProcessesToUpdate::All, true, refresh_kind());
        Self {
            system,
            users: Users::new_with_refreshed_list(),
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
        for (pid, process) in self.system.processes() {
            let pid = pid.as_u32();
            let parent = process.parent().map(|p| p.as_u32());
            let name = process.name().to_string_lossy().into_owned();
            let user = process
                .user_id()
                .and_then(|id| self.users.get_user_by_id(id))
                .map(|u| u.name().to_string());

            procs.insert(
                pid,
                ProcInfo {
                    pid,
                    parent,
                    name,
                    cpu: process.cpu_usage(),
                    memory: process.memory(),
                    status: map_status(process.status()),
                    user,
                    start_time: process.start_time(),
                },
            );
        }

        Ok(Snapshot::new(procs))
    }
}
