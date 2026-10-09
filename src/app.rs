//! Application state: the tank plus everything the UI needs.

use std::cmp::Ordering;
use std::collections::HashMap;

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
    /// How many consecutive samples each incumbent fish has been outranked, for
    /// the cut-off hysteresis.
    streaks: HashMap<(u32, u64), u8>,
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
            streaks: HashMap::new(),
        }
    }

    /// Feed a fresh snapshot: diff it, pick the top processes, and update the tank.
    pub fn apply_snapshot(&mut self, snapshot: Snapshot) {
        let events = diff::diff(&self.prev, &snapshot);

        // Rank, then apply cut-off hysteresis so processes near the boundary
        // don't leave and re-hatch every sample.
        let ranked = rank_procs(&snapshot, &self.config, false);
        let incumbents = self.tank.resident_identities();
        let (mut selected, streaks) = select_with_hysteresis(
            &ranked,
            &incumbents,
            &self.streaks,
            self.config.max_fish,
            HYSTERESIS_MARGIN,
            HYSTERESIS_STREAK,
        );
        self.streaks = streaks;
        if self.config.kernel {
            selected.extend(
                rank_procs(&snapshot, &self.config, true)
                    .into_iter()
                    .take(CRAB_LIMIT),
            );
        }

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

/// How much better a newcomer must be than an incumbent to take its slot, and
/// how many samples an outranked incumbent keeps its place regardless.
const HYSTERESIS_MARGIN: f32 = 0.2;
const HYSTERESIS_STREAK: u8 = 3;

/// The filtered, ranked candidates of one kind, best first. Pure.
pub fn rank_procs(snapshot: &Snapshot, config: &Config, kernel: bool) -> Vec<ProcInfo> {
    let passes = |p: &ProcInfo| -> bool {
        p.kernel == kernel
            && config
                .user
                .as_ref()
                .is_none_or(|user| p.user.as_deref() == Some(user.as_str()))
            && config.filter.as_ref().is_none_or(|re| re.is_match(&p.name))
    };

    let mut ranked: Vec<&ProcInfo> = snapshot.procs.values().filter(|p| passes(p)).collect();
    ranked.sort_by(|a, b| {
        b.score()
            .partial_cmp(&a.score())
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.pid.cmp(&b.pid))
    });
    ranked.into_iter().cloned().collect()
}

