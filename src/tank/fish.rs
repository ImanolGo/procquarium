//! A single fish. At this stage it is a static sprite; motion arrives in M3.

use crate::source::ProcInfo;

use super::mapping;

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
    /// Size class, 0..=3.
    pub size: f32,
    pub facing: Facing,
}

impl Fish {
    pub fn new(info: ProcInfo, pos: (f32, f32), facing: Facing) -> Self {
        let size = f32::from(mapping::size_class(info.memory));
        Self {
            pid: info.pid,
            info,
            pos,
            size,
            facing,
        }
    }

    /// Update the appearance from a fresh sample.
    pub fn apply_info(&mut self, info: ProcInfo) {
        self.size = f32::from(mapping::size_class(info.memory));
        self.info = info;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::fake::proc;

    #[test]
    fn new_fish_matches_mapping() {
        let f = Fish::new(
            proc(1, "test").with_memory(64 * 1024 * 1024),
            (20.0, 10.0),
            Facing::Right,
        );
        assert_eq!(f.size, 1.0);
        assert_eq!(f.pid, 1);
    }

    #[test]
    fn apply_info_resizes_the_fish() {
        let mut f = Fish::new(proc(1, "test"), (0.0, 0.0), Facing::Left);
        assert_eq!(f.size, 0.0);
        f.apply_info(proc(1, "test").with_memory(2 * 1024 * 1024 * 1024));
        assert_eq!(f.size, 3.0);
    }
}
