//! Application state: the tank plus everything the UI needs.

use std::cmp::Ordering;

use crate::config::Config;
use crate::diff;
use crate::source::{ProcInfo, Snapshot};
use crate::tank::Tank;

/// Top-level state. The tank is pure simulation; the rest is UI state and the
/// previous snapshot used for diffing.
pub struct App {
    pub tank: Tank,
    pub config: Config,
    pub paused: bool,
    pub show_labels: bool,
    pub selected: Option<u32>,
    /// False until the second (CPU-meaningful) sample has arrived.
    pub ready: bool,
    pub status: Option<String>,
    /// Overall system load in `0.0..=1.0` from the latest sample; drives the
    /// day/night cycle.
    pub load: f32,
    prev: Snapshot,
    pub tick: f32,
    /// Username of our own process, so we never feed another user's processes.
    self_user: Option<String>,
    /// Pids that ate since the last drain and are due a priority nudge.
    pending_boosts: Vec<(u32, u64)>,
    /// Processes that left and whose priority nudge should be undone.
    pending_restores: Vec<(u32, u64)>,
}

impl App {
    pub fn new(config: Config, width: u16, height: u16, seed: u64) -> Self {
        let max_fish = config.max_fish;
        Self {
            tank: Tank::new(width, height, max_fish, seed),
            config,
            paused: false,
            show_labels: true,
            selected: None,
            ready: false,
            status: Some("filling the tank…".to_string()),
            load: 0.0,
            prev: Snapshot::default(),
            tick: 0.0,
            self_user: None,
            pending_boosts: Vec::new(),
            pending_restores: Vec::new(),
        }
    }

    /// Feed a fresh snapshot: diff it, pick the top processes, and update the tank.
    pub fn apply_snapshot(&mut self, snapshot: Snapshot) {
        let events = diff::diff(&self.prev, &snapshot);
        let selected = select_procs(&snapshot, &self.config);
        self.tank.apply(&events, &selected);
        self.load = snapshot.load;
        self.self_user = snapshot
            .procs
            .get(&std::process::id())
            .and_then(|p| p.user.clone());
        self.prev = snapshot;
        self.ready = true;
        self.status = None;

        if let Some(pid) = self.selected
            && !self.tank.has_living(pid)
        {
            self.selected = None;
        }
    }

    pub fn update(&mut self, dt: f32) {
        self.tick += dt;
        if !self.paused {
            self.tank.update(dt);
        }

        // Anything that ate and is ours becomes a pending priority nudge, and
        // anything that left the tank should have its nudge undone.
        for info in self.tank.take_eaten() {
            if self.config.feed && owns_process(&info, self.self_user.as_deref()) {
                self.pending_boosts.push(info.identity());
            }
        }
        self.pending_restores.extend(self.tank.take_departed());
    }

    /// Drop a pellet of food: above the selected fish if there is one, otherwise
    /// at a random spot. Only does anything when `--feed` is enabled.
    pub fn drop_food(&mut self) {
        if !self.config.feed {
            return;
        }
        let x = self
            .selected
            .and_then(|pid| self.tank.fish(pid))
            .map(|f| f.pos.0);
        self.tank.drop_food(x);
    }

    /// Take the processes that should be reniced since the last call.
    pub fn take_pending_boosts(&mut self) -> Vec<(u32, u64)> {
        std::mem::take(&mut self.pending_boosts)
    }

    /// Take the processes whose priority nudge should be undone.
    pub fn take_pending_restores(&mut self) -> Vec<(u32, u64)> {
        std::mem::take(&mut self.pending_restores)
    }

    pub fn resize(&mut self, width: u16, height: u16) {
        self.tank.resize(width, height);
    }

    /// Move the selection forward or backward through the current fish.
    pub fn select_next(&mut self, backwards: bool) {
        let pids = self.tank.fish_pids();
        if pids.is_empty() {
            self.selected = None;
            return;
        }
        let current = self
            .selected
            .and_then(|p| pids.iter().position(|x| *x == p));
        let next = match current {
            Some(i) if backwards => (i + pids.len() - 1) % pids.len(),
            Some(i) => (i + 1) % pids.len(),
            None if backwards => pids.len() - 1,
            None => 0,
        };
        self.selected = Some(pids[next]);
    }

    pub fn adjust_max_fish(&mut self, delta: isize) {
        let new = (self.config.max_fish as isize + delta).max(1) as usize;
        self.config.max_fish = new;
        self.tank.max_fish = new;
    }

    pub fn selected_fish(&self) -> Option<&crate::tank::fish::Fish> {
        self.selected.and_then(|pid| self.tank.fish(pid))
    }
}

/// How many kernel threads show up as crabs when `--kernel` is on. They score
/// near zero, so without a reserved slot they would never win a fish.
const CRAB_LIMIT: usize = 6;

