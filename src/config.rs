//! Configuration derived from the command line.

use std::time::Duration;

use anyhow::{Result, bail};
use regex::Regex;

/// Everything the simulation needs to know about how the user wants to run.
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
}

impl Config {
    /// Build a config, validating the raw filter regex and interval.
    pub fn new(
        interval_secs: f64,
        max_fish: usize,
        user: Option<String>,
        filter: Option<String>,
        kernel: bool,
        ascii: bool,
    ) -> Result<Self> {
        if !interval_secs.is_finite() || interval_secs <= 0.0 {
            bail!("--interval must be a positive number of seconds");
        }
        let filter = match filter {
            Some(pattern) => Some(
                Regex::new(&pattern)
                    .map_err(|e| anyhow::anyhow!("--filter is not a valid regex: {e}"))?,
            ),
            None => None,
        };

        Ok(Self {
            interval: Duration::from_secs_f64(interval_secs),
            max_fish,
            user: user.filter(|u| !u.is_empty()),
            filter,
            kernel,
            ascii,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Result<Config> {
        Config::new(1.0, 60, None, None, false, false)
    }

    #[test]
    fn defaults_are_sane() {
        let c = base().expect("valid");
        assert_eq!(c.max_fish, 60);
        assert_eq!(c.interval, Duration::from_secs(1));
        assert!(c.filter.is_none());
    }

    #[test]
    fn rejects_non_positive_interval() {
        assert!(Config::new(0.0, 60, None, None, false, false).is_err());
        assert!(Config::new(-1.0, 60, None, None, false, false).is_err());
    }

    #[test]
    fn rejects_bad_regex() {
        assert!(Config::new(1.0, 60, None, Some("(".into()), false, false).is_err());
    }

    #[test]
    fn compiles_filter() {
        let c = Config::new(1.0, 60, None, Some("^fire.*".into()), false, false).expect("valid");
        assert!(c.filter.unwrap().is_match("firefox"));
    }
}
