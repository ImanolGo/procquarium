//! The tank owns every moving thing and knows nothing about I/O.

pub mod decor;
pub mod fish;
pub mod mapping;
pub mod sprites;

use std::collections::{HashMap, HashSet};

use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use crate::diff::ProcEvent;
use crate::source::ProcInfo;

use decor::Decor;
use fish::{CreatureKind, Facing, Fish, FishState};

/// How long an egg wobbles on the sand before it hatches.
const EGG_TIME: f32 = 1.0;

/// A process that has just appeared, waiting to hatch.
#[derive(Debug, Clone)]
pub struct Egg {
    pub pid: u32,
    pub info: ProcInfo,
    pub x: f32,
    pub timer: f32,
    pub phase: f32,
}

/// A pellet of food that sinks through the water.
#[derive(Debug, Clone)]
pub struct Food {
    pub x: f32,
    pub y: f32,
    pub vy: f32,
}

/// The most pellets we keep on screen at once.
const FOOD_LIMIT: usize = 40;

/// The aquarium: fish, eggs and decor.
pub struct Tank {
    pub fish: Vec<Fish>,
    pub eggs: Vec<Egg>,
    /// Pellets of food falling through the water.
    pub food: Vec<Food>,
    pub decor: Decor,
    pub width: u16,
    pub height: u16,
    pub max_fish: usize,
    pub time: f32,
    /// Count of fish hatched this run, for the status line.
    pub hatched: u64,
    /// Processes that ate since the last [`Tank::take_eaten`].
    eaten: Vec<ProcInfo>,
    /// Identities that left the tank (died or finished leaving) since the last
    /// [`Tank::take_departed`].
    departed: Vec<(u32, u64)>,
    rng: ChaCha8Rng,
}

impl Tank {
    pub fn new(width: u16, height: u16, max_fish: usize, seed: u64) -> Self {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let mut decor = Decor::new();
        decor.configure(width, height, &mut rng);
        Self {
            fish: Vec::new(),
            eggs: Vec::new(),
            food: Vec::new(),
            decor,
            width,
            height,
            max_fish,
            time: 0.0,
            hatched: 0,
            eaten: Vec::new(),
            departed: Vec::new(),
            rng,
        }
    }

    /// Handle a terminal resize: regenerate decor and re-clamp positions.
    pub fn resize(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
        self.decor.configure(width, height, &mut self.rng);
        let w = width.max(2) as f32;
        let top = 1.0_f32;
        let bottom = (height.max(3) as f32 - 2.0).max(top);
        for f in &mut self.fish {
            f.pos.0 = f.pos.0.clamp(0.5, (w - 1.5).max(0.5));
            f.pos.1 = f.pos.1.clamp(top, bottom);
        }
        for e in &mut self.eggs {
            e.x = e.x.clamp(2.0, (w - 2.0).max(2.0));
        }
    }

    /// True if this process is already represented by a living creature or an
    /// egg. Dying fish (floating corpses) don't count, so a reused PID gets a
    /// fresh creature while the old one fades.
    pub fn contains(&self, id: (u32, u64)) -> bool {
        self.fish
            .iter()
            .any(|f| f.state != FishState::Exiting && f.identity() == id)
            || self.eggs.iter().any(|e| e.info.identity() == id)
    }

    fn fish_mut(&mut self, id: (u32, u64)) -> Option<&mut Fish> {
        self.fish
            .iter_mut()
            .find(|f| f.state != FishState::Exiting && f.identity() == id)
    }

    fn living_fish_mut(&mut self, pid: u32) -> Option<&mut Fish> {
        self.fish
            .iter_mut()
            .find(|f| f.pid == pid && f.state != FishState::Exiting)
    }

    fn ensure_egg(&mut self, info: ProcInfo) {
        if self.contains(info.identity()) {
            return;
        }
        let max_x = (self.width.max(4) as f32 - 2.0).max(3.0);
        let x = self.rng.random_range(2.0..max_x);
        let phase = self.rng.random_range(0.0..std::f32::consts::TAU);
        self.eggs.push(Egg {
            pid: info.pid,
            info,
            x,
            timer: 0.0,
            phase,
        });
    }

