//! Application state: the tank plus everything the UI needs.

use std::cmp::Ordering;
use std::collections::{HashMap, VecDeque};

use crate::config::Config;
use crate::diff;
use crate::source::{KillOutcome, ProcInfo, Snapshot};
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
    /// Username of our own process, so we never feed another user's processes.
    self_user: Option<String>,
    /// Pids that ate since the last drain and are due a priority nudge.
    pending_boosts: Vec<(u32, u64)>,
    /// Processes that left and whose priority nudge should be undone.
    pending_restores: Vec<(u32, u64)>,
    /// How many consecutive samples each incumbent fish has been outranked, for
    /// the cut-off hysteresis.
    streaks: HashMap<(u32, u64), u8>,
    /// Recent CPU samples per creature, for the details sparkline.
    history: HashMap<(u32, u64), VecDeque<f32>>,
    /// The active search query, when `/` is open.
    search: Option<String>,
    /// Selection to restore if the search is cancelled.
    search_prev: Option<u32>,
    /// A pending "send SIGTERM to ...?" confirmation.
    confirm: Option<ProcInfo>,
    /// Identities `(pid, start_time)` to signal, drained by the main loop.
    pending_kills: Vec<(u32, u64)>,
    /// The name of each confirmed kill, for the status line when it finishes.
    kill_names: HashMap<(u32, u64), String>,
}

/// How many CPU samples to remember, and how many to draw.
const SPARK_LEN: usize = 60;
const SPARK_DISPLAY: usize = 24;

impl App {
    pub fn new(config: Config, width: u16, height: u16, seed: u64) -> Self {
        Self {
            tank: Tank::new(width, height, seed),
            config,
            paused: false,
            show_labels: true,
            selected: None,
            ready: false,
            status: Some("filling the tank…".to_string()),
            load: 0.0,
            prev: Snapshot::default(),
            self_user: None,
            pending_boosts: Vec::new(),
            pending_restores: Vec::new(),
            streaks: HashMap::new(),
            history: HashMap::new(),
            search: None,
            search_prev: None,
            confirm: None,
            pending_kills: Vec::new(),
            kill_names: HashMap::new(),
        }
    }

    /// Feed a fresh snapshot: diff it, pick the top processes, and update the tank.
    pub fn apply_snapshot(&mut self, snapshot: Snapshot) {
        let first_sample = !self.ready;
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

        self.tank.apply(&events, &selected, first_sample);
        self.record_cpu();
        self.load = snapshot.load;
        self.self_user = snapshot
            .procs
            .get(&std::process::id())
            .and_then(|p| p.user.clone());

        self.prev = snapshot;
        self.ready = true;
        self.status = None;

        // If the fish we were asked to kill has died or left the tank, close
        // the prompt: there is nothing left to confirm.
        if let Some(id) = self.confirm.as_ref().map(|c| c.identity()) {
            let present = self
                .prev
                .procs
                .get(&id.0)
                .is_some_and(|p| p.identity() == id);
            let resident = self.tank.fish(id.0).is_some_and(|f| f.identity() == id);
            if (!present || !resident)
                && let Some(confirm) = self.confirm.take()
            {
                self.status = Some(format!("{} is no longer running", confirm.name));
            }
        }

        if let Some(pid) = self.selected
            && !self.tank.has_living(pid)
        {
            self.selected = None;
        }
    }

