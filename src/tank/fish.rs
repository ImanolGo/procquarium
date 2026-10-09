//! A single fish: motion and easing towards its targets.

use crate::source::ProcInfo;

use super::mapping;

/// Time constant for easing size and speed towards their targets.
const EASE_TAU: f32 = 0.35;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Facing {
    Left,
    Right,
}

#[derive(Debug, Clone)]
pub struct Fish {
    pub pid: u32,
    pub info: ProcInfo,
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
    pub age: f32,
}

impl Fish {
    pub fn new(info: ProcInfo, pos: (f32, f32), phase: f32, facing: Facing) -> Self {
        let target_size = f32::from(mapping::size_class(info.memory));
        let target_speed = mapping::target_speed(info.cpu);
        let vel_x = match facing {
            Facing::Right => target_speed,
            Facing::Left => -target_speed,
        };
        Self {
            pid: info.pid,
            info,
            pos,
            vel: (vel_x, 0.0),
            size: target_size,
            target_size,
            speed: target_speed,
            target_speed,
            facing,
            phase,
            age: 0.0,
        }
    }

    /// Update the targets from a fresh sample; the current values ease towards
    /// them in [`Fish::update`].
    pub fn apply_info(&mut self, info: ProcInfo) {
        self.target_size = f32::from(mapping::size_class(info.memory));
        self.target_speed = mapping::target_speed(info.cpu);
        self.info = info;
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
    use crate::source::fake::proc;

    fn fish(mem: u64, cpu: f32) -> Fish {
        let info = proc(1, "test").with_memory(mem).with_cpu(cpu);
        Fish::new(info, (20.0, 10.0), 0.0, Facing::Right)
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
}
