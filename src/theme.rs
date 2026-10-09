//! The tank's colours and sprites: a built-in theme, optionally overridden by
//! the `[theme]` table of a config file.
//!
//! [`crate::config::FileConfig`] reads the config file (from `--config <PATH>`,
//! or `$XDG_CONFIG_HOME/procquarium/config.toml`, falling back to
//! `~/.config/procquarium/config.toml`) and hands the `[theme]` table to
//! [`Theme::apply_section`]. If there is no file, the built-in theme is used.

use anyhow::{Result, bail};
use ratatui::style::Color;
use serde::Deserialize;

use crate::tank::mapping;
use crate::tank::sprites::Sprites;

/// Colours and sprites the tank is drawn with.
#[derive(Debug, Clone)]
pub struct Theme {
    pub palette: Vec<Color>,
    pub sprites: Sprites,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            palette: mapping::PALETTE.to_vec(),
            sprites: Sprites::default(),
        }
    }
}

impl Theme {
    /// Apply the `[theme]` table of a config file on top of this theme.
    /// Everything is optional; anything absent keeps its current value.
    pub fn apply_section(&mut self, section: ThemeSection) -> Result<()> {
        if let Some(palette) = section.palette {
            if palette.is_empty() {
                bail!("theme.palette must contain at least one colour");
            }
            self.palette = palette
                .iter()
                .map(|entry| parse_hex(entry))
                .collect::<Result<Vec<_>>>()?;
        }

        if let Some(sprites) = section.sprites {
            let base = std::mem::take(&mut self.sprites);
            self.sprites = sprites.apply(base)?;
        }

        if !self.sprites.is_valid() {
            bail!("sprites must all be non-empty");
        }
        Ok(())
    }
}

fn parse_hex(text: &str) -> Result<Color> {
    let hex = text.strip_prefix('#').unwrap_or(text);
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        bail!("palette entry {text:?} is not a #rrggbb colour");
    }
    let channel = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).unwrap_or(0);
    Ok(Color::Rgb(channel(0), channel(2), channel(4)))
}

/// The `[theme]` table of a config file: an optional palette and optional
/// sprite overrides.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeSection {
    palette: Option<Vec<String>>,
    sprites: Option<SpritesFile>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct SpritesFile {
    fish: Option<Vec<String>>,
    fish_ascii: Option<Vec<String>>,
    crab: Option<String>,
    crab_ascii: Option<String>,
    jellyfish: Option<String>,
    jellyfish_ascii: Option<String>,
}

impl SpritesFile {
    fn apply(self, mut sprites: Sprites) -> Result<Sprites> {
        if let Some(fish) = self.fish {
            sprites.fish = to_four(fish, "sprites.fish")?;
        }
        if let Some(fish) = self.fish_ascii {
            sprites.fish_ascii = to_four(fish, "sprites.fish_ascii")?;
        }
        if let Some(crab) = self.crab {
            sprites.crab = crab;
        }
        if let Some(crab) = self.crab_ascii {
            sprites.crab_ascii = crab;
        }
        if let Some(jellyfish) = self.jellyfish {
            sprites.jellyfish = jellyfish;
        }
        if let Some(jellyfish) = self.jellyfish_ascii {
            sprites.jellyfish_ascii = jellyfish;
        }
        Ok(sprites)
    }
}

fn to_four(mut list: Vec<String>, field: &str) -> Result<[String; 4]> {
    if list.len() != 4 {
        bail!("{field} must have exactly four entries, one per size class");
    }
    Ok([
        list.remove(0),
        list.remove(0),
        list.remove(0),
        list.remove(0),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parse a bare `[theme]` table and apply it to the default theme.
    fn theme_from(text: &str) -> Result<Theme> {
        let mut theme = Theme::default();
        theme.apply_section(toml::from_str(text)?)?;
        Ok(theme)
    }

    #[test]
    fn empty_section_is_the_default_theme() {
        let theme = theme_from("").expect("valid");
        assert_eq!(theme.palette, mapping::PALETTE.to_vec());
        assert_eq!(theme.sprites.fish[0], "><>");
    }

    #[test]
    fn custom_palette_is_parsed() {
        let theme = theme_from(r##"palette = ["#010203", "ffeedd"]"##).expect("valid");
        assert_eq!(
            theme.palette,
            vec![Color::Rgb(1, 2, 3), Color::Rgb(255, 238, 221)]
        );
    }

    #[test]
    fn custom_sprites_override_only_what_is_given() {
        let theme = theme_from(
            r#"
            [sprites]
            crab = "><°°><!"
            fish = ["a", "b", "c", "d"]
            "#,
        )
        .expect("valid");
        assert_eq!(theme.sprites.crab, "><°°><!");
        assert_eq!(theme.sprites.fish, ["a", "b", "c", "d"]);
        assert_eq!(theme.sprites.fish_ascii[0], "><>");
    }

    #[test]
    fn rejects_bad_colours() {
        assert!(theme_from(r#"palette = ["nope"]"#).is_err());
        assert!(theme_from("palette = []").is_err());
    }

    #[test]
    fn rejects_wrong_sprite_counts_and_unknown_fields() {
        assert!(theme_from("[sprites]\nfish = [\"a\", \"b\"]").is_err());
        assert!(theme_from("nonsense = 1").is_err());
    }
}
