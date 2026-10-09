//! Configuration derived from the command line.

use std::time::Duration;

use anyhow::{Result, bail};
use regex::Regex;

use crate::theme::Theme;

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
    /// Draw in the terminal's default colours (`NO_COLOR`).
    pub mono: bool,
    /// Colours and sprites, optionally loaded from a config file.
    pub theme: Theme,
}

impl Config {
    /// Start building a config; every field has a sensible default.
    pub fn builder() -> ConfigBuilder {
        ConfigBuilder::default()
    }
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

    /// Validate and build the config, with a clear error for anything invalid.
    pub fn build(self) -> Result<Config> {
        if !self.interval_secs.is_finite() || self.interval_secs <= 0.0 {
            bail!("--interval must be a positive number of seconds");
        }
        let interval = Duration::try_from_secs_f64(self.interval_secs)
            .map_err(|_| anyhow::anyhow!("--interval is too large"))?;
        if interval < sysinfo::MINIMUM_CPU_UPDATE_INTERVAL {
            bail!(
                "--interval must be at least {} ms: sysinfo needs two samples close \
                 together before CPU numbers are meaningful",
                sysinfo::MINIMUM_CPU_UPDATE_INTERVAL.as_millis()
            );
        }
        if interval > Duration::from_secs(3600) {
            bail!("--interval must be at most 3600 seconds");
        }
        if !(1..=500).contains(&self.max_fish) {
            bail!("--max-fish must be between 1 and 500");
        }

        let filter = match self.filter {
            Some(pattern) => Some(
                Regex::new(&pattern)
                    .map_err(|e| anyhow::anyhow!("--filter is not a valid regex: {e}"))?,
            ),
            None => None,
        };

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
            mono: std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty()),
            theme: Theme::default(),
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
}
