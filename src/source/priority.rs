//! Capped, reversible priority nudges for fed processes.
//!
//! The bookkeeping here is pure; the actual `getpriority`/`setpriority` calls
//! live behind [`PriorityBackend`], so the cap and the restore list can be
//! tested without touching the system.

use std::collections::HashMap;

use super::PriorityBoost;

/// How many niceness steps below the original a fed process may go.
const MAX_STEPS: i32 = 2;

/// The system calls behind [`BoostBook`].
pub trait PriorityBackend {
    /// Current niceness for `pid`, or `None` if it can't be read.
    fn get(&mut self, pid: u32) -> Option<i32>;
    /// Set niceness for `pid`; `true` on success.
    fn set(&mut self, pid: u32, nice: i32) -> bool;
}

#[derive(Debug, Clone, Copy)]
struct Entry {
    pid: u32,
    original: i32,
    current: i32,
}

/// Remembers the original niceness of every process we have fed, so boosts can
/// be capped and later undone.
#[derive(Debug, Default)]
pub struct BoostBook {
    entries: HashMap<(u32, u64), Entry>,
}

impl BoostBook {
    pub fn new() -> Self {
        Self::default()
    }

    /// Give a process one more step of priority, never more than [`MAX_STEPS`]
    /// below where it started. Recording the original happens on the first
    /// boost.
    pub fn boost(
        &mut self,
        id: (u32, u64),
        pid: u32,
        backend: &mut dyn PriorityBackend,
    ) -> PriorityBoost {
        if let std::collections::hash_map::Entry::Vacant(slot) = self.entries.entry(id) {
            match backend.get(pid) {
                Some(nice) => {
                    slot.insert(Entry {
                        pid,
                        original: nice,
                        current: nice,
                    });
                }
                None => return PriorityBoost::Unsupported,
            }
        }

        let Some(entry) = self.entries.get_mut(&id) else {
            return PriorityBoost::Unsupported;
        };
        if entry.current <= entry.original - MAX_STEPS {
            return PriorityBoost::Applied; // already as boosted as we allow
        }
        let target = entry.current - 1;
        if backend.set(pid, target) {
            entry.current = target;
            PriorityBoost::Applied
        } else {
            PriorityBoost::Denied
        }
    }

    /// Undo the boost for one process. Returns whether it was tracked.
    pub fn restore(&mut self, id: (u32, u64), backend: &mut dyn PriorityBackend) -> bool {
        match self.entries.remove(&id) {
            Some(entry) => {
                backend.set(entry.pid, entry.original);
                true
            }
            None => false,
        }
    }

    /// Undo every boost, returning how many were restored.
    pub fn restore_all(&mut self, backend: &mut dyn PriorityBackend) -> usize {
        let entries: Vec<Entry> = self.entries.drain().map(|(_, e)| e).collect();
        let count = entries.len();
        for entry in entries {
            backend.set(entry.pid, entry.original);
        }
        count
    }

    pub fn is_tracked(&self, id: (u32, u64)) -> bool {
        self.entries.contains_key(&id)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Fake {
        nice: HashMap<u32, i32>,
        sets: Vec<(u32, i32)>,
        fail: bool,
        unreadable: bool,
    }

    impl PriorityBackend for Fake {
        fn get(&mut self, pid: u32) -> Option<i32> {
            if self.unreadable {
                return None;
            }
            self.nice.get(&pid).copied()
        }
        fn set(&mut self, pid: u32, nice: i32) -> bool {
            if self.fail {
                return false;
            }
            self.sets.push((pid, nice));
            self.nice.insert(pid, nice);
            true
        }
    }

    fn fake(nice: i32) -> Fake {
        let mut f = Fake::default();
        f.nice.insert(7, nice);
        f
    }

    #[test]
    fn boosting_is_capped_at_two_steps() {
        let mut book = BoostBook::new();
        let mut backend = fake(5);
        for _ in 0..10 {
            let _ = book.boost((7, 1), 7, &mut backend);
        }
        assert_eq!(backend.sets, vec![(7, 4), (7, 3)], "only two steps, ever");
        assert_eq!(backend.nice[&7], 3);
    }

    #[test]
    fn restore_puts_the_original_back() {
        let mut book = BoostBook::new();
        let mut backend = fake(5);
        let _ = book.boost((7, 1), 7, &mut backend);
        let _ = book.boost((7, 1), 7, &mut backend);
        assert!(book.restore((7, 1), &mut backend));
        assert_eq!(backend.nice[&7], 5, "back to where it started");
        assert!(!book.is_tracked((7, 1)));
        assert!(!book.restore((7, 1), &mut backend), "already restored");
    }

    #[test]
    fn restore_all_undoes_everything() {
        let mut book = BoostBook::new();
        let mut backend = fake(5);
        backend.nice.insert(9, 2);
        let _ = book.boost((7, 1), 7, &mut backend);
        let _ = book.boost((9, 2), 9, &mut backend);
        assert_eq!(book.len(), 2);
        assert_eq!(book.restore_all(&mut backend), 2);
        assert_eq!(backend.nice[&7], 5);
        assert_eq!(backend.nice[&9], 2);
        assert!(book.is_empty());
    }

    #[test]
    fn unreadable_priority_is_unsupported() {
        let mut book = BoostBook::new();
        let mut backend = fake(5);
        backend.unreadable = true;
        assert_eq!(
            book.boost((7, 1), 7, &mut backend),
            PriorityBoost::Unsupported
        );
        assert!(!book.is_tracked((7, 1)));
    }

    #[test]
    fn denied_set_does_not_advance_the_cap() {
        let mut book = BoostBook::new();
        let mut backend = fake(5);
        backend.fail = true;
        assert_eq!(book.boost((7, 1), 7, &mut backend), PriorityBoost::Denied);
        // The failed attempt didn't consume a step, so a later success still
        // reaches the full two steps.
        backend.fail = false;
        let _ = book.boost((7, 1), 7, &mut backend);
        let _ = book.boost((7, 1), 7, &mut backend);
        assert_eq!(backend.sets, vec![(7, 4), (7, 3)]);
    }
}
