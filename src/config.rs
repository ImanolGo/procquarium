//! Configuration derived from the command line.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use regex::Regex;
use serde::Deserialize;

use crate::theme::{Theme, ThemeSection};

/// Everything the simulation and UI need to know about how the user wants to run.
#[derive(Debug, Clone)]
pub struct Config {
    /// How often to take a process snapshot.
    pub interval: Duration,
    /// Maximum number of fish (and therefore processes) in the tank.
    pub max_fish: usize,
    /// Only show processes owned by this user.
    pub user: Option<String>,
    /// Only show processes whose name matches this regex.
    pub filter: Option<Regex>,
    /// Include kernel threads.
    pub kernel: bool,
    /// Use plain ASCII glyphs instead of the nicer Unicode ones.
    pub ascii: bool,
    /// Any key exits, and no labels or info box are drawn.
    pub screensaver: bool,
    /// Let `f` drop food, and feed the fish that eat it a priority nudge.
    pub feed: bool,
    /// Fixed RNG seed, for reproducible tanks and tests.
    pub seed: Option<u64>,
    /// Print a snapshot and exit (hidden debugging flag).
    pub dump: bool,
    /// Don't capture the mouse, so text selection in the terminal keeps working.
    pub no_mouse: bool,
    /// Write every snapshot as a JSON line to this file.
    pub record: Option<std::path::PathBuf>,
    /// Replay snapshots from this recording instead of sampling.
    pub replay: Option<std::path::PathBuf>,
    /// Allow `k` to send SIGTERM to the selected process (opt-in).
    pub kill: bool,
    /// Draw in the terminal's default colours (`NO_COLOR`).
    pub mono: bool,
    /// Colours and sprites, from the config file or the built-in theme.
    pub theme: Theme,
}

impl Config {
    /// Start building a config; every field has a sensible default.
    pub fn builder() -> ConfigBuilder {
        ConfigBuilder::default()
    }
}

/// The contents of a config file: optional settings and a `[theme]` table.
///
/// Every field is optional, and every value here is a default that the
/// command-line flags override. Unknown keys are rejected so a typo is an
/// error rather than a silent no-op.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileConfig {
    pub interval: Option<f64>,
    pub max_fish: Option<usize>,
    pub user: Option<String>,
    pub filter: Option<String>,
    pub kernel: Option<bool>,
    pub ascii: Option<bool>,
    pub feed: Option<bool>,
    pub theme: Option<ThemeSection>,
}

impl FileConfig {
    /// Load the config file at `path`, else the default location, else return an
    /// empty config. An explicit `path` that cannot be read is an error; a
    /// missing default file is not.
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

    /// Parse a config file from TOML text.
    pub fn from_toml(text: &str) -> Result<Self> {
        Ok(toml::from_str(text)?)
    }
}

/// The default config file: `$XDG_CONFIG_HOME/procquarium/config.toml`, falling
/// back to `~/.config/procquarium/config.toml`.
fn default_config_path() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME")
        && !xdg.is_empty()
    {
        return Some(PathBuf::from(xdg).join("procquarium/config.toml"));
    }
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config/procquarium/config.toml"))
}

/// Builder for [`Config`], so callers set only what they care about instead of
/// passing a row of positional flags.
#[derive(Debug, Clone)]
pub struct ConfigBuilder {
    interval_secs: f64,
    max_fish: usize,
    user: Option<String>,
    filter: Option<String>,
    kernel: bool,
    ascii: bool,
    screensaver: bool,
    feed: bool,
    seed: Option<u64>,
    dump: bool,
    no_mouse: bool,
    record: Option<std::path::PathBuf>,
    replay: Option<std::path::PathBuf>,
    kill: bool,
    theme_section: Option<ThemeSection>,
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self {
            interval_secs: 1.0,
            max_fish: 25,
            user: None,
            filter: None,
            kernel: false,
            ascii: false,
            screensaver: false,
            feed: false,
            seed: None,
            dump: false,
            no_mouse: false,
            record: None,
            replay: None,
            kill: false,
            theme_section: None,
        }
    }
}

impl ConfigBuilder {
    pub fn interval(mut self, secs: f64) -> Self {
        self.interval_secs = secs;
        self
    }

    pub fn max_fish(mut self, max_fish: usize) -> Self {
        self.max_fish = max_fish;
        self
    }

    pub fn user(mut self, user: Option<String>) -> Self {
        self.user = user;
        self
    }

    pub fn filter(mut self, filter: Option<String>) -> Self {
        self.filter = filter;
        self
    }

    pub fn kernel(mut self, kernel: bool) -> Self {
        self.kernel = kernel;
        self
    }

    pub fn ascii(mut self, ascii: bool) -> Self {
        self.ascii = ascii;
        self
    }

    pub fn screensaver(mut self, screensaver: bool) -> Self {
        self.screensaver = screensaver;
        self
    }

    pub fn feed(mut self, feed: bool) -> Self {
        self.feed = feed;
        self
    }

    pub fn seed(mut self, seed: Option<u64>) -> Self {
        self.seed = seed;
        self
    }

    pub fn dump(mut self, dump: bool) -> Self {
        self.dump = dump;
        self
    }

    pub fn no_mouse(mut self, no_mouse: bool) -> Self {
        self.no_mouse = no_mouse;
        self
    }

    pub fn record(mut self, record: Option<std::path::PathBuf>) -> Self {
        self.record = record;
        self
    }

    pub fn replay(mut self, replay: Option<std::path::PathBuf>) -> Self {
        self.replay = replay;
        self
    }

    pub fn kill(mut self, kill: bool) -> Self {
        self.kill = kill;
        self
    }

