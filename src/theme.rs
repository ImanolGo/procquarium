//! An optional user theme: a colour palette and custom sprites from TOML.
//!
//! Loaded from `--config <PATH>`, or `$XDG_CONFIG_HOME/procquarium/config.toml`
//! (falling back to `~/.config/procquarium/config.toml`). If there is no file,
//! the built-in theme is used.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
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
    /// Load the theme from an explicit path, else the default location, else the
    /// built-in theme.
    pub fn load(path: Option<&Path>) -> Result<Self> {
        if let Some(path) = path {
            let text = std::fs::read_to_string(path)
                .with_context(|| format!("reading config {}", path.display()))?;
            return Self::from_toml(&text)
                .with_context(|| format!("parsing config {}", path.display()));
        }

        match default_config_path() {
            Some(path) if path.exists() => {
                let text = std::fs::read_to_string(&path)
                    .with_context(|| format!("reading config {}", path.display()))?;
                Self::from_toml(&text).with_context(|| format!("parsing config {}", path.display()))
            }
            _ => Ok(Self::default()),
        }
    }

    /// Parse a theme from TOML text.
    pub fn from_toml(text: &str) -> Result<Self> {
        let file: ThemeFile = toml::from_str(text)?;
        let mut theme = Self::default();

        if let Some(palette) = file.palette {
            if palette.is_empty() {
                bail!("palette must contain at least one colour");
            }
            theme.palette = palette
                .iter()
                .map(|entry| parse_hex(entry))
                .collect::<Result<Vec<_>>>()?;
        }

        if let Some(sprites) = file.sprites {
            theme.sprites = sprites.apply(theme.sprites)?;
        }

        if !theme.sprites.is_valid() {
            bail!("sprites must all be non-empty");
        }
        Ok(theme)
    }
}

fn default_config_path() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME")
        && !xdg.is_empty()
    {
        return Some(PathBuf::from(xdg).join("procquarium/config.toml"));
    }
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config/procquarium/config.toml"))
}

fn parse_hex(text: &str) -> Result<Color> {
    let hex = text.strip_prefix('#').unwrap_or(text);
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        bail!("palette entry {text:?} is not a #rrggbb colour");
    }
    let channel = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).unwrap_or(0);
    Ok(Color::Rgb(channel(0), channel(2), channel(4)))
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ThemeFile {
    palette: Option<Vec<String>>,
    sprites: Option<SpritesFile>,
}

#[derive(Debug, Default, Deserialize)]
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

    #[test]
    fn empty_config_is_the_default_theme() {
        let theme = Theme::from_toml("").expect("valid");
        assert_eq!(theme.palette, mapping::PALETTE.to_vec());
        assert_eq!(theme.sprites.fish[0], "><>");
    }

    #[test]
    fn custom_palette_is_parsed() {
        let theme = Theme::from_toml(r##"palette = ["#010203", "ffeedd"]"##).expect("valid");
        assert_eq!(
            theme.palette,
            vec![Color::Rgb(1, 2, 3), Color::Rgb(255, 238, 221)]
        );
    }

    #[test]
    fn custom_sprites_override_only_what_is_given() {
        let theme = Theme::from_toml(
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
        assert!(Theme::from_toml(r#"palette = ["nope"]"#).is_err());
        assert!(Theme::from_toml("palette = []").is_err());
    }

    #[test]
    fn rejects_wrong_sprite_counts_and_unknown_fields() {
        assert!(Theme::from_toml("[sprites]\nfish = [\"a\", \"b\"]").is_err());
        assert!(Theme::from_toml("nonsense = 1").is_err());
    }
}
