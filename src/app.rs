//! Application state: the tank plus the previous snapshot used for diffing.

use std::cmp::Ordering;
use std::collections::HashSet;

use crate::config::Config;
use crate::diff;
use crate::source::{ProcInfo, Snapshot};
use crate::tank::Tank;

pub struct App {
    pub tank: Tank,
    pub config: Config,
    /// False until the first (CPU-meaningful) sample has arrived.
    pub ready: bool,
    pub status: Option<String>,
    prev: Snapshot,
}

impl App {
    pub fn new(config: Config, width: u16, height: u16, seed: u64) -> Self {
        let max_fish = config.max_fish;
        Self {
            tank: Tank::new(width, height, max_fish, seed),
            config,
            ready: false,
            status: Some("filling the tank…".to_string()),
            prev: Snapshot::default(),
        }
    }

    /// Feed a fresh snapshot: diff it, pick the top processes, update the tank.
    pub fn apply_snapshot(&mut self, snapshot: Snapshot) {
        let events = diff::diff(&self.prev, &snapshot);
        let selected: HashSet<u32> = select_pids(&snapshot, &self.config).into_iter().collect();
        self.tank.apply(&events, &selected);
        self.prev = snapshot;
        self.ready = true;
        self.status = None;
    }

    pub fn update(&mut self, dt: f32) {
        self.tank.update(dt);
    }
}

/// Choose which processes deserve a fish: apply the filters, rank by score,
/// keep the top `max_fish`. Pure, so it is easy to test.
pub fn select_pids(snapshot: &Snapshot, config: &Config) -> Vec<u32> {
    let mut candidates: Vec<&ProcInfo> = snapshot
        .procs
        .values()
        .filter(|p| config.kernel || !p.kernel)
        .filter(|p| {
            config
                .user
                .as_ref()
                .is_none_or(|user| p.user.as_deref() == Some(user.as_str()))
        })
        .filter(|p| config.filter.as_ref().is_none_or(|re| re.is_match(&p.name)))
        .collect();

    candidates.sort_by(|a, b| {
        b.score()
            .partial_cmp(&a.score())
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.pid.cmp(&b.pid))
    });
    candidates.truncate(config.max_fish);
    candidates.into_iter().map(|p| p.pid).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::fake::{proc, snapshot};

    fn config() -> Config {
        Config::new(1.0, 60, None, None, false, false, None).expect("valid")
    }

    #[test]
    fn kernel_threads_are_excluded_by_default_and_included_with_flag() {
        let snap = snapshot(vec![
            proc(1, "normal").with_memory(1024),
            proc(2, "kworker").with_memory(99999999).kernel_thread(),
        ]);
        assert_eq!(select_pids(&snap, &config()), vec![1]);

        let mut with_kernel = config();
        with_kernel.kernel = true;
        assert_eq!(select_pids(&snap, &with_kernel).len(), 2);
    }

    #[test]
    fn user_filter_applies() {
        let snap = snapshot(vec![
            proc(1, "mine").with_user("imanolgo"),
            proc(2, "root-thing").with_user("root"),
        ]);
        let mut c = config();
        c.user = Some("imanolgo".to_string());
        assert_eq!(select_pids(&snap, &c), vec![1]);
    }

    #[test]
    fn regex_filter_applies_to_the_name() {
        let snap = snapshot(vec![proc(1, "firefox"), proc(2, "cargo")]);
        let mut c = config();
        c.filter = Some(regex::Regex::new("^fire").expect("valid"));
        assert_eq!(select_pids(&snap, &c), vec![1]);
    }

    #[test]
    fn ranking_prefers_high_score_and_truncates() {
        let snap = snapshot(vec![
            proc(1, "big").with_memory(4 * 1024 * 1024 * 1024),
            proc(2, "hot").with_cpu(100.0),
            proc(3, "tiny"),
        ]);
        let mut c = config();
        c.max_fish = 2;
        let picked = select_pids(&snap, &c);
        assert_eq!(picked.len(), 2);
        assert!(picked.contains(&1));
        assert!(picked.contains(&2));
    }

    #[test]
    fn ranking_is_deterministic_on_ties() {
        let snap = snapshot(vec![proc(5, "a"), proc(3, "b"), proc(9, "c")]);
        assert_eq!(select_pids(&snap, &config()), vec![3, 5, 9]);
    }
}