    /// Apply settings from a config file. Flags set afterwards override them.
    pub fn from_file(mut self, file: FileConfig) -> Self {
        if let Some(interval) = file.interval {
            self.interval_secs = interval;
        }
        if let Some(max_fish) = file.max_fish {
            self.max_fish = max_fish;
        }
        if let Some(user) = file.user {
            self.user = Some(user);
        }
        if let Some(filter) = file.filter {
            self.filter = Some(filter);
        }
        if let Some(kernel) = file.kernel {
            self.kernel = kernel;
        }
        if let Some(ascii) = file.ascii {
            self.ascii = ascii;
        }
        if let Some(feed) = file.feed {
            self.feed = feed;
        }
        self.theme_section = file.theme;
        self
    }

    /// Validate and build the config, with a clear error for anything invalid.
    pub fn build(self) -> Result<Config> {
        if !self.interval_secs.is_finite() || self.interval_secs <= 0.0 {
            bail!("interval must be a positive number of seconds");
        }
        let interval = Duration::try_from_secs_f64(self.interval_secs)
            .map_err(|_| anyhow::anyhow!("interval is too large"))?;
        if interval < sysinfo::MINIMUM_CPU_UPDATE_INTERVAL {
            bail!(
                "interval must be at least {} ms: sysinfo needs two samples close \
                 together before CPU numbers are meaningful",
                sysinfo::MINIMUM_CPU_UPDATE_INTERVAL.as_millis()
            );
        }
        if interval > Duration::from_secs(3600) {
            bail!("interval must be at most 3600 seconds");
        }
        if !(1..=500).contains(&self.max_fish) {
            bail!("max_fish must be between 1 and 500");
        }

        let filter = match self.filter {
            Some(pattern) => Some(
                Regex::new(&pattern)
                    .map_err(|e| anyhow::anyhow!("filter is not a valid regex: {e}"))?,
            ),
            None => None,
        };

        let mut theme = Theme::default();
        if let Some(section) = self.theme_section {
            theme.apply_section(section)?;
        }

        Ok(Config {
            interval,
            max_fish: self.max_fish,
            user: self.user.filter(|u| !u.is_empty()),
            filter,
            kernel: self.kernel,
            ascii: self.ascii,
            screensaver: self.screensaver,
            feed: self.feed,
            seed: self.seed,
            dump: self.dump,
            no_mouse: self.no_mouse,
            record: self.record,
            replay: self.replay,
            kill: self.kill,
            mono: std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty()),
            theme,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_sane() {
        let c = Config::builder().build().expect("valid");
        assert_eq!(c.max_fish, 25);
        assert_eq!(c.interval, Duration::from_secs(1));
        assert!(c.filter.is_none());
        assert!(!c.feed);
    }

    #[test]
    fn rejects_bad_intervals() {
        let bad = |secs| Config::builder().interval(secs).build().is_err();
        assert!(bad(0.0));
        assert!(bad(-1.0));
        assert!(bad(1e20), "must not panic on overflow");
        assert!(bad(f64::NAN));
        assert!(bad(0.1), "below the sysinfo minimum");
        assert!(bad(9999.0), "above the maximum");
        assert!(
            Config::builder()
                .interval(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL.as_secs_f64())
                .build()
                .is_ok()
        );
    }

    #[test]
    fn rejects_out_of_range_max_fish() {
        assert!(Config::builder().max_fish(0).build().is_err());
        assert!(Config::builder().max_fish(501).build().is_err());
    }

    #[test]
    fn rejects_bad_regex() {
        assert!(Config::builder().filter(Some("(".into())).build().is_err());
    }

    #[test]
    fn compiles_filter() {
        let c = Config::builder()
            .filter(Some("^fire.*".into()))
            .build()
            .expect("valid");
        assert!(c.filter.expect("filter").is_match("firefox"));
    }

    #[test]
    fn config_file_supplies_defaults() {
        let file = FileConfig::from_toml(
            r#"
            interval = 2.0
            max_fish = 40
            user = "alice"
            filter = "^rust"
            kernel = true
            ascii = true
            feed = true
            "#,
        )
        .expect("valid");
        let c = Config::builder().from_file(file).build().expect("valid");
        assert_eq!(c.interval, Duration::from_secs(2));
        assert_eq!(c.max_fish, 40);
        assert_eq!(c.user.as_deref(), Some("alice"));
        assert!(c.filter.expect("filter").is_match("rustc"));
        assert!(c.kernel);
        assert!(c.ascii);
        assert!(c.feed);
    }

    #[test]
    fn flags_override_the_config_file() {
        let file = FileConfig::from_toml("interval = 2.0\nmax_fish = 40").expect("valid");
        let c = Config::builder()
            .from_file(file)
            .interval(3.0)
            .max_fish(7)
            .build()
            .expect("valid");
        assert_eq!(c.interval, Duration::from_secs(3));
        assert_eq!(c.max_fish, 7);
    }

    #[test]
    fn config_file_theme_section_is_applied() {
        let file = FileConfig::from_toml(
            r##"
            [theme]
            palette = ["#010203"]

            [theme.sprites]
            crab = "CRAB"
            "##,
        )
        .expect("valid");
        let c = Config::builder().from_file(file).build().expect("valid");
        assert_eq!(c.theme.palette, vec![ratatui::style::Color::Rgb(1, 2, 3)]);
        assert_eq!(c.theme.sprites.crab, "CRAB");
    }

    #[test]
    fn config_file_rejects_unknown_keys_and_the_old_flat_theme() {
        assert!(FileConfig::from_toml("nonsense = 1").is_err());
        // The pre-1.0 flat theme format is gone: palette now lives under [theme].
        assert!(FileConfig::from_toml(r##"palette = ["#010203"]"##).is_err());
    }
}