    pub fn update(&mut self, dt: f32) {
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

    /// The active search query, when `/` is open.
    pub fn search_query(&self) -> Option<&str> {
        self.search.as_deref()
    }

    /// Ask to kill the selected fish: only with `--kill`, and only for a real
    /// selection. The confirmation prompt is shown until answered.
    pub fn start_kill(&mut self) {
        if !self.config.kill {
            return;
        }
        if let Some(fish) = self.selected_fish() {
            self.confirm = Some(fish.info.clone());
        }
    }

    /// The pending kill confirmation, if any.
    pub fn kill_prompt(&self) -> Option<&ProcInfo> {
        self.confirm.as_ref()
    }

    /// Confirm the kill: queue the signal for our own processes only. The whole
    /// identity is queued, so the signal is refused if the pid is reused before
    /// it is sent.
    pub fn confirm_kill(&mut self) {
        if let Some(info) = self.confirm.take()
            && owns_process(&info, self.self_user.as_deref())
        {
            self.kill_names.insert(info.identity(), info.name.clone());
            self.pending_kills.push(info.identity());
        }
    }

    /// Dismiss the kill confirmation.
    pub fn cancel_kill(&mut self) {
        self.confirm = None;
    }

    /// Identities to signal since the last call.
    pub fn take_pending_kills(&mut self) -> Vec<(u32, u64)> {
        std::mem::take(&mut self.pending_kills)
    }

    /// Report the outcome of a confirmed kill in the status line.
    pub fn finish_kill(&mut self, id: (u32, u64), outcome: KillOutcome) {
        let name = self
            .kill_names
            .remove(&id)
            .unwrap_or_else(|| format!("pid {}", id.0));
        self.status = Some(match outcome {
            KillOutcome::Signalled => format!("sent SIGTERM to {name}"),
            KillOutcome::AlreadyExited => format!("{name} had already exited"),
            KillOutcome::PidReused => format!("{name} had already exited (pid was reused)"),
            KillOutcome::Failed => format!("could not signal {name}"),
        });
    }

    /// Open the search line, remembering the selection to restore on cancel.
    pub fn start_search(&mut self) {
        self.search_prev = self.selected;
        self.search = Some(String::new());
    }

    /// Add a character to the query and jump to the first matching fish.
    pub fn search_push(&mut self, c: char) {
        if let Some(query) = &mut self.search {
            query.push(c);
        }
        self.select_first_match();
    }

    /// Delete the last character of the query.
    pub fn search_backspace(&mut self) {
        if let Some(query) = &mut self.search {
            query.pop();
        }
        self.select_first_match();
    }

    /// Keep the current selection and close the search.
    pub fn search_commit(&mut self) {
        self.search = None;
    }

    /// Close the search and put the selection back where it was.
    pub fn search_cancel(&mut self) {
        self.search = None;
        self.selected = self.search_prev;
    }

    fn select_first_match(&mut self) {
        let Some(query) = self.search.clone() else {
            return;
        };
        if query.is_empty() {
            return;
        }
        let needle = query.to_lowercase();
        self.selected = self.tank.fish_pids().into_iter().find(|pid| {
            self.tank
                .fish(*pid)
                .is_some_and(|f| f.info.name.to_lowercase().contains(&needle))
        });
    }

    /// Select the fish under a terminal cell, or clear the selection when the
    /// click lands on empty water.
    pub fn select_at(&mut self, column: u16, row: u16) {
        self.selected = crate::render::fish_at(self, column, row);
    }

    /// Remember each living creature's CPU for the details sparkline, and forget
    /// the ones that have gone.
    fn record_cpu(&mut self) {
        let mut live = std::collections::HashSet::new();
        for f in &self.tank.fish {
            let id = f.identity();
            let samples = self.history.entry(id).or_default();
            samples.push_back(f.info.cpu);
            while samples.len() > SPARK_LEN {
                samples.pop_front();
            }
            live.insert(id);
        }
        self.history.retain(|id, _| live.contains(id));
    }

    /// A one-row sparkline of the recent CPU samples for a creature.
    pub fn cpu_sparkline(&self, id: (u32, u64)) -> String {
        const GLYPHS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
        let Some(history) = self.history.get(&id) else {
            return String::new();
        };
        let start = history.len().saturating_sub(SPARK_DISPLAY);
        history
            .iter()
            .skip(start)
            .map(|cpu| {
                let level =
                    (cpu.clamp(0.0, 100.0) / 100.0 * (GLYPHS.len() - 1) as f32).round() as usize;
                GLYPHS[level.min(GLYPHS.len() - 1)]
            })
            .collect()
    }

    pub fn adjust_max_fish(&mut self, delta: isize) {
        let new = (self.config.max_fish as isize + delta).max(1) as usize;
        self.config.max_fish = new;
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
        Config::builder().build().expect("valid")
    }

    fn config_max(n: usize) -> Config {
        Config::builder().max_fish(n).build().expect("valid")
    }

    #[test]
    fn kill_is_opt_in_and_only_for_own_processes() {
        // Without --kill, k does nothing.
        let mut app = App::new(config(), 80, 24, 1);
        app.apply_snapshot(snapshot(vec![proc(1, "a").with_user("me")]));
        for _ in 0..150 {
            app.update(0.01);
        }
        app.self_user = Some("me".into());
        app.selected = Some(1);
        app.start_kill();
        assert!(app.kill_prompt().is_none(), "off without --kill");

        // With --kill and ownership, it queues after confirmation.
        let mut app = App::new(
            Config::builder().kill(true).build().expect("valid"),
            80,
            24,
            1,
        );
        app.apply_snapshot(snapshot(vec![proc(1, "a").with_user("me")]));
        for _ in 0..150 {
            app.update(0.01);
        }
        app.self_user = Some("me".into());
        app.selected = Some(1);
        app.start_kill();
        assert!(app.kill_prompt().is_some());
        app.confirm_kill();
        assert_eq!(app.take_pending_kills(), vec![(1, 1)]);

        // Someone else's process is refused.
        let mut app = App::new(
            Config::builder().kill(true).build().expect("valid"),
            80,
            24,
            1,
        );
        app.apply_snapshot(snapshot(vec![proc(1, "a").with_user("root")]));
        for _ in 0..150 {
            app.update(0.01);
        }
        app.self_user = Some("me".into());
        app.selected = Some(1);
        app.start_kill();
        app.confirm_kill();
        assert!(app.take_pending_kills().is_empty());
    }

    #[test]
    fn kill_prompt_closes_when_the_process_exits() {
        let mut app = App::new(
            Config::builder().kill(true).build().expect("valid"),
            80,
            24,
            1,
        );
        app.apply_snapshot(snapshot(vec![proc(1, "a").with_user("me")]));
        for _ in 0..150 {
            app.update(0.01);
        }
        app.self_user = Some("me".into());
        app.selected = Some(1);
        app.start_kill();
        assert!(app.kill_prompt().is_some());

        // The process is gone from the next snapshot: the prompt closes itself.
        app.apply_snapshot(snapshot(vec![proc(2, "other").with_user("me")]));
        assert!(
            app.kill_prompt().is_none(),
            "prompt closes when the fish dies"
        );
        assert_eq!(app.status.as_deref(), Some("a is no longer running"));
    }

    #[test]
    fn kill_outcomes_are_reported_in_the_status_line() {
        let mut app = App::new(config(), 80, 24, 1);
        let id = (42, 7);
        let cases = [
            (KillOutcome::Signalled, "sent SIGTERM to firefox"),
            (KillOutcome::AlreadyExited, "firefox had already exited"),
            (
                KillOutcome::PidReused,
                "firefox had already exited (pid was reused)",
            ),
            (KillOutcome::Failed, "could not signal firefox"),
        ];
        for (outcome, expected) in cases {
            app.kill_names.insert(id, "firefox".to_string());
            app.finish_kill(id, outcome);
            assert_eq!(app.status.as_deref(), Some(expected));
        }
    }

    #[test]
    fn search_selects_the_first_matching_fish() {
        let mut app = App::new(config(), 80, 24, 1);
        app.apply_snapshot(snapshot(vec![proc(1, "firefox"), proc(2, "cargo")]));
        for _ in 0..150 {
            app.update(0.01);
        }
        app.start_search();
        app.search_push('c');
        app.search_push('a');
        let name = app
            .selected
            .and_then(|pid| app.tank.fish(pid))
            .map(|f| f.info.name.clone());
        assert_eq!(name.as_deref(), Some("cargo"));

        app.search_commit();
        assert!(app.search_query().is_none());
        assert_eq!(app.selected, Some(2), "commit keeps the selection");
    }

    #[test]
    fn cancelling_search_restores_the_selection() {
        let mut app = App::new(config(), 80, 24, 1);
        app.apply_snapshot(snapshot(vec![proc(1, "firefox"), proc(2, "cargo")]));
        for _ in 0..150 {
            app.update(0.01);
        }
        app.selected = Some(1);
        app.start_search();
        app.search_push('c');
        assert_eq!(app.selected, Some(2), "moved to the match");
        app.search_cancel();
        assert_eq!(app.selected, Some(1), "cancel restores it");
    }

    #[test]
    fn cpu_sparkline_maps_history_to_glyphs() {
        let mut app = App::new(config(), 80, 24, 1);
        app.history
            .insert((9, 1), [0.0, 50.0, 100.0].into_iter().collect());
        assert_eq!(app.cpu_sparkline((9, 1)), "▁▅█");
        assert_eq!(app.cpu_sparkline((404, 1)), "");
    }

    #[test]
    fn clicking_a_fish_selects_it_and_empty_water_clears() {
        let mut app = App::new(config_max(5), 80, 24, 1);
        app.apply_snapshot(snapshot(vec![proc(1, "a")]));
        for _ in 0..150 {
            app.update(0.01);
        }
        app.tank.fish[0].pos = (40.0, 12.0);
        let pid = app.tank.fish[0].pid;
        app.select_at(40, 12);
        assert_eq!(app.selected, Some(pid));
        app.select_at(5, 5);
        assert_eq!(app.selected, None);
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
            .apply(&[ProcEvent::Spawned(shell.clone())], &[shell], false);
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
