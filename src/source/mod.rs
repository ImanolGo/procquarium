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
    /// Disk bytes read and written since the last sample, for the I/O bubbles.
    pub io: u64,
    /// Seconds the process has been running, for the barnacles on old fish.
    pub run_time: u64,
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

    /// Nudge a process towards a slightly higher priority. Opt-in and
    /// best-effort: on Unix this lowers the nice value, which needs
    /// `CAP_SYS_NICE`, so it commonly comes back `Denied`. The default is a
    /// no-op so fakes and platforms without priorities stay quiet.
    fn boost_priority(&mut self, _id: (u32, u64)) -> PriorityBoost {
        PriorityBoost::Unsupported
    }

    /// Undo the boost for one process, if it was boosted.
    fn restore_priority(&mut self, _id: (u32, u64)) {}

    /// Undo every boost (called on the way out).
    fn restore_all_priorities(&mut self) {}

    /// Send SIGTERM to a process, identified by pid and start time. Opt-in
    /// (`--kill`). A source must re-check the process just before signalling and
    /// refuse when the start time no longer matches, so a reused pid is never
    /// signalled. The default is a no-op.
    fn kill_term(&mut self, _id: (u32, u64)) -> KillOutcome {
        KillOutcome::Failed
    }
}

/// What happened when a [`ProcessSource::kill_term`] was attempted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KillOutcome {
    /// SIGTERM was delivered.
    Signalled,
    /// The process had already exited.
    AlreadyExited,
    /// The pid now belongs to a different process (its start time changed), so
    /// nothing was signalled.
    PidReused,
    /// The signal could not be sent (for example, permission denied).
    Failed,
}

/// Result of a [`ProcessSource::boost_priority`] attempt.
///
/// `Applied` and `Denied` are only produced on Unix; the other platforms always
/// return `Unsupported`, so silence the dead-code lint there.
#[cfg_attr(not(unix), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriorityBoost {
    Applied,
    Denied,
    Unsupported,
}

#[cfg(test)]
pub mod fake;
pub mod priority;
pub mod record;
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
            io: 0,
            run_time: 0,
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

    #[test]
    fn a_matching_process_is_signalled() {
        use crate::source::fake::{FakeSource, proc, snapshot};

        let mut source = FakeSource::new(vec![snapshot(vec![proc(1, "a").with_start_time(1)])]);
        source.snapshot().expect("sample");
        assert_eq!(source.kill_term((1, 1)), KillOutcome::Signalled);
        assert_eq!(source.kills, vec![(1, 1)]);
    }

    #[test]
    fn a_reused_pid_is_not_signalled() {
        use crate::source::fake::{FakeSource, proc, snapshot};

        // pid 1 exits and the pid is reused by a new process before the kill is
        // confirmed.
        let mut source = FakeSource::new(vec![
            snapshot(vec![proc(1, "old").with_start_time(1)]),
            snapshot(vec![proc(1, "new").with_start_time(2)]),
        ]);
        source.snapshot().expect("first sample");
        source.snapshot().expect("second sample");

        // The confirmed request was for the old process, which is gone.
        assert_eq!(source.kill_term((1, 1)), KillOutcome::PidReused);
        assert!(
            source.kills.is_empty(),
            "the reused pid must not be signalled"
        );

        // A process that has fully exited is reported as gone.
        assert_eq!(source.kill_term((2, 1)), KillOutcome::AlreadyExited);
    }
}