    /// Add the creature for a newly selected process: fish hatch from eggs,
    /// crabs and jellyfish appear directly.
    fn ensure_creature(&mut self, info: ProcInfo) {
        if self.contains(info.identity()) {
            return;
        }
        match CreatureKind::for_info(&info) {
            CreatureKind::Fish => self.ensure_egg(info),
            kind => self.spawn_creature(info, kind),
        }
    }

    fn spawn_creature(&mut self, info: ProcInfo, kind: CreatureKind) {
        let w = self.width.max(4) as f32;
        let h = self.height.max(3) as f32;
        let x = self.rng.random_range(2.0..(w - 2.0).max(3.0));
        let (y, home_y) = match kind {
            CreatureKind::Crab => (h - 1.0, h - 1.0),
            CreatureKind::Jellyfish => {
                let home = self.rng.random_range(4.0..(h - 3.0).max(5.0));
                (home, home)
            }
            CreatureKind::Fish => (h - 2.0, h - 2.0),
        };
        let phase = self.rng.random_range(0.0..std::f32::consts::TAU);
        let facing = if self.rng.random::<bool>() {
            Facing::Right
        } else {
            Facing::Left
        };
        self.fish
            .push(Fish::new(info, (x, y), home_y, phase, facing, kind));
    }

    /// Add a fish directly in open water (first sample), optionally swimming in
    /// from a wall.
    fn spawn_fish_in_water(&mut self, info: ProcInfo, entering: bool) {
        let w = self.width.max(4) as f32;
        let h = self.height.max(3) as f32;
        let y = self.rng.random_range(2.0..(h - 2.0).max(3.0));
        let phase = self.rng.random_range(0.0..std::f32::consts::TAU);
        if entering {
            let from_left = self.rng.random::<bool>();
            let facing = if from_left {
                Facing::Right
            } else {
                Facing::Left
            };
            let x = if from_left { -6.0 } else { w + 5.0 };
            let mut f = Fish::new(info, (x, y), y, phase, facing, CreatureKind::Fish);
            f.begin_entering(if from_left { 1.0 } else { -1.0 });
            self.fish.push(f);
        } else {
            let x = self.rng.random_range(2.0..(w - 2.0).max(3.0));
            let facing = if self.rng.random::<bool>() {
                Facing::Right
            } else {
                Facing::Left
            };
            let mut f = Fish::new(info, (x, y), y, phase, facing, CreatureKind::Fish);
            f.appear = 1.0;
            self.fish.push(f);
        }
    }

    /// A process promoted into the top N: fish swim in, other kinds just appear.
    fn begin_promoted(&mut self, info: ProcInfo) {
        match CreatureKind::for_info(&info) {
            CreatureKind::Fish => self.spawn_fish_in_water(info, true),
            kind => self.spawn_creature(info, kind),
        }
    }

    /// A process that just appeared (after the first sample).
    fn begin_spawned(&mut self, info: ProcInfo) {
        self.ensure_creature(info);
    }

    /// The very first sample: everything appears in place, no eggs.
    fn begin_initial(&mut self, info: ProcInfo) {
        match CreatureKind::for_info(&info) {
            CreatureKind::Fish => self.spawn_fish_in_water(info, false),
            kind => self.spawn_creature(info, kind),
        }
    }

