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
use fish::{Facing, Fish, FishState};

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

/// The aquarium: fish, eggs and decor.
pub struct Tank {
    pub fish: Vec<Fish>,
    pub eggs: Vec<Egg>,
    pub decor: Decor,
    pub width: u16,
    pub height: u16,
    pub max_fish: usize,
    pub time: f32,
    /// Count of fish hatched this run, for the status line.
    pub hatched: u64,
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
            decor,
            width,
            height,
            max_fish,
            time: 0.0,
            hatched: 0,
            rng,
        }
    }

    /// True if this pid is already represented by a fish or egg.
    pub fn contains(&self, pid: u32) -> bool {
        self.fish.iter().any(|f| f.pid == pid) || self.eggs.iter().any(|e| e.pid == pid)
    }

    fn ensure_egg(&mut self, info: ProcInfo) {
        if self.contains(info.pid) {
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

    /// Apply process events, then make any fish that is no longer in the
    /// selected set swim away.
    pub fn apply(&mut self, events: &[ProcEvent], selected: &HashSet<u32>) {
        for event in events {
            match event {
                ProcEvent::Spawned(info) => {
                    if selected.contains(&info.pid) {
                        self.ensure_egg(info.clone());
                    }
                }
                ProcEvent::Exited { pid } => {
                    self.eggs.retain(|e| e.pid != *pid);
                    if let Some(f) = self.fish.iter_mut().find(|f| f.pid == *pid) {
                        f.begin_exit();
                    }
                }
                ProcEvent::Changed(info) => {
                    if let Some(f) = self.fish.iter_mut().find(|f| f.pid == info.pid) {
                        f.apply_info(info.clone());
                    } else if selected.contains(&info.pid) {
                        self.ensure_egg(info.clone());
                    }
                }
            }
        }

        let w = self.width as f32;
        for f in &mut self.fish {
            if f.state == FishState::Alive && !selected.contains(&f.pid) {
                let dir = if f.pos.0 < w / 2.0 { -1.0 } else { 1.0 };
                f.begin_leaving(dir);
            }
        }
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
            let facing = if self.rng.random::<bool>() {
                Facing::Right
            } else {
                Facing::Left
            };
            let mut f = Fish::new(info, (x, h - 2.0), phase, facing);
            f.vel.1 = -2.5;
            self.fish.push(f);
            self.hatched += 1;
        }

        // Snapshot parent positions before mutating the fish.
        let positions: HashMap<u32, (f32, f32)> = self
            .fish
            .iter()
            .filter(|f| f.state == FishState::Alive)
            .map(|f| (f.pid, f.pos))
            .collect();

        for f in &mut self.fish {
            let parent = f.info.parent.and_then(|p| positions.get(&p).copied());
            f.update(dt, w, h, parent);
        }
        self.fish.retain(|f| !f.finished());

        self.decor
            .update(dt, self.width, self.height, &mut self.rng, &self.fish);
    }

    /// Fish pids in a stable order (as they appear in the tank), for cycling.
    pub fn fish_pids(&self) -> Vec<u32> {
        self.fish.iter().map(|f| f.pid).collect()
    }

    pub fn fish(&self, pid: u32) -> Option<&Fish> {
        self.fish.iter().find(|f| f.pid == pid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::ProcStatus;
    use crate::source::fake::proc;

    fn selected(pids: &[u32]) -> HashSet<u32> {
        pids.iter().copied().collect()
    }

    #[test]
    fn spawn_event_produces_an_egg_then_a_fish() {
        let mut tank = Tank::new(80, 24, 60, 1);
        let sel = selected(&[1]);
        tank.apply(&[ProcEvent::Spawned(proc(1, "shell"))], &sel);
        assert_eq!(tank.eggs.len(), 1);
        assert!(tank.fish.is_empty());

        // Half a second: still an egg.
        for _ in 0..50 {
            tank.update(0.01);
        }
        assert!(tank.fish.is_empty());

        // Another second: hatched.
        for _ in 0..100 {
            tank.update(0.01);
        }
        assert_eq!(tank.fish.len(), 1);
        assert_eq!(tank.fish[0].pid, 1);
    }

    #[test]
    fn exit_event_makes_the_fish_float_and_disappear() {
        let mut tank = Tank::new(80, 24, 60, 1);
        let sel = selected(&[1]);
        tank.apply(&[ProcEvent::Spawned(proc(1, "shell"))], &sel);
        for _ in 0..150 {
            tank.update(0.01);
        }
        assert_eq!(tank.fish.len(), 1);

        tank.apply(&[ProcEvent::Exited { pid: 1 }], &selected(&[]));
        assert_eq!(tank.fish[0].state, FishState::Exiting);
        for _ in 0..500 {
            tank.update(0.05);
        }
        assert!(tank.fish.is_empty(), "corpse should have faded away");
    }

    #[test]
    fn dropping_out_of_top_n_swims_away_without_dying() {
        let mut tank = Tank::new(80, 24, 60, 1);
        let sel = selected(&[1]);
        tank.apply(&[ProcEvent::Spawned(proc(1, "shell"))], &sel);
        for _ in 0..150 {
            tank.update(0.01);
        }
        assert_eq!(tank.fish.len(), 1);

        tank.apply(&[], &selected(&[]));
        assert_eq!(tank.fish[0].state, FishState::Leaving);
        for _ in 0..500 {
            tank.update(0.05);
        }
        assert!(tank.fish.is_empty());
    }

    #[test]
    fn unselected_spawn_gets_no_egg() {
        let mut tank = Tank::new(80, 24, 60, 1);
        tank.apply(&[ProcEvent::Spawned(proc(1, "shell"))], &selected(&[]));
        assert!(tank.eggs.is_empty());
        assert!(tank.fish.is_empty());
    }

    #[test]
    fn degenerate_terminal_sizes_do_not_panic() {
        // Some terminals briefly report zero columns at startup.
        for (w, h) in [(0, 0), (1, 1), (2, 3), (4, 4)] {
            let mut tank = Tank::new(w, h, 60, 1);
            tank.apply(&[ProcEvent::Spawned(proc(1, "shell"))], &selected(&[1]));
            for _ in 0..200 {
                tank.update(0.05);
            }
        }
    }

    #[test]
    fn zombie_fish_stays_in_the_tank() {
        let mut tank = Tank::new(80, 24, 60, 1);
        let sel = selected(&[1]);
        tank.apply(&[ProcEvent::Spawned(proc(1, "shell"))], &sel);
        for _ in 0..150 {
            tank.update(0.01);
        }
        tank.apply(
            &[ProcEvent::Changed(
                proc(1, "shell").with_status(ProcStatus::Zombie),
            )],
            &sel,
        );
        for _ in 0..200 {
            tank.update(0.01);
        }
        assert_eq!(tank.fish.len(), 1);
        assert!(tank.fish[0].is_zombie());
    }
}
