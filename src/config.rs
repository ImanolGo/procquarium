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
    /// Colours and sprites, optionally loaded from a config file.
    pub theme: Theme,
}

impl Config {
    /// Build a config, validating the raw filter regex and interval.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
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
    ) -> Result<Self> {
        if !interval_secs.is_finite() || interval_secs <= 0.0 {
            bail!("--interval must be a positive number of seconds");
        }
        let interval = Duration::try_from_secs_f64(interval_secs)
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
        if !(1..=500).contains(&max_fish) {
            bail!("--max-fish must be between 1 and 500");
        }

        let filter = match filter {
            Some(pattern) => Some(
                Regex::new(&pattern)
                    .map_err(|e| anyhow::anyhow!("--filter is not a valid regex: {e}"))?,
            ),
            None => None,
        };

        Ok(Self {
            interval,
            max_fish,
            user: user.filter(|u| !u.is_empty()),
            filter,
            kernel,
            ascii,
            screensaver,
            feed,
            seed,
            dump,
            theme: Theme::default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A valid config with everything off.
    fn config() -> Config {
        Config::new(1.0, 60, None, None, false, false, false, false, None, false).expect("valid")
    }

    #[test]
    fn defaults_are_sane() {
        let c = config();
        assert_eq!(c.max_fish, 60);
        assert_eq!(c.interval, Duration::from_secs(1));
        assert!(c.filter.is_none());
        assert!(!c.feed);
    }

    #[test]
    fn rejects_bad_intervals() {
        let bad = |secs| {
            Config::new(
                secs, 60, None, None, false, false, false, false, None, false,
            )
            .is_err()
        };
        assert!(bad(0.0));
        assert!(bad(-1.0));
        assert!(bad(1e20), "must not panic on overflow");
        assert!(bad(f64::NAN));
        assert!(bad(0.1), "below the sysinfo minimum");
        assert!(bad(9999.0), "above the maximum");
        // The sysinfo minimum itself is accepted.
        assert!(
            Config::new(
                sysinfo::MINIMUM_CPU_UPDATE_INTERVAL.as_secs_f64(),
                60,
                None,
                None,
                false,
                false,
                false,
                false,
                None,
                false,
            )
            .is_ok()
        );
    }

    #[test]
    fn rejects_out_of_range_max_fish() {
        let bad =
            |n| Config::new(1.0, n, None, None, false, false, false, false, None, false).is_err();
        assert!(bad(0));
        assert!(bad(501));
    }

    #[test]
    fn rejects_bad_regex() {
        let err = Config::new(
            1.0,
            60,
            None,
            Some("(".into()),
            false,
            false,
            false,
            false,
            None,
            false,
        );
        assert!(err.is_err());
    }

    #[test]
    fn compiles_filter() {
        let c = Config::new(
            1.0,
            60,
            None,
            Some("^fire.*".into()),
            false,
            false,
            false,
            false,
            None,
            false,
        )
        .expect("valid");
        assert!(c.filter.unwrap().is_match("firefox"));
    }
}