    /// Apply process events, then reconcile against the current selection.
    ///
    /// Reconciliation matters because a process can enter the top N without an
    /// event of its own (a bigger one exited, and its own numbers didn't
    /// change), so we can't rely on events alone to create creatures.
    pub fn apply(&mut self, events: &[ProcEvent], selected: &[ProcInfo], first_sample: bool) {
        let selected_ids: HashSet<(u32, u64)> = selected.iter().map(|p| p.identity()).collect();
        let spawned_ids: HashSet<(u32, u64)> = events
            .iter()
            .filter_map(|event| match event {
                ProcEvent::Spawned(info) => Some(info.identity()),
                _ => None,
            })
            .collect();

        for event in events {
            match event {
                ProcEvent::Spawned(_) => {
                    // Creation is handled in the reconcile pass below.
                }
                ProcEvent::Exited { pid } => {
                    self.eggs.retain(|e| e.info.pid != *pid);
                    if let Some(f) = self.living_fish_mut(*pid) {
                        let id = f.identity();
                        f.begin_exit();
                        self.departed.push(id);
                    }
                }
                ProcEvent::Changed(info) => {
                    if let Some(f) = self.fish_mut(info.identity()) {
                        f.apply_info(info.clone());
                    }
                }
            }
        }

        // Every selected process must have a living creature; a leaving one that
        // is selected again turns around instead of being replaced.
        for info in selected {
            let id = info.identity();
            match self
                .fish
                .iter()
                .find(|f| f.state != FishState::Exiting && f.identity() == id)
                .map(|f| f.state)
            {
                Some(FishState::Leaving) => {
                    if let Some(f) = self.fish_mut(id) {
                        f.state = FishState::Alive;
                        f.timer = 0.0;
                    }
                }
                Some(_) => {}
                None => {
                    if !self.contains(id) {
                        if first_sample {
                            self.begin_initial(info.clone());
                        } else if spawned_ids.contains(&id) {
                            self.begin_spawned(info.clone());
                        } else {
                            self.begin_promoted(info.clone());
                        }
                    }
                }
            }
        }

        // Eggs for processes that fell out of the selection are abandoned.
        self.eggs
            .retain(|e| selected_ids.contains(&e.info.identity()));

        // Anything selected no longer gets to stay: swim away.
        let w = self.width as f32;
        let mut leaving = Vec::new();
        for f in &mut self.fish {
            if matches!(f.state, FishState::Alive | FishState::Entering)
                && !selected_ids.contains(&f.identity())
            {
                let dir = if f.pos.0 < w / 2.0 { -1.0 } else { 1.0 };
                leaving.push(f.identity());
                f.begin_leaving(dir);
            }
        }
        self.departed.extend(leaving);
    }

    /// Creatures that are alive or leaving (i.e. not floating corpses). Eggs
    /// don't count.
    pub fn living_count(&self) -> usize {
        self.fish
            .iter()
            .filter(|f| f.state != FishState::Exiting)
            .count()
    }

    /// Identities currently occupying the tank (living creatures and eggs).
    /// Used to keep incumbents in place across samples.
    pub fn resident_identities(&self) -> HashSet<(u32, u64)> {
        self.fish
            .iter()
            .filter(|f| f.state != FishState::Exiting)
            .map(|f| f.identity())
            .chain(self.eggs.iter().map(|e| e.info.identity()))
            .collect()
    }

    /// Advance the whole tank by `dt` seconds.
    pub fn update(&mut self, dt: f32) {
        self.time += dt;
        let w = self.width.max(2) as f32;
        let h = self.height.max(3) as f32;

        // Hatch eggs.
        let mut hatched = Vec::new();
        self.eggs.retain_mut(|egg| {
            egg.timer += dt;
            egg.phase += dt * 7.0;
            if egg.timer >= EGG_TIME {
                hatched.push((egg.info.clone(), egg.x));
                false
            } else {
                true
            }
        });
        for (info, x) in hatched {
            let phase = self.rng.random_range(0.0..std::f32::consts::TAU);
            let home_y = self.rng.random_range(2.0..(h - 2.0).max(3.0));
            let facing = if self.rng.random::<bool>() {
                Facing::Right
            } else {
                Facing::Left
            };
            let mut f = Fish::new(
                info,
                (x, h - 2.0),
                home_y,
                phase,
                facing,
                CreatureKind::Fish,
            );
            f.vel.1 = -2.5;
            self.fish.push(f);
            self.hatched += 1;
        }

        // Drop and sink food.
        for pellet in &mut self.food {
            pellet.y += pellet.vy * dt;
        }
        self.food.retain(|pellet| pellet.y < h - 1.0);
        let pellets: Vec<(f32, f32)> = self.food.iter().map(|p| (p.x, p.y)).collect();

        // Snapshot parent positions before mutating the fish.
        let positions: HashMap<u32, (f32, f32)> = self
            .fish
            .iter()
            .filter(|f| f.state == FishState::Alive)
            .map(|f| (f.pid, f.pos))
            .collect();

        for f in &mut self.fish {
            let parent = f.info.parent.and_then(|p| positions.get(&p).copied());
            let nearest = if f.kind == CreatureKind::Fish {
                pellets
                    .iter()
                    .copied()
                    .filter(|(fx, fy)| {
                        let (dx, dy) = (fx - f.pos.0, fy - f.pos.1);
                        dx * dx + dy * dy < 400.0
                    })
                    .min_by(|a, b| {
                        let da = (a.0 - f.pos.0).powi(2) + (a.1 - f.pos.1).powi(2);
                        let db = (b.0 - f.pos.0).powi(2) + (b.1 - f.pos.1).powi(2);
                        da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                    })
            } else {
                None
            };
            f.update(dt, w, h, parent, nearest);
        }

        // Fish that reached a pellet eat it; remember who for the priority nudge.
        let mut eaten = Vec::new();
        for f in &mut self.fish {
            if f.kind != CreatureKind::Fish {
                continue;
            }
            if let Some(index) = self
                .food
                .iter()
                .position(|p| (p.x - f.pos.0).abs() < 2.2 && (f.pos.1 - p.y).abs() < 1.6)
            {
                f.fed = 2.5;
                self.eaten.push(f.info.clone());
                eaten.push(index);
            }
        }
        eaten.sort_unstable();
        eaten.dedup();
        for index in eaten.into_iter().rev() {
            self.food.remove(index);
        }
        self.food.truncate(FOOD_LIMIT);

        // Record departures (for priority restore) before dropping corpses.
        let finished: Vec<(u32, u64)> = self
            .fish
            .iter()
            .filter(|f| f.finished())
            .map(|f| f.identity())
            .collect();
        self.departed.extend(finished);
        self.fish.retain(|f| !f.finished());

        self.decor
            .update(dt, self.width, self.height, &mut self.rng, &self.fish);
    }

