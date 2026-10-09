//! Scripted [`ProcessSource`] used by tests.

use std::collections::HashMap;

use anyhow::Result;

use super::{ProcInfo, ProcStatus, ProcessSource, Snapshot};

/// Returns previously scripted snapshots in order. Once the script runs out it
/// keeps returning the last snapshot, which makes it easy to hold a steady
/// state for a rendering test.
#[derive(Debug, Clone, Default)]
pub struct FakeSource {
    snapshots: Vec<Snapshot>,
    index: usize,
}

impl FakeSource {
    pub fn new(snapshots: Vec<Snapshot>) -> Self {
        Self {
            snapshots,
            index: 0,
        }
    }

    /// Convenience for tests that only care about rendering: a single snapshot
    /// is both the warm-up and the live sample.
    pub fn constant(procs: Vec<ProcInfo>) -> Self {
        let snap = snapshot(procs);
        Self::new(vec![snap.clone(), snap])
    }

    pub fn from_script(procs: impl IntoIterator<Item = Vec<ProcInfo>>) -> Self {
        Self::new(procs.into_iter().map(snapshot).collect())
    }
}

impl ProcessSource for FakeSource {
    fn snapshot(&mut self) -> Result<Snapshot> {
        let snap = self.snapshots.get(self.index).cloned().unwrap_or_default();
        if self.index + 1 < self.snapshots.len() {
            self.index += 1;
        }
        Ok(snap)
    }
}

/// Build a snapshot from a list of processes, keyed by pid.
pub fn snapshot(procs: Vec<ProcInfo>) -> Snapshot {
    Snapshot::new(
        procs
            .into_iter()
            .map(|p| (p.pid, p))
            .collect::<HashMap<_, _>>(),
    )
}

/// Fluent builder for test processes.
pub fn proc(pid: u32, name: &str) -> ProcInfo {
    ProcInfo {
        pid,
        parent: None,
        name: name.to_string(),
        cpu: 0.0,
        memory: 0,
        status: ProcStatus::Sleeping,
        user: None,
        start_time: 1,
        kernel: false,
        container: false,
    }
}

impl ProcInfo {
    pub fn with_cpu(mut self, cpu: f32) -> Self {
        self.cpu = cpu;
        self
    }

    pub fn with_memory(mut self, memory: u64) -> Self {
        self.memory = memory;
        self
    }

    pub fn with_parent(mut self, parent: u32) -> Self {
        self.parent = Some(parent);
        self
    }

    pub fn with_start_time(mut self, start_time: u64) -> Self {
        self.start_time = start_time;
        self
    }

    pub fn with_status(mut self, status: ProcStatus) -> Self {
        self.status = status;
        self
    }

    pub fn with_user(mut self, user: &str) -> Self {
        self.user = Some(user.to_string());
        self
    }

    pub fn kernel_thread(mut self) -> Self {
        self.kernel = true;
        self
    }

    pub fn container_process(mut self) -> Self {
        self.container = true;
        self
    }
}
