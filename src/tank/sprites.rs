//! Fish sprites per size class, with mirroring and zombie/ASCII variants.
//!
//! The built-in set lives in [`Sprites::default`]; a theme file can replace any
//! of it.

/// Right-facing sprite per size class, in the default (Unicode) style.
pub const RIGHT: [&str; 4] = ["><>", "><(°>", "><((°>", "><(((°>"];

/// Right-facing sprite per size class, in ASCII style.
pub const RIGHT_ASCII: [&str; 4] = ["><>", "><(o>", "><((o>", "><(((o>"];

/// The default crab (a kernel thread).
pub const CRAB: &str = "><°°><";
/// The default ASCII crab.
pub const CRAB_ASCII: &str = "><oo><";
/// The default jellyfish (a container process).
pub const JELLYFISH: &str = "~(°°)~";
/// The default ASCII jellyfish.
pub const JELLYFISH_ASCII: &str = "~(oo)~";

/// Mirror a right-facing sprite into a left-facing one by reversing the
/// character order and swapping the mirrored glyphs. Swapping alone is not
/// enough: the eye and tail have to change ends too.
pub fn mirror(chars: &[char]) -> Vec<char> {
    chars.iter().rev().map(|&c| swap(c)).collect()
}

/// The left/right partner of a glyph, for [`mirror`].
fn swap(c: char) -> char {
    match c {
        '>' => '<',
        '<' => '>',
        '(' => ')',
        ')' => '(',
        '[' => ']',
        ']' => '[',
        '{' => '}',
        '}' => '{',
        '/' => '\\',
        '\\' => '/',
        '«' => '»',
        '»' => '«',
        '‹' => '›',
        '›' => '‹',
        other => other,
    }
}

/// The eye glyph for a fish.
pub fn eye(ascii: bool, zombie: bool) -> char {
    match (ascii, zombie) {
        (false, false) => '°',
        (false, true) => '✕',
        (true, false) => 'o',
        (true, true) => 'x',
    }
}

fn is_eye(c: char) -> bool {
    c == '°' || c == 'o'
}

fn replace_eye(chars: &mut [char], new_eye: char) {
    for c in chars.iter_mut() {
        if is_eye(*c) {
            *c = new_eye;
        }
    }
}

/// The full set of sprites, replacing the built-in ones only where a theme
/// overrides them.
#[derive(Debug, Clone)]
pub struct Sprites {
    pub fish: [String; 4],
    pub fish_ascii: [String; 4],
    pub crab: String,
    pub crab_ascii: String,
    pub jellyfish: String,
    pub jellyfish_ascii: String,
}

impl Default for Sprites {
    fn default() -> Self {
        Self {
            fish: RIGHT.map(str::to_string),
            fish_ascii: RIGHT_ASCII.map(str::to_string),
            crab: CRAB.to_string(),
            crab_ascii: CRAB_ASCII.to_string(),
            jellyfish: JELLYFISH.to_string(),
            jellyfish_ascii: JELLYFISH_ASCII.to_string(),
        }
    }
}

impl Sprites {
    fn fish_base(&self, size: u8, ascii: bool) -> &str {
        let size = size.min(3) as usize;
        if ascii {
            &self.fish_ascii[size]
        } else {
            &self.fish[size]
        }
    }

    /// Build a fish sprite into `out`, reusing the buffer.
    pub fn fish_into(
        &self,
        out: &mut Vec<char>,
        size: u8,
        facing_left: bool,
        ascii: bool,
        zombie: bool,
    ) {
        out.clear();
        out.extend(self.fish_base(size, ascii).chars());
        replace_eye(out, eye(ascii, zombie));
        if facing_left {
            let mirrored = mirror(out);
            *out = mirrored;
        }
    }

    /// A dying fish: zombie eye, tail removed, mirrored as needed.
    pub fn dead_fish_into(&self, out: &mut Vec<char>, size: u8, facing_left: bool, ascii: bool) {
        self.fish_into(out, size, false, ascii, true);
        if !out.is_empty() {
            out.remove(0);
        }
        if facing_left {
            let mirrored = mirror(out);
            *out = mirrored;
        }
    }

    /// A crab. Crabs walk sideways, so there is no facing.
    pub fn crab_into(&self, out: &mut Vec<char>, ascii: bool, zombie: bool) {
        out.clear();
        out.extend(if ascii { &self.crab_ascii } else { &self.crab }.chars());
        replace_eye(out, eye(ascii, zombie));
    }

    /// A jellyfish.
    pub fn jellyfish_into(&self, out: &mut Vec<char>, ascii: bool, zombie: bool) {
        out.clear();
        out.extend(
            if ascii {
                &self.jellyfish_ascii
            } else {
                &self.jellyfish
            }
            .chars(),
        );
        replace_eye(out, eye(ascii, zombie));
    }

