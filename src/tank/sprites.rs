//! Fish sprites per size class, with mirroring and zombie/ASCII variants.

/// Right-facing sprite per size class, in the default (Unicode) style.
pub const RIGHT: [&str; 4] = ["><>", "><(°>", "><((°>", "><(((°>"];

/// Right-facing sprite per size class, in ASCII style.
pub const RIGHT_ASCII: [&str; 4] = ["><>", "><(o>", "><((o>", "><(((o>"];

/// Mirror a right-facing sprite into a left-facing one by swapping the bracket
/// and arrow characters. The eye is symmetric and passes through unchanged.
pub fn mirror(chars: &[char]) -> Vec<char> {
    chars
        .iter()
        .map(|&c| match c {
            '>' => '<',
            '<' => '>',
            '(' => ')',
            ')' => '(',
            other => other,
        })
        .collect()
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

/// Build the final sprite: size class 0..=3, facing and zombie/ASCII variants.
#[cfg(test)]
pub fn sprite(size: u8, facing_left: bool, ascii: bool, zombie: bool) -> Vec<char> {
    let mut out = Vec::with_capacity(8);
    sprite_into(&mut out, size, facing_left, ascii, zombie);
    out
}

/// Like [`sprite`], but reusing a caller-owned buffer so rendering does not
/// allocate per fish per frame.
pub fn sprite_into(out: &mut Vec<char>, size: u8, facing_left: bool, ascii: bool, zombie: bool) {
    out.clear();
    let size = size.min(3) as usize;
    let base = if ascii {
        RIGHT_ASCII[size]
    } else {
        RIGHT[size]
    };
    out.extend(base.chars());
    replace_eye(out, eye(ascii, zombie));
    if facing_left {
        let mirrored = mirror(out);
        *out = mirrored;
    }
}

/// A dying fish: zombie eye, tail removed, mirrored as needed.
pub fn dead_sprite_into(out: &mut Vec<char>, size: u8, facing_left: bool, ascii: bool) {
    sprite_into(out, size, false, ascii, true);
    if !out.is_empty() {
        out.remove(0);
    }
    if facing_left {
        let mirrored = mirror(out);
        *out = mirrored;
    }
}

const CRAB: &str = "><°°><";
const CRAB_ASCII: &str = "><oo><";
const JELLYFISH: &str = "~(°°)~";
const JELLYFISH_ASCII: &str = "~(oo)~";

/// A crab (kernel thread). Crabs walk sideways, so there is no facing.
pub fn crab_into(out: &mut Vec<char>, ascii: bool, zombie: bool) {
    out.clear();
    out.extend(if ascii { CRAB_ASCII } else { CRAB }.chars());
    replace_eye(out, eye(ascii, zombie));
}

/// A jellyfish (container process).
pub fn jellyfish_into(out: &mut Vec<char>, ascii: bool, zombie: bool) {
    out.clear();
    out.extend(if ascii { JELLYFISH_ASCII } else { JELLYFISH }.chars());
    replace_eye(out, eye(ascii, zombie));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mirror_swaps_brackets_and_arrows() {
        assert_eq!(
            mirror(&['>', '<', '(', '°', '>']),
            vec!['<', '>', ')', '°', '<']
        );
        assert_eq!(mirror(&['>', '<', '>']), vec!['<', '>', '<']);
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
            "<>)°<".chars().collect::<Vec<_>>()
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
        dead_sprite_into(&mut out, 2, false, false);
        assert_eq!(out, "<((✕>".chars().collect::<Vec<_>>());

        let mut mirrored = Vec::new();
        dead_sprite_into(&mut mirrored, 2, true, false);
        assert_eq!(mirrored, ">))✕<".chars().collect::<Vec<_>>());
    }

    #[test]
    fn crab_and_jellyfish_sprites() {
        let mut crab = Vec::new();
        crab_into(&mut crab, false, false);
        assert_eq!(crab, "><°°><".chars().collect::<Vec<_>>());
        crab_into(&mut crab, true, true);
        assert_eq!(crab, "><xx><".chars().collect::<Vec<_>>());

        let mut jelly = Vec::new();
        jellyfish_into(&mut jelly, false, false);
        assert_eq!(jelly, "~(°°)~".chars().collect::<Vec<_>>());
        jellyfish_into(&mut jelly, true, true);
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
}
