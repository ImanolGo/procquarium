//! Tank decoration: sand, surface, swaying seaweed and rising bubbles.

use rand::Rng;
use rand_chacha::ChaCha8Rng;

use super::fish::{Fish, FishState};

/// A strand of seaweed anchored on the sand.
#[derive(Debug, Clone)]
pub struct Strand {
    pub x: f32,
    pub segments: usize,
    pub phase: f32,
    pub speed: f32,
}

/// A bubble rising towards the surface.
#[derive(Debug, Clone)]
pub struct Bubble {
    pub x: f32,
    pub y: f32,
    pub vy: f32,
    pub big: bool,
}

/// All the non-fish moving parts of the tank.
#[derive(Debug, Default)]
pub struct Decor {
    pub strands: Vec<Strand>,
    pub bubbles: Vec<Bubble>,
    pub time: f32,
    bubble_timer: f32,
}

/// A process doing at least this many disk bytes per sample breathes at full
/// rate; below `IO_MIN_BYTES` a trickle is ignored.
const IO_SCALE: f32 = 1024.0 * 1024.0;
const IO_MIN_BYTES: u64 = 16 * 1024;

impl Decor {
    pub fn new() -> Self {
        Self::default()
    }

    /// (Re)generate the static decor for a new terminal size. Density scales
    /// with the width.
    pub fn configure(&mut self, width: u16, height: u16, rng: &mut ChaCha8Rng) {
        let w = width.max(4) as f32;
        let h = height.max(6) as usize;
        let count = (width as usize / 14).clamp(3, 6);
        let max_segments = (h.saturating_sub(4)).clamp(1, 6);
        self.strands = (0..count)
            .map(|_| Strand {
                x: rng.random_range(1.0..(w - 1.0)),
                segments: rng.random_range(1..=max_segments),
                phase: rng.random_range(0.0..std::f32::consts::TAU),
                speed: rng.random_range(0.6..1.4),
            })
            .collect();
        self.bubbles.clear();
        self.time = 0.0;
        self.bubble_timer = rng.random_range(0.2..0.8);
    }

    /// Advance the decor and let fast fish shed a few bubbles.
    pub fn update(
        &mut self,
        dt: f32,
        width: u16,
        height: u16,
        rng: &mut ChaCha8Rng,
        fish: &[Fish],
    ) {
        let w = width.max(2) as f32;
        let h = height.max(3) as f32;
        self.time += dt;

        // Ambient bubbles.
        self.bubble_timer -= dt;
        if self.bubble_timer <= 0.0 {
            let x = rng.random_range(1.0..(w - 1.0).max(2.0));
            self.bubbles.push(Bubble {
                x,
                y: h - 2.0,
                vy: rng.random_range(1.2..3.0),
                big: rng.random::<f32>() < 0.25,
            });
            self.bubble_timer = rng.random_range(0.25..1.1);
        }

        // Bubbles from fast-swimming fish, from just ahead of the mouth.
        for f in fish {
            if f.state == FishState::Alive && f.speed > 12.0 && rng.random::<f32>() < dt * 1.2 {
                let ahead = if f.facing == super::fish::Facing::Right {
                    2.0
                } else {
                    -2.0
                };
                self.bubbles.push(Bubble {
                    x: f.pos.0 + ahead,
                    y: f.pos.1,
                    vy: rng.random_range(1.5..3.5),
                    big: rng.random::<f32>() < 0.4,
                });
            }
        }

        // Bubbles from processes doing disk I/O, in proportion to throughput.
        for f in fish {
            if f.state == FishState::Exiting || f.info.io < IO_MIN_BYTES {
                continue;
            }
            let rate = (f.info.io as f32 / IO_SCALE).clamp(0.0, 1.0);
            if rng.random::<f32>() < rate * dt * 6.0 {
                self.bubbles.push(Bubble {
                    x: f.pos.0,
                    y: f.pos.1,
                    vy: rng.random_range(1.5..3.5),
                    big: true,
                });
            }
        }

        // Rise and retire.
        for b in &mut self.bubbles {
            b.y -= b.vy * dt;
        }
        self.bubbles.retain(|b| b.y > 0.5);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::fake::proc;
    use crate::tank::fish::{Facing, Fish};

    fn rng() -> ChaCha8Rng {
        use rand::SeedableRng;
        ChaCha8Rng::seed_from_u64(42)
    }

    #[test]
    fn configure_scales_strands_with_width() {
        let mut d = Decor::new();
        let mut r = rng();
        d.configure(30, 24, &mut r);
        assert_eq!(d.strands.len(), 3);
        d.configure(200, 24, &mut r);
        assert_eq!(d.strands.len(), 6);
    }

    #[test]
    fn ambient_bubbles_are_spawned_and_retired() {
        let mut d = Decor::new();
        let mut r = rng();
        d.configure(80, 24, &mut r);
        for _ in 0..600 {
            d.update(0.05, 80, 24, &mut r, &[]);
            assert!(d.bubbles.len() < 200, "bubbles must not grow forever");
        }
        assert!(!d.bubbles.is_empty(), "ambient bubbles should appear");
    }

    #[test]
    fn busy_io_sheds_bubbles() {
        let mut d = Decor::new();
        let mut r = rng();
        d.configure(80, 24, &mut r);
        // A fish with no speed but heavy disk I/O should still breathe.
        let f = Fish::new(
            proc(1, "dd").with_memory(0).with_io(8 * 1024 * 1024),
            (40.0, 10.0),
            10.0,
            0.0,
            Facing::Right,
            crate::tank::fish::CreatureKind::Fish,
        );
        let mut spawned = false;
        for _ in 0..200 {
            let before = d.bubbles.len();
            d.update(0.05, 80, 24, &mut r, std::slice::from_ref(&f));
            for b in d.bubbles.iter().skip(before) {
                if (b.x - 40.0).abs() < 2.0 && (b.y - 10.0).abs() < 2.0 {
                    spawned = true;
                }
            }
        }
        assert!(spawned, "an I/O-heavy fish should breathe bubbles");
    }

    #[test]
    fn fast_fish_shed_bubbles() {
        let mut d = Decor::new();
        let mut r = rng();
        d.configure(80, 24, &mut r);
        let mut f = Fish::new(
            proc(1, "fast").with_cpu(100.0),
            (40.0, 10.0),
            10.0,
            0.0,
            Facing::Right,
            crate::tank::fish::CreatureKind::Fish,
        );
        f.speed = 20.0;
        let mut spawned_near_fish = false;
        for _ in 0..200 {
            let before = d.bubbles.len();
            d.update(0.05, 80, 24, &mut r, std::slice::from_ref(&f));
            for b in d.bubbles.iter().skip(before) {
                if (b.x - 40.0).abs() < 3.0 && (b.y - 10.0).abs() < 3.0 {
                    spawned_near_fish = true;
                }
            }
        }
        assert!(
            spawned_near_fish,
            "fast fish should shed bubbles ahead of it"
        );
    }
}