    /// Every sprite has to be non-empty, otherwise nothing would be drawn.
    pub fn is_valid(&self) -> bool {
        self.fish.iter().all(|s| !s.is_empty())
            && self.fish_ascii.iter().all(|s| !s.is_empty())
            && !self.crab.is_empty()
            && !self.crab_ascii.is_empty()
            && !self.jellyfish.is_empty()
            && !self.jellyfish_ascii.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sprite(size: u8, facing_left: bool, ascii: bool, zombie: bool) -> Vec<char> {
        let mut out = Vec::new();
        Sprites::default().fish_into(&mut out, size, facing_left, ascii, zombie);
        out
    }

    #[test]
    fn mirror_swaps_and_reverses() {
        assert_eq!(
            mirror(&['>', '<', '(', '°', '>']),
            vec!['<', '°', ')', '>', '<']
        );
        assert_eq!(mirror(&['>', '<', '>']), vec!['<', '>', '<']);
        // A theme can use other bracket pairs.
        assert_eq!(
            mirror(&['[', ']', '{', '}', '/', '\\']),
            vec!['/', '\\', '{', '}', '[', ']']
        );
    }

    #[test]
    fn mirror_is_an_involution() {
        for base in RIGHT {
            let chars: Vec<char> = base.chars().collect();
            assert_eq!(mirror(&mirror(&chars)), chars);
        }
    }

    #[test]
    fn right_facing_sprites_are_as_specified() {
        assert_eq!(
            sprite(0, false, false, false),
            "><>".chars().collect::<Vec<_>>()
        );
        assert_eq!(
            sprite(3, false, false, false),
            "><(((°>".chars().collect::<Vec<_>>()
        );
    }

    #[test]
    fn left_facing_is_mirrored() {
        assert_eq!(
            sprite(1, true, false, false),
            "<°)><".chars().collect::<Vec<_>>()
        );
        assert_eq!(
            sprite(3, true, false, false),
            "<°)))><".chars().collect::<Vec<_>>()
        );
    }

    #[test]
    fn zombie_eye_replaces_the_normal_eye() {
        assert_eq!(
            sprite(2, false, false, true),
            "><((✕>".chars().collect::<Vec<_>>()
        );
        assert_eq!(
            sprite(2, false, true, true),
            "><((x>".chars().collect::<Vec<_>>()
        );
    }

    #[test]
    fn dead_sprite_drops_the_tail_and_uses_a_zombie_eye() {
        let mut out = Vec::new();
        Sprites::default().dead_fish_into(&mut out, 2, false, false);
        assert_eq!(out, "<((✕>".chars().collect::<Vec<_>>());

        let mut mirrored = Vec::new();
        Sprites::default().dead_fish_into(&mut mirrored, 2, true, false);
        assert_eq!(mirrored, "<✕))>".chars().collect::<Vec<_>>());
    }

    #[test]
    fn crab_and_jellyfish_sprites() {
        let s = Sprites::default();
        let mut crab = Vec::new();
        s.crab_into(&mut crab, false, false);
        assert_eq!(crab, "><°°><".chars().collect::<Vec<_>>());
        s.crab_into(&mut crab, true, true);
        assert_eq!(crab, "><xx><".chars().collect::<Vec<_>>());

        let mut jelly = Vec::new();
        s.jellyfish_into(&mut jelly, false, false);
        assert_eq!(jelly, "~(°°)~".chars().collect::<Vec<_>>());
        s.jellyfish_into(&mut jelly, true, true);
        assert_eq!(jelly, "~(xx)~".chars().collect::<Vec<_>>());
    }

    #[test]
    fn ascii_uses_o_eye_and_zombie_x() {
        assert_eq!(
            sprite(1, false, true, false),
            "><(o>".chars().collect::<Vec<_>>()
        );
        assert_eq!(
            sprite(1, false, true, true),
            "><(x>".chars().collect::<Vec<_>>()
        );
    }

    #[test]
    fn size_zero_has_no_eye_but_still_mirrors() {
        assert_eq!(
            sprite(0, true, false, true),
            "<><".chars().collect::<Vec<_>>()
        );
    }

    #[test]
    fn custom_sprites_are_used() {
        let mut s = Sprites::default();
        s.fish[0] = ">>>".to_string();
        let mut out = Vec::new();
        s.fish_into(&mut out, 0, false, false, false);
        assert_eq!(out, ">>>".chars().collect::<Vec<_>>());
        assert!(s.is_valid());

        s.crab = String::new();
        assert!(!s.is_valid());
    }
}
