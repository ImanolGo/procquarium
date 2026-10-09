//! The tank owns the fish. At this stage they are spawned and removed but do
//! not move yet.

pub mod fish;
pub mod mapping;
pub mod sprites;

use std::collections::HashSet;

use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use crate::diff::ProcEvent;
use crate::source::ProcInfo;

use fish::{Facing, Fish};

/// The aquarium.
pub struct Tank {
    pub fish: Vec<Fish>,
    pub width: u16,
    pub height: u16,
    rng: ChaCha8Rng,
}

impl Tank {
    pub fn new(width: u16, height: u16, _max_fish: usize, seed: u64) -> Self {
        Self {
            fish: Vec::new(),
            width,
            height,
            rng: ChaCha8Rng::seed_from_u64(seed),
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
        let facing = if self.rng.random::<bool>() {
            Facing::Right
        } else {
            Facing::Left
        };
        self.fish.push(Fish::new(info, (x, y), facing));
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
}
