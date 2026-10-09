//! Turn two snapshots into a list of lifecycle events.

use crate::source::{ProcInfo, Snapshot};

/// What changed between two consecutive snapshots.
#[derive(Debug, Clone, PartialEq)]
pub enum ProcEvent {
    Spawned(ProcInfo),
    Exited {
        pid: u32,
    },
    /// cpu/memory/status/parent changed for an existing process.
    Changed(ProcInfo),
}

/// Diff `prev` into `next`.
///
/// A process is identified by `(pid, start_time)`. A PID that reappears with a
/// different start time is reported as an exit followed by a spawn, never as a
/// change.
pub fn diff(prev: &Snapshot, next: &Snapshot) -> Vec<ProcEvent> {
    let mut events = Vec::new();

    // Iterate in pid order so the event stream (and therefore a seeded tank)
    // is deterministic; HashMap iteration order is not.
    let mut prev_pids: Vec<u32> = prev.procs.keys().copied().collect();
    prev_pids.sort_unstable();
    let mut next_pids: Vec<u32> = next.procs.keys().copied().collect();
    next_pids.sort_unstable();

    // Exits, including PID reuse.
    for pid in &prev_pids {
        let old = &prev.procs[pid];
        match next.procs.get(pid) {
            None => events.push(ProcEvent::Exited { pid: *pid }),
            Some(new) if new.identity() != old.identity() => {
                events.push(ProcEvent::Exited { pid: *pid });
            }
            Some(_) => {}
        }
    }

    // Spawns and changes.
    for pid in &next_pids {
        let new = &next.procs[pid];
        match prev.procs.get(pid) {
            None => events.push(ProcEvent::Spawned(new.clone())),
            Some(old) if old.identity() != new.identity() => {
                events.push(ProcEvent::Spawned(new.clone()));
            }
            Some(old) => {
                if changed(old, new) {
                    events.push(ProcEvent::Changed(new.clone()));
                }
            }
        }
    }

    events
}

fn changed(old: &ProcInfo, new: &ProcInfo) -> bool {
    old.cpu != new.cpu
        || old.memory != new.memory
        || old.status != new.status
        || old.parent != new.parent
        || old.name != new.name
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::ProcStatus;
    use crate::source::fake::{proc, snapshot};

    #[test]
    fn spawn_is_reported_once() {
        let prev = snapshot(vec![]);
        let next = snapshot(vec![proc(1, "a")]);
        assert_eq!(diff(&prev, &next), vec![ProcEvent::Spawned(proc(1, "a"))]);
    }

    #[test]
    fn exit_is_reported_once() {
        let prev = snapshot(vec![proc(1, "a")]);
        let next = snapshot(vec![]);
        assert_eq!(diff(&prev, &next), vec![ProcEvent::Exited { pid: 1 }]);
    }

    #[test]
    fn unchanged_process_produces_nothing() {
        let prev = snapshot(vec![proc(1, "a")]);
        let next = prev.clone();
        assert!(diff(&prev, &next).is_empty());
    }

    #[test]
    fn cpu_change_is_a_change() {
        let prev = snapshot(vec![proc(1, "a").with_cpu(1.0)]);
        let next = snapshot(vec![proc(1, "a").with_cpu(9.0)]);
        assert_eq!(
            diff(&prev, &next),
            vec![ProcEvent::Changed(proc(1, "a").with_cpu(9.0))]
        );
    }

    #[test]
    fn parent_change_is_a_change() {
        let prev = snapshot(vec![proc(1, "a")]);
        let next = snapshot(vec![proc(1, "a").with_parent(42)]);
        assert_eq!(
            diff(&prev, &next),
            vec![ProcEvent::Changed(proc(1, "a").with_parent(42))]
        );
    }

    #[test]
    fn status_change_is_a_change() {
        let prev = snapshot(vec![proc(1, "a")]);
        let next = snapshot(vec![proc(1, "a").with_status(ProcStatus::Zombie)]);
        assert_eq!(
            diff(&prev, &next),
            vec![ProcEvent::Changed(
                proc(1, "a").with_status(ProcStatus::Zombie)
            )]
        );
    }

    #[test]
    fn pid_reuse_is_exit_then_spawn() {
        let prev = snapshot(vec![proc(1, "old").with_start_time(10)]);
        let next = snapshot(vec![proc(1, "new").with_start_time(20)]);
        let events = diff(&prev, &next);
        assert_eq!(
            events,
            vec![
                ProcEvent::Exited { pid: 1 },
                ProcEvent::Spawned(proc(1, "new").with_start_time(20)),
            ]
        );
    }
}
