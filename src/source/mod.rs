//! Process sampling: the data model shared by the real and fake sources.

use std::collections::HashMap;

use anyhow::Result;

/// Coarse process state, mapped from the platform's finer-grained view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcStatus {
    Running,
    Sleeping,
    Zombie,
    Other,
}

/// A single process as seen at one sampling instant.
#[derive(Debug, Clone, PartialEq)]
pub struct ProcInfo {
    pub pid: u32,
    pub parent: Option<u32>,
    pub name: String,
    /// Percent of one core. Can exceed 100 when a process uses several cores.
    pub cpu: f32,
    /// Resident memory in bytes.
    pub memory: u64,
    pub status: ProcStatus,
    pub user: Option<String>,
    /// Seconds since the epoch when the process started. Together with `pid` this
    /// forms the identity of a process, so a reused PID is a birth, not a change.
    pub start_time: u64,
    /// True for kernel threads (Linux: no executable path and parent 2, or pid 2).
    pub kernel: bool,
    /// True when the process appears to run inside a container.
    pub container: bool,
}

impl ProcInfo {
    /// Identity used to detect PID reuse.
    pub fn identity(&self) -> (u32, u64) {
        (self.pid, self.start_time)
    }

    /// Ranking score: CPU plus a log-scaled memory term, so one whale does not
    /// dwarf everything else.
    pub fn score(&self) -> f32 {
        let mib = self.memory as f32 / (1024.0 * 1024.0);
        self.cpu + (mib + 1.0).log2() * 2.0
    }
}

/// The whole process table at one instant.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Snapshot {
    pub procs: HashMap<u32, ProcInfo>,
    /// Overall CPU load in `0.0..=1.0`, used to darken the water as the system
    /// gets busier (the day/night cycle).
    pub load: f32,
}

impl Snapshot {
    pub fn new(procs: HashMap<u32, ProcInfo>) -> Self {
        Self { procs, load: 0.0 }
    }

    pub fn with_load(mut self, load: f32) -> Self {
        self.load = load.clamp(0.0, 1.0);
        self
    }
}

/// Something that can hand out process snapshots.
pub trait ProcessSource {
    fn snapshot(&mut self) -> Result<Snapshot>;

    /// Nudge `pid` towards a slightly higher priority. Opt-in and best-effort:
    /// on Unix this lowers the nice value, which needs `CAP_SYS_NICE`, so it
    /// commonly comes back `Denied`. The default is a no-op so fakes and
    /// platforms without priorities stay quiet.
    fn boost_priority(&mut self, _pid: u32) -> PriorityBoost {
        PriorityBoost::Unsupported
    }
}

/// Result of a [`ProcessSource::boost_priority`] attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriorityBoost {
    Applied,
    Denied,
    Unsupported,
}

#[cfg(test)]
pub mod fake;
pub mod sysinfo_source;

#[cfg(test)]
mod tests {
    use super::*;

    fn proc(pid: u32, memory: u64, cpu: f32) -> ProcInfo {
        ProcInfo {
            pid,
            parent: None,
            name: format!("p{pid}"),
            cpu,
            memory,
            status: ProcStatus::Running,
            user: None,
            start_time: 1,
            kernel: false,
            container: false,
        }
    }

    #[test]
    fn score_grows_with_cpu_and_memory() {
        let small = proc(1, 1024 * 1024, 5.0);
        let big = proc(2, 1024 * 1024 * 1024, 5.0);
        assert!(big.score() > small.score());

        let hot = proc(3, 1024 * 1024, 80.0);
        assert!(hot.score() > small.score());
    }

    #[test]
    fn identity_is_pid_and_start_time() {
        let mut p = proc(7, 0, 0.0);
        assert_eq!(p.identity(), (7, 1));
        p.start_time = 2;
        assert_eq!(p.identity(), (7, 2));
    }

    #[test]
    fn snapshot_load_is_clamped() {
        assert_eq!(Snapshot::default().with_load(2.0).load, 1.0);
        assert_eq!(Snapshot::default().with_load(-1.0).load, 0.0);
        assert_eq!(Snapshot::default().with_load(0.4).load, 0.4);
    }
}
