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
use fish::{Facing, Fish};

/// The aquarium.
pub struct Tank {
    pub fish: Vec<Fish>,
    pub decor: Decor,
    pub width: u16,
    pub height: u16,
    pub time: f32,
    rng: ChaCha8Rng,
}

impl Tank {
    pub fn new(width: u16, height: u16, _max_fish: usize, seed: u64) -> Self {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let mut decor = Decor::new();
        decor.configure(width, height, &mut rng);
        Self {
            fish: Vec::new(),
            decor,
            width,
            height,
            time: 0.0,
            rng,
        }
    }

    /// True if this pid is already represented by a fish.
    pub fn contains(&self, pid: u32) -> bool {
        self.fish.iter().any(|f| f.pid == pid)
    }

    /// Apply process events. Only selected processes get a fish.
    pub fn apply(&mut self, events: &[ProcEvent], selected: &HashSet<u32>) {
        for event in events {
            match event {
                ProcEvent::Spawned(info) => {
                    if selected.contains(&info.pid) {
                        self.spawn_fish(info.clone());
                    }
                }
                ProcEvent::Exited { pid } => {
                    self.fish.retain(|f| f.pid != *pid);
                }
                ProcEvent::Changed(info) => {
                    if let Some(f) = self.fish.iter_mut().find(|f| f.pid == info.pid) {
                        f.apply_info(info.clone());
                    } else if selected.contains(&info.pid) {
                        self.spawn_fish(info.clone());
                    }
                }
            }
        }
    }

    fn spawn_fish(&mut self, info: ProcInfo) {
        if self.contains(info.pid) {
            return;
        }
        let w = self.width.max(4) as f32;
        let h = self.height.max(3) as f32;
        let x = self.rng.random_range(2.0..(w - 2.0).max(3.0));
        let y = self.rng.random_range(1.0..(h - 2.0).max(1.5));
        let phase = self.rng.random_range(0.0..std::f32::consts::TAU);
        let facing = if self.rng.random::<bool>() {
            Facing::Right
        } else {
            Facing::Left
        };
        self.fish.push(Fish::new(info, (x, y), phase, facing));
    }

    /// Advance the whole tank by `dt` seconds.
    pub fn update(&mut self, dt: f32) {
        self.time += dt;
        let w = self.width.max(2) as f32;
        let h = self.height.max(3) as f32;

        // Snapshot parent positions before mutating the fish.
        let positions: HashMap<u32, (f32, f32)> =
            self.fish.iter().map(|f| (f.pid, f.pos)).collect();

        for f in &mut self.fish {
            let parent = f.info.parent.and_then(|p| positions.get(&p).copied());
            f.update(dt, w, h, parent);
        }

        self.decor
            .update(dt, self.width, self.height, &mut self.rng, &self.fish);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::fake::proc;

    #[test]
    fn selected_spawn_becomes_a_fish() {
        let mut tank = Tank::new(80, 24, 60, 1);
        let selected: HashSet<u32> = [1u32].into_iter().collect();
        tank.apply(&[ProcEvent::Spawned(proc(1, "shell"))], &selected);
        assert_eq!(tank.fish.len(), 1);
        assert_eq!(tank.fish[0].pid, 1);
    }

    #[test]
    fn exit_removes_the_fish() {
        let mut tank = Tank::new(80, 24, 60, 1);
        let selected: HashSet<u32> = [1u32].into_iter().collect();
        tank.apply(&[ProcEvent::Spawned(proc(1, "shell"))], &selected);
        tank.apply(&[ProcEvent::Exited { pid: 1 }], &HashSet::new());
        assert!(tank.fish.is_empty());
    }

    #[test]
    fn unselected_spawn_gets_no_fish() {
        let mut tank = Tank::new(80, 24, 60, 1);
        tank.apply(&[ProcEvent::Spawned(proc(1, "shell"))], &HashSet::new());
        assert!(tank.fish.is_empty());
    }

    #[test]
    fn update_moves_fish_and_keeps_them_in_the_water() {
        let mut tank = Tank::new(80, 24, 60, 1);
        let selected: HashSet<u32> = [1u32].into_iter().collect();
        tank.apply(
            &[ProcEvent::Spawned(proc(1, "shell").with_cpu(50.0))],
            &selected,
        );
        let start = tank.fish[0].pos;
        for _ in 0..300 {
            tank.update(0.033);
        }
        assert_ne!(tank.fish[0].pos, start, "fish should have moved");
        assert!((1.0..=22.0).contains(&tank.fish[0].pos.1));
    }
}