/// Choose which processes appear: rank ordinary processes by score and keep the
/// top `max_fish`. With `--kernel`, a handful of kernel threads are added as
/// crabs beyond the fish budget. No hysteresis; see
/// [`select_with_hysteresis`] for that. Pure, so easy to test.
pub fn select_procs(snapshot: &Snapshot, config: &Config) -> Vec<ProcInfo> {
    let mut selected = rank_procs(snapshot, config, false);
    selected.truncate(config.max_fish);
    if config.kernel {
        selected.extend(
            rank_procs(snapshot, config, true)
                .into_iter()
                .take(CRAB_LIMIT),
        );
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

/// Pick `max_fish` from `ranked` (best first), keeping incumbents in place
/// unless a newcomer beats the weakest of them by `margin`, or that incumbent
/// has been outranked for `max_streak` samples in a row. Returns the selection
/// and the updated streak bookkeeping. Pure.
pub fn select_with_hysteresis(
    ranked: &[ProcInfo],
    incumbents: &std::collections::HashSet<(u32, u64)>,
    streaks: &std::collections::HashMap<(u32, u64), u8>,
    max_fish: usize,
    margin: f32,
    max_streak: u8,
) -> (Vec<ProcInfo>, std::collections::HashMap<(u32, u64), u8>) {
    use std::collections::HashMap;

    // Update streaks: an incumbent is "outranked" when a non-incumbent scores
    // higher than it does this sample.
    let best_newcomer = ranked
        .iter()
        .find(|p| !incumbents.contains(&p.identity()))
        .map(|p| p.score())
        .unwrap_or(f32::NEG_INFINITY);
    let mut new_streaks: HashMap<(u32, u64), u8> = HashMap::new();
    for p in ranked {
        if incumbents.contains(&p.identity()) {
            let previous = streaks.get(&p.identity()).copied().unwrap_or(0);
            let streak = if p.score() < best_newcomer {
                previous.saturating_add(1)
            } else {
                0
            };
            new_streaks.insert(p.identity(), streak);
        }
    }

    // Keep incumbents first, then fill any free slots with the best newcomers.
    let mut selected: Vec<&ProcInfo> = ranked
        .iter()
        .filter(|p| incumbents.contains(&p.identity()))
        .take(max_fish)
        .collect();
    for p in ranked {
        if selected.len() >= max_fish {
            break;
        }
        if !incumbents.contains(&p.identity()) {
            selected.push(p);
        }
    }

    // A newcomer may evict the weakest incumbent only with a clear margin, or
    // if that incumbent has been outranked for long enough.
    if selected.len() >= max_fish {
        for newcomer in ranked
            .iter()
            .filter(|p| !incumbents.contains(&p.identity()))
        {
            let weakest = selected
                .iter()
                .enumerate()
                .filter(|(_, s)| incumbents.contains(&s.identity()))
                .min_by(|a, b| {
                    a.1.score()
                        .partial_cmp(&b.1.score())
                        .unwrap_or(Ordering::Equal)
                })
                .map(|(i, _)| i);
            let Some(index) = weakest else { break };
            let incumbent = selected[index];
            let streak = new_streaks.get(&incumbent.identity()).copied().unwrap_or(0);
            if newcomer.score() > incumbent.score() * (1.0 + margin) || streak >= max_streak {
                selected[index] = newcomer;
            } else {
                break;
            }
        }
    }

    (selected.into_iter().cloned().collect(), new_streaks)
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

    fn config_max(n: usize) -> Config {
        Config::new(1.0, n, None, None, false, false, false, false, None, false).expect("valid")
    }

    #[test]
    fn no_churn_when_two_processes_oscillate_at_the_cutoff() {
        // max_fish = 1 and two processes trade places just above/below each
        // other. Without hysteresis they'd leave and re-hatch every sample.
        let mut app = App::new(config_max(1), 80, 24, 1);
        let mut resident: Option<u32> = None;
        let mut changes = 0;
        for i in 0..12 {
            let (a, b) = if i % 2 == 0 {
                (50.0, 51.0)
            } else {
                (51.0, 50.0)
            };
            app.apply_snapshot(snapshot(vec![
                proc(1, "a").with_cpu(a),
                proc(2, "b").with_cpu(b),
            ]));
            for _ in 0..20 {
                app.update(0.01);
            }
            let now = app
                .tank
                .fish
                .iter()
                .find(|f| f.state != crate::tank::fish::FishState::Exiting)
                .map(|f| f.pid);
            if let (Some(prev), Some(cur)) = (resident, now)
                && prev != cur
            {
                changes += 1;
            }
            if now.is_some() {
                resident = now;
            }
        }
        assert_eq!(changes, 0, "the incumbent should keep its slot");
        assert_eq!(app.tank.living_count(), 1);
    }

    #[test]
    fn a_much_smaller_incumbent_is_replaced_by_a_much_bigger_newcomer() {
        let mut app = App::new(config_max(1), 80, 24, 1);
        app.apply_snapshot(snapshot(vec![proc(1, "small").with_cpu(10.0)]));
        for _ in 0..20 {
            app.update(0.01);
        }
        // A process far bigger than the incumbent should take the slot.
        app.apply_snapshot(snapshot(vec![
            proc(1, "small").with_cpu(10.0),
            proc(2, "big").with_cpu(200.0),
        ]));
        for _ in 0..150 {
            app.update(0.01);
        }
        assert!(app.tank.fish.iter().any(|f| f.pid == 2 && !f.finished()));
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
