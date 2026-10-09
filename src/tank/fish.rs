//! A single fish: motion, easing towards targets, and lifecycle state.

use crate::source::{ProcInfo, ProcStatus};

use super::mapping;

/// Time constant for easing size and speed towards their targets.
const EASE_TAU: f32 = 0.35;
/// How long a dead fish spends drifting to the surface before it starts to fade.
const DEATH_TIME: f32 = 3.0;
/// How long the fade-out takes once it has reached the surface.
const FADE_TIME: f32 = 1.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Facing {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FishState {
    /// Swimming normally.
    Alive,
    /// A process exited: float to the surface and fade out.
    Exiting,
    /// Dropped out of the top N: swim off screen without dying.
    Leaving,
}

#[derive(Debug, Clone)]
pub struct Fish {
    pub pid: u32,
    pub info: ProcInfo,
    pub state: FishState,
    /// Centre position in cells.
    pub pos: (f32, f32),
    pub vel: (f32, f32),
    /// Eased size class, 0..=3.
    pub size: f32,
    pub target_size: f32,
    /// Eased cruising speed in cells/second.
    pub speed: f32,
    pub target_speed: f32,
    pub facing: Facing,
    /// Per-fish phase for the deterministic wander noise.
    pub phase: f32,
    /// A slowly drifting depth the fish steers towards, so the water column
    /// fills up instead of every fish hugging the sand.
    pub home_y: f32,
    pub age: f32,
    /// Time spent in the current lifecycle state.
    pub timer: f32,
    /// 0..1 progress towards removal (fade for `Exiting`, off-screen for `Leaving`).
    pub death: f32,
    /// Horizontal direction used while leaving the tank.
    pub leave_dir: f32,
}

impl Fish {
    pub fn new(info: ProcInfo, pos: (f32, f32), home_y: f32, phase: f32, facing: Facing) -> Self {
        let target_size = f32::from(mapping::size_class(info.memory));
        let target_speed = if info.status == ProcStatus::Zombie {
            1.0
        } else {
            mapping::target_speed(info.cpu)
        };
        let vel_x = match facing {
            Facing::Right => target_speed,
            Facing::Left => -target_speed,
        };
        Self {
            pid: info.pid,
            info,
            state: FishState::Alive,
            pos,
            vel: (vel_x, 0.0),
            size: target_size,
            target_size,
            speed: target_speed,
            target_speed,
            facing,
            phase,
            home_y,
            age: 0.0,
            timer: 0.0,
            death: 0.0,
            leave_dir: 0.0,
        }
    }

    /// Update the targets from a fresh sample; the current values ease towards
    /// them in [`Fish::update`].
    pub fn apply_info(&mut self, info: ProcInfo) {
        self.target_size = f32::from(mapping::size_class(info.memory));
        self.target_speed = if info.status == ProcStatus::Zombie {
            1.0
        } else {
            mapping::target_speed(info.cpu)
        };
        self.info = info;
    }

    pub fn begin_exit(&mut self) {
        if self.state == FishState::Alive {
            self.state = FishState::Exiting;
            self.timer = 0.0;
            self.death = 0.0;
        }
    }

    pub fn begin_leaving(&mut self, dir: f32) {
        if self.state == FishState::Alive {
            self.state = FishState::Leaving;
            self.leave_dir = if dir >= 0.0 { 1.0 } else { -1.0 };
            self.timer = 0.0;
        }
    }

    /// True once the fish should be removed from the tank.
    pub fn finished(&self) -> bool {
        self.death >= 1.0
    }

    pub fn is_zombie(&self) -> bool {
        self.info.status == ProcStatus::Zombie
    }

    /// Advance one frame. `w`/`h` are the tank size in cells; `parent` is the
    /// parent fish's position if it is currently in the tank.
    pub fn update(&mut self, dt: f32, w: f32, h: f32, parent: Option<(f32, f32)>) {
        self.age += dt;
        let top = 1.0_f32;
        let bottom = (h - 2.0).max(top);
        let margin = 3.0_f32.min(w * 0.25).max(1.0);
        let ease = 1.0 - (-dt / EASE_TAU).exp();
        self.size += (self.target_size - self.size) * ease;
        self.speed += (self.target_speed - self.speed) * ease;

        match self.state {
            FishState::Exiting => {
                self.timer += dt;
                let rise = 1.0 - (-dt / 1.2).exp();
                self.pos.1 += (0.3 - self.pos.1) * rise;
                self.pos.0 += self.vel.0 * dt * 0.15;
                self.pos.0 = self.pos.0.clamp(0.5, (w - 1.5).max(0.5));
                if self.timer >= DEATH_TIME || self.pos.1 <= 0.6 {
                    self.death += dt / FADE_TIME;
                }
                return;
            }
            FishState::Leaving => {
                self.timer += dt;
                let accel = 1.0 - (-dt / 0.4).exp();
                let target = self.leave_dir * (self.speed + 8.0);
                self.vel.0 += (target - self.vel.0) * accel;
                self.vel.1 += (0.0 - self.vel.1) * accel;
                self.pos.0 += self.vel.0 * dt;
                self.pos.1 += self.vel.1 * dt;
                self.facing = if self.vel.0 >= 0.0 {
                    Facing::Right
                } else {
                    Facing::Left
                };
                if self.pos.0 < -12.0
                    || self.pos.0 > w + 12.0
                    || self.pos.1 < -4.0
                    || self.pos.1 > h + 4.0
                    || self.timer > 15.0
                {
                    self.death = 1.0;
                }
                return;
            }
            FishState::Alive => {}
        }

        // Wander: deterministic pseudo-noise, so a fixed seed gives a fixed tank.
        let noise_x = (self.age * 0.8 + self.phase).sin();
        let noise_y = (self.age * 0.6 + self.phase * 1.7).cos();
        self.vel.0 += noise_x * 8.0 * dt;
        self.vel.1 += noise_y * 3.0 * dt;

        // School gently with the parent, aiming a little behind it.
        if let Some((px, py)) = parent {
            let behind = if self.pos.0 >= px { 2.0 } else { -2.0 };
            self.vel.0 += (px + behind - self.pos.0) * 0.5 * dt;
            self.vel.1 += (py - self.pos.1) * 0.5 * dt;
        }

        // Drift towards a slowly moving home depth so the whole water column
        // fills up rather than everyone settling on the sand.
        let span = (bottom - top).max(1.0);
        let desired_y = self.home_y + (self.age * 0.25 + self.phase).sin() * span * 0.15;
        self.vel.1 += (desired_y - self.pos.1) * 0.8 * dt;

        // Cruise at the current (eased) speed.
        let dir = if self.vel.0 >= 0.0 { 1.0 } else { -1.0 };
        let cruise = 1.0 - (-dt / 0.5).exp();
        self.vel.0 += (dir * self.speed - self.vel.0) * cruise;

        // Soft repulsion from the walls.
        if self.pos.0 < margin {
            self.vel.0 += (margin - self.pos.0) * 12.0 * dt;
        } else if self.pos.0 > w - 1.0 - margin {
            self.vel.0 -= (self.pos.0 - (w - 1.0 - margin)) * 12.0 * dt;
        }
        if self.pos.1 < top + 1.0 {
            self.vel.1 += (top + 1.0 - self.pos.1) * 10.0 * dt;
        } else if self.pos.1 > bottom - 1.0 {
            self.vel.1 -= (self.pos.1 - (bottom - 1.0)) * 10.0 * dt;
        }

        // Vertical motion stays much gentler than horizontal.
        let vmax_x = (self.speed + 3.0).max(3.0);
        self.vel.0 = self.vel.0.clamp(-vmax_x, vmax_x);
        let vmax_y = (self.speed * 0.35).max(1.0);
        self.vel.1 = self.vel.1.clamp(-vmax_y, vmax_y);

        // Integrate, then hard-clamp so a fish never leaves the water.
        self.pos.0 += self.vel.0 * dt;
        self.pos.1 += self.vel.1 * dt;
        self.pos.0 = self.pos.0.clamp(0.5, (w - 1.5).max(0.5));
        self.pos.1 = self.pos.1.clamp(top, bottom);

        if self.vel.0 > 0.05 {
            self.facing = Facing::Right;
        } else if self.vel.0 < -0.05 {
            self.facing = Facing::Left;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::ProcStatus;
    use crate::source::fake::proc;

    fn fish(mem: u64, cpu: f32) -> Fish {
        let info = proc(1, "test").with_memory(mem).with_cpu(cpu);
        Fish::new(info, (20.0, 10.0), 10.0, 0.0, Facing::Right)
    }

    #[test]
    fn new_fish_matches_mapping() {
        let f = fish(64 * 1024 * 1024, 100.0);
        assert_eq!(f.target_size, 1.0);
        assert!((f.target_speed - 20.0).abs() < 1e-6);
    }

    #[test]
    fn size_eases_towards_target_over_about_a_second() {
        let mut f = fish(0, 0.0);
        // Move target up, then advance one second in small steps.
        f.apply_info(proc(1, "test").with_memory(2 * 1024 * 1024 * 1024));
        assert_eq!(f.target_size, 3.0);
        assert_eq!(f.size, 0.0);
        for _ in 0..100 {
            f.update(0.01, 80.0, 24.0, None);
        }
        assert!(
            f.size > 2.5,
            "size should mostly catch up in a second: {}",
            f.size
        );
        assert!(f.size < 3.0, "but not instantly");
    }

    #[test]
    fn fish_never_leaves_the_water() {
        let mut f = fish(0, 0.0);
        for _ in 0..5000 {
            f.update(0.05, 80.0, 24.0, None);
            assert!(
                (0.5..=78.5).contains(&f.pos.0),
                "x out of bounds: {}",
                f.pos.0
            );
            assert!(
                (1.0..=22.0).contains(&f.pos.1),
                "y out of bounds: {}",
                f.pos.1
            );
        }
    }

    #[test]
    fn facing_follows_horizontal_velocity() {
        let mut f = fish(0, 0.0);
        f.vel = (-10.0, 0.0);
        f.update(0.1, 80.0, 24.0, None);
        assert_eq!(f.facing, Facing::Left);
        f.vel = (10.0, 0.0);
        f.update(0.1, 80.0, 24.0, None);
        assert_eq!(f.facing, Facing::Right);
    }

    #[test]
    fn exiting_floats_up_then_fades_and_finishes() {
        let mut f = fish(0, 0.0);
        f.begin_exit();
        assert_eq!(f.state, FishState::Exiting);
        let start_y = f.pos.1;
        for _ in 0..200 {
            f.update(0.05, 80.0, 24.0, None);
        }
        assert!(f.pos.1 < start_y, "should rise towards the surface");
        assert!(f.finished(), "should eventually be removed");
    }

    #[test]
    fn leaving_swims_off_screen() {
        let mut f = fish(0, 0.0);
        f.begin_leaving(1.0);
        for _ in 0..500 {
            f.update(0.05, 80.0, 24.0, None);
        }
        assert!(f.finished());
        assert!(f.pos.0 > 78.0, "should have swum off to the right");
    }

    #[test]
    fn zombie_gets_a_slow_target_speed() {
        let mut f = fish(64 * 1024 * 1024, 100.0);
        f.apply_info(
            proc(1, "test")
                .with_memory(64 * 1024 * 1024)
                .with_cpu(100.0)
                .with_status(ProcStatus::Zombie),
        );
        assert_eq!(f.target_speed, 1.0);
        assert!(f.is_zombie());
    }
}
