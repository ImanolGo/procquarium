//! Pure mappings from process numbers to fish appearance.
//!
//! Everything here is deterministic and free of I/O so it can be unit tested.

use ratatui::style::Color;

const MIB: u64 = 1024 * 1024;

/// Size class 0..=3 from resident memory on a log-ish scale.
///
/// 0: < 16 MiB, 1: < 128 MiB, 2: < 1 GiB, 3: >= 1 GiB.
pub fn size_class(memory: u64) -> u8 {
    if memory < 16 * MIB {
        0
    } else if memory < 128 * MIB {
        1
    } else if memory < 1024 * MIB {
        2
    } else {
        3
    }
}

/// Target cruising speed in cells per second from CPU usage.
///
/// Idle processes still drift slowly at 2 cells/s; a process pegged at one core
/// reaches 20 cells/s.
pub fn target_speed(cpu: f32) -> f32 {
    2.0 + 18.0 * (cpu / 100.0).clamp(0.0, 1.0).sqrt()
}

/// A palette of twelve colours chosen to read well on the dark blue water (and
/// reasonably on a light terminal, since the water background is always ours).
pub const PALETTE: [Color; 12] = [
    Color::Rgb(255, 183, 77),  // amber
    Color::Rgb(129, 199, 132), // green
    Color::Rgb(77, 208, 225),  // cyan
    Color::Rgb(100, 181, 246), // blue
    Color::Rgb(186, 104, 200), // purple
    Color::Rgb(240, 98, 146),  // pink
    Color::Rgb(255, 138, 101), // coral
    Color::Rgb(220, 231, 117), // lime
    Color::Rgb(121, 134, 203), // indigo
    Color::Rgb(255, 213, 79),  // yellow
    Color::Rgb(128, 222, 234), // pale cyan
    Color::Rgb(144, 164, 174), // blue grey
];

/// FNV-1a hash, stable across runs and platforms (unlike `DefaultHasher`).
pub fn fnv1a(input: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in input.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Colour for a process name; the same name always gets the same colour.
pub fn color_for_name(name: &str) -> Color {
    PALETTE[(fnv1a(name) % PALETTE.len() as u64) as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_class_boundaries() {
        assert_eq!(size_class(0), 0);
        assert_eq!(size_class(15 * MIB), 0);
        assert_eq!(size_class(16 * MIB), 1);
        assert_eq!(size_class(127 * MIB), 1);
        assert_eq!(size_class(128 * MIB), 2);
        assert_eq!(size_class(1023 * MIB), 2);
        assert_eq!(size_class(1024 * MIB), 3);
        assert_eq!(size_class(64 * 1024 * MIB), 3);
    }

    #[test]
    fn speed_is_bounded_and_monotonic() {
        assert_eq!(target_speed(0.0), 2.0);
        assert!((target_speed(100.0) - 20.0).abs() < 1e-6);
        assert!(
            (target_speed(400.0) - 20.0).abs() < 1e-6,
            "clamped at one core"
        );
        assert!(target_speed(25.0) < target_speed(64.0));
        assert!(target_speed(64.0) < target_speed(100.0));
    }

    #[test]
    fn colour_is_stable_and_in_palette() {
        assert_eq!(color_for_name("firefox"), color_for_name("firefox"));
        assert!(PALETTE.contains(&color_for_name("firefox")));
    }

    #[test]
    fn fnv1a_matches_known_vectors() {
        assert_eq!(fnv1a(""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a("a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv1a("foobar"), 0x8594_4171_f739_67e8);
    }
}