    /// Drop a pellet at `x`, or a random spot when `None`.
    pub fn drop_food(&mut self, x: Option<f32>) {
        if self.food.len() >= FOOD_LIMIT {
            return;
        }
        let w = self.width.max(4) as f32;
        let x = x.unwrap_or_else(|| self.rng.random_range(1.0..(w - 1.0).max(2.0)));
        self.food.push(Food {
            x: x.clamp(1.0, (w - 1.0).max(2.0)),
            y: 1.0,
            vy: 0.8,
        });
    }

    /// Processes that ate since the last call.
    pub fn take_eaten(&mut self) -> Vec<ProcInfo> {
        std::mem::take(&mut self.eaten)
    }

    /// Identities that left the tank since the last call.
    pub fn take_departed(&mut self) -> Vec<(u32, u64)> {
        std::mem::take(&mut self.departed)
    }

    /// Fish pids in a stable order (as they appear in the tank), for cycling.
    /// Dying fish are skipped so the selection can't land on a corpse.
    pub fn fish_pids(&self) -> Vec<u32> {
        self.fish
            .iter()
            .filter(|f| f.state != FishState::Exiting)
            .map(|f| f.pid)
            .collect()
    }

    /// The living creature for a pid, if any.
    pub fn fish(&self, pid: u32) -> Option<&Fish> {
        self.fish
            .iter()
            .find(|f| f.pid == pid && f.state != FishState::Exiting)
    }