/// Choose which processes appear: apply the filters, rank ordinary processes by
/// score and keep the top `max_fish`. With `--kernel`, a handful of kernel
/// threads are added as crabs beyond the fish budget. Pure, so easy to test.
pub fn select_procs(snapshot: &Snapshot, config: &Config) -> Vec<ProcInfo> {
    let passes = |p: &ProcInfo| -> bool {
        config
            .user
            .as_ref()
            .is_none_or(|user| p.user.as_deref() == Some(user.as_str()))
            && config.filter.as_ref().is_none_or(|re| re.is_match(&p.name))
    };

    let mut fish: Vec<&ProcInfo> = snapshot
        .procs
        .values()
        .filter(|p| !p.kernel && passes(p))
        .collect();
    fish.sort_by(|a, b| {
        b.score()
            .partial_cmp(&a.score())
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.pid.cmp(&b.pid))
    });
    fish.truncate(config.max_fish);

    let mut selected: Vec<ProcInfo> = fish.into_iter().cloned().collect();

    if config.kernel {
        let mut crabs: Vec<&ProcInfo> = snapshot
            .procs
            .values()
            .filter(|p| p.kernel && passes(p))
            .collect();
        crabs.sort_by(|a, b| {
            b.score()
                .partial_cmp(&a.score())
                .unwrap_or(Ordering::Equal)
                .then_with(|| a.pid.cmp(&b.pid))
        });
        selected.extend(crabs.into_iter().take(CRAB_LIMIT).cloned());
    }

    selected
}

/// The selected processes' pids, in ranking order.
pub fn select_pids(snapshot: &Snapshot, config: &Config) -> Vec<u32> {
    select_procs(snapshot, config)
        .into_iter()
        .map(|p| p.pid)
        .collect()
}

/// Whether we are allowed to touch this process's priority: only our own.
pub fn owns_process(info: &ProcInfo, self_user: Option<&str>) -> bool {
    match self_user {
        Some(me) => info.user.as_deref() == Some(me),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::fake::{proc, snapshot};

    fn config() -> Config {
        Config::new(1.0, 60, None, None, false, false, false, false, None, false).expect("valid")
    }

    #[test]
    fn kernel_threads_are_excluded_by_default_and_included_with_flag() {
        let snap = snapshot(vec![
            proc(1, "normal").with_memory(1024),
            proc(2, "kworker").with_memory(99999999).kernel_thread(),
        ]);
        let default = select_pids(&snap, &config());
        assert_eq!(default, vec![1]);

        let mut with_kernel = config();
        with_kernel.kernel = true;
        let both = select_pids(&snap, &with_kernel);
        assert_eq!(both.len(), 2);
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
    fn kernel_threads_get_reserved_crab_slots() {
        let mut procs = vec![proc(1, "hot").with_cpu(50.0)];
        for pid in 10..20 {
            procs.push(proc(pid, "worker"));
        }
        procs.push(proc(99, "kworker").kernel_thread());
        let mut c = config();
        c.max_fish = 3;
        c.kernel = true;
        let picked = select_pids(&snapshot(procs), &c);
        assert_eq!(picked.len(), 4, "3 fish plus one crab");
        assert!(picked.contains(&99));
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
        let picked = select_pids(&snap, &config());
        assert_eq!(picked, vec![3, 5, 9], "ties break on ascending pid");
    }

    #[test]
    fn dropping_pids_makes_fish_leave() {
        use crate::diff::ProcEvent;
        let mut app = App::new(config(), 80, 24, 1);
        let shell = proc(1, "shell");
        app.tank
            .apply(&[ProcEvent::Spawned(shell.clone())], &[shell]);
        for _ in 0..150 {
            app.update(0.01);
        }
        assert_eq!(app.tank.fish.len(), 1);

        // A new snapshot without pid 1: the fish should start leaving.
        app.apply_snapshot(snapshot(vec![proc(2, "other")]));
        assert_eq!(
            app.tank.fish[0].state,
            crate::tank::fish::FishState::Leaving
        );
    }

    #[test]
    fn scripted_lifecycle_eggs_hatches_then_floats() {
        use crate::source::ProcessSource;
        use crate::source::fake::FakeSource;

        let mut source = FakeSource::from_script(vec![vec![], vec![proc(1, "sleep")], vec![]]);
        let mut app = App::new(config(), 80, 24, 3);

        app.apply_snapshot(source.snapshot().expect("sample"));
        assert!(app.ready);
        app.apply_snapshot(source.snapshot().expect("sample"));
        assert_eq!(app.tank.eggs.len(), 1);

        for _ in 0..150 {
            app.update(0.01);
        }
        assert_eq!(app.tank.fish.len(), 1);

        app.apply_snapshot(source.snapshot().expect("sample"));
        assert_eq!(
            app.tank.fish[0].state,
            crate::tank::fish::FishState::Exiting
        );
    }

    #[test]
    fn only_own_processes_can_be_fed() {
        let mine = proc(1, "a").with_user("imanolgo");
        let theirs = proc(2, "b").with_user("root");
        assert!(owns_process(&mine, Some("imanolgo")));
        assert!(!owns_process(&theirs, Some("imanolgo")));
        assert!(!owns_process(&mine, None));
    }

    #[test]
    fn food_is_ignored_unless_feeding_is_enabled() {
        let mut app = App::new(config(), 80, 24, 1);
        app.drop_food();
        assert!(app.tank.food.is_empty());

        app.config.feed = true;
        app.drop_food();
        assert_eq!(app.tank.food.len(), 1);
    }
}