    /// Whether a pid is represented by a living creature.
    pub fn has_living(&self, pid: u32) -> bool {
        self.fish(pid).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::ProcStatus;
    use crate::source::fake::proc;

    fn spawn(info: &ProcInfo) -> ProcEvent {
        ProcEvent::Spawned(info.clone())
    }

    #[test]
    fn spawn_event_produces_an_egg_then_a_fish() {
        let mut tank = Tank::new(80, 24, 60, 1);
        let shell = proc(1, "shell");
        tank.apply(&[spawn(&shell)], &[shell], false);
        assert_eq!(tank.eggs.len(), 1);
        assert!(tank.fish.is_empty());

        for _ in 0..50 {
            tank.update(0.01);
        }
        assert!(tank.fish.is_empty(), "still an egg");

        for _ in 0..100 {
            tank.update(0.01);
        }
        assert_eq!(tank.fish.len(), 1);
        assert_eq!(tank.fish[0].pid, 1);
    }

    #[test]
    fn exit_event_makes_the_fish_float_and_disappear() {
        let mut tank = Tank::new(80, 24, 60, 1);
        let shell = proc(1, "shell");
        tank.apply(&[spawn(&shell)], &[shell], false);
        for _ in 0..150 {
            tank.update(0.01);
        }
        assert_eq!(tank.fish.len(), 1);

        tank.apply(&[ProcEvent::Exited { pid: 1 }], &[], false);
        assert_eq!(tank.fish[0].state, FishState::Exiting);
        for _ in 0..500 {
            tank.update(0.05);
        }
        assert!(tank.fish.is_empty(), "corpse should have faded away");
    }

    #[test]
    fn dropping_out_of_top_n_swims_away_without_dying() {
        let mut tank = Tank::new(80, 24, 60, 1);
        let shell = proc(1, "shell");
        tank.apply(&[spawn(&shell)], &[shell], false);
        for _ in 0..150 {
            tank.update(0.01);
        }
        assert_eq!(tank.fish.len(), 1);

        tank.apply(&[], &[], false);
        assert_eq!(tank.fish[0].state, FishState::Leaving);
        for _ in 0..500 {
            tank.update(0.05);
        }
        assert!(tank.fish.is_empty());
    }

    #[test]
    fn unselected_spawn_gets_no_egg() {
        let mut tank = Tank::new(80, 24, 60, 1);
        tank.apply(&[spawn(&proc(1, "shell"))], &[], false);
        assert!(tank.eggs.is_empty());
        assert!(tank.fish.is_empty());
    }

    #[test]
    fn degenerate_terminal_sizes_do_not_panic() {
        // Some terminals briefly report zero columns at startup.
        for (w, h) in [(0, 0), (1, 1), (2, 3), (4, 4)] {
            let mut tank = Tank::new(w, h, 60, 1);
            let shell = proc(1, "shell");
            tank.apply(&[spawn(&shell)], &[shell], false);
            for _ in 0..200 {
                tank.update(0.05);
            }
        }
    }

    #[test]
    fn zombie_fish_stays_in_the_tank() {
        let mut tank = Tank::new(80, 24, 60, 1);
        let shell = proc(1, "shell");
        tank.apply(&[spawn(&shell)], std::slice::from_ref(&shell), false);
        for _ in 0..150 {
            tank.update(0.01);
        }
        tank.apply(
            &[ProcEvent::Changed(
                proc(1, "shell").with_status(ProcStatus::Zombie),
            )],
            &[shell],
            false,
        );
        for _ in 0..200 {
            tank.update(0.01);
        }
        assert_eq!(tank.fish.len(), 1);
        assert!(tank.fish[0].is_zombie());
    }

    #[test]
    fn container_processes_become_jellyfish() {
        let mut tank = Tank::new(80, 24, 60, 1);
        let nginx = proc(1, "nginx").container_process();
        tank.apply(&[spawn(&nginx)], &[nginx], false);
        assert!(tank.eggs.is_empty());
        assert_eq!(tank.fish.len(), 1);
        assert_eq!(tank.fish[0].kind, CreatureKind::Jellyfish);
    }

    #[test]
    fn kernel_threads_become_crabs_on_the_sand() {
        let mut tank = Tank::new(80, 24, 60, 1);
        let kworker = proc(1, "kworker").kernel_thread();
        tank.apply(&[spawn(&kworker)], &[kworker], false);
        assert_eq!(tank.fish.len(), 1);
        assert_eq!(tank.fish[0].kind, CreatureKind::Crab);
        for _ in 0..100 {
            tank.update(0.05);
        }
        assert_eq!(tank.fish[0].pos.1, 23.0, "crab sits on the sand");
    }

    #[test]
    fn a_fish_that_reaches_food_eats_it() {
        let mut tank = Tank::new(80, 24, 60, 1);
        let shell = proc(1, "shell");
        tank.apply(&[spawn(&shell)], &[shell], false);
        for _ in 0..150 {
            tank.update(0.01);
        }
        let x = tank.fish[0].pos.0;
        tank.drop_food(Some(x));
        assert_eq!(tank.food.len(), 1);

        let mut ate = false;
        for _ in 0..800 {
            tank.update(0.02);
            if tank.take_eaten().iter().any(|p| p.pid == 1) {
                ate = true;
                break;
            }
        }
        assert!(ate, "fish should reach and eat the pellet");
        assert!(tank.food.is_empty(), "the pellet should be gone");
    }

    #[test]
    fn promoted_process_gets_a_creature_without_its_own_event() {
        // max_fish = 1. The big process is selected first; when it exits the
        // small one is promoted even though its own numbers never changed, so
        // diff reports nothing for it.
        let mut tank = Tank::new(80, 24, 1, 1);
        let big = proc(1, "big").with_memory(900 * 1024 * 1024);
        let small = proc(2, "small").with_memory(10 * 1024 * 1024);
        tank.apply(&[spawn(&big), spawn(&small)], &[big], false);
        for _ in 0..150 {
            tank.update(0.01);
        }
        assert_eq!(tank.living_count(), 1);

        tank.apply(&[ProcEvent::Exited { pid: 1 }], &[small], false);
        for _ in 0..150 {
            tank.update(0.01);
        }
        assert_eq!(
            tank.living_count(),
            1,
            "the promoted process got a creature"
        );
        assert!(
            tank.fish
                .iter()
                .any(|f| f.pid == 2 && f.state != FishState::Exiting)
        );
    }

    #[test]
    fn pid_reuse_gets_a_new_creature_while_the_corpse_floats() {
        let mut tank = Tank::new(80, 24, 60, 1);
        let old = proc(1, "old").with_start_time(10);
        tank.apply(&[spawn(&old)], &[old], false);
        for _ in 0..150 {
            tank.update(0.01);
        }
        tank.apply(&[ProcEvent::Exited { pid: 1 }], &[], false);
        assert_eq!(tank.fish[0].state, FishState::Exiting);

        let new = proc(1, "new").with_start_time(20);
        tank.apply(&[spawn(&new)], &[new], false);
        assert!(
            tank.contains((1, 20)),
            "the reused PID must get its own creature"
        );
    }

    #[test]
    fn re_selected_leaving_fish_turns_around() {
        let mut tank = Tank::new(80, 24, 60, 1);
        let shell = proc(1, "shell");
        tank.apply(&[spawn(&shell)], std::slice::from_ref(&shell), false);
        for _ in 0..150 {
            tank.update(0.01);
        }
        tank.apply(&[], &[], false);
        assert_eq!(tank.fish[0].state, FishState::Leaving);

        tank.apply(&[], &[shell], false);
        assert_eq!(tank.fish[0].state, FishState::Alive);
        assert_eq!(
            tank.fish.len(),
            1,
            "it turned around rather than duplicating"
        );
    }

    #[test]
    fn reconcile_keeps_exactly_the_selected_creatures() {
        let mut tank = Tank::new(80, 24, 60, 1);
        let procs: Vec<ProcInfo> = (1..=5).map(|i| proc(i, "p")).collect();
        let events: Vec<ProcEvent> = procs.iter().map(spawn).collect();
        tank.apply(&events, &procs[..3], false);
        for _ in 0..150 {
            tank.update(0.01);
        }
        assert_eq!(tank.living_count(), 3);
    }

    #[test]
    fn first_sample_places_fish_in_the_water_without_eggs() {
        let mut tank = Tank::new(80, 24, 60, 1);
        let a = proc(1, "a");
        let b = proc(2, "b");
        tank.apply(&[spawn(&a), spawn(&b)], &[a, b], true);
        assert!(tank.eggs.is_empty(), "no eggs on the first sample");
        assert_eq!(tank.fish.len(), 2);
        assert!(tank.fish.iter().all(|f| f.state == FishState::Alive));
        assert!(tank.fish.iter().all(|f| f.pos.1 < 23.0), "placed at depth");
    }

    #[test]
    fn a_promoted_process_swims_in() {
        let mut tank = Tank::new(80, 24, 60, 1);
        let a = proc(1, "a");
        let b = proc(2, "b");
        tank.apply(&[spawn(&a), spawn(&b)], std::slice::from_ref(&a), true);
        for _ in 0..150 {
            tank.update(0.01);
        }
        // b enters the top N with no event of its own.
        tank.apply(&[], &[a, b], false);
        let promoted = tank.fish.iter().find(|f| f.pid == 2).expect("b has a fish");
        assert_eq!(promoted.state, FishState::Entering);
    }

    #[test]
    fn only_spawned_events_after_the_first_sample_lay_eggs() {
        let mut tank = Tank::new(80, 24, 60, 1);
        let a = proc(1, "a");
        tank.apply(&[spawn(&a)], std::slice::from_ref(&a), true);
        for _ in 0..150 {
            tank.update(0.01);
        }
        let b = proc(2, "b");
        tank.apply(&[spawn(&b)], std::slice::from_ref(&b), false);
        assert!(tank.eggs.iter().any(|e| e.pid == 2), "spawn lays an egg");
    }
}
