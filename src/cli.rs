//! The command-line interface, shared by the binary and the docs generator.

use clap::Parser;

use crate::config::ColorMode;

/// The help text shown after the options.
pub const AFTER_HELP: &str = "\
Examples:
  procquarium                      open the tank
  procquarium --screensaver        any key or mouse event exits
  procquarium --user $USER         only your own processes
  procquarium --max-fish 120       a crowded tank
  procquarium --filter '^rust' --interval 0.5
  procquarium --feed               drop food with f
  procquarium --seed 7             the same tank every time
  procquarium --config tank.toml   your own colours, sprites and defaults
  procquarium --record run.jsonl   save a session
  procquarium --replay run.jsonl   play it back later

Keys while running:
  q / Esc          quit
  Space            pause the tank
  l                show or hide process names
  Tab / Shift+Tab  cycle fish and show details (PID, CPU, memory)
  + / -            more or fewer fish
  f                drop food (with --feed)
  /                search for a process by name
  k                send SIGTERM to the selected process (with --kill)
  click a fish     select it (click empty water to clear)";

/// The parsed command line.
#[derive(Parser, Debug)]
#[command(
    name = "procquarium",
    version,
    about = "Your running processes, as fish.",
    long_about = "A terminal aquarium where every fish is a process on your machine. \
Big fish use a lot of memory, fast fish are burning CPU, and when a process exits \
its fish quietly floats to the surface. Leave it running in a spare pane, or use \
it as a screensaver.",
    after_help = AFTER_HELP
)]
pub struct Cli {
    /// Seconds between process samples.
    #[arg(long, value_name = "SECS")]
    pub interval: Option<f64>,

    /// Maximum number of fish in the tank.
    #[arg(
        long,
        value_name = "N",
        value_parser = clap::builder::RangedU64ValueParser::<usize>::new().range(1..=500)
    )]
    pub max_fish: Option<usize>,

    /// Only show processes owned by this user.
    #[arg(long, value_name = "USER")]
    pub user: Option<String>,

    /// Only show processes whose name matches this regex.
    #[arg(long, value_name = "REGEX")]
    pub filter: Option<String>,

    /// Include kernel threads.
    #[arg(long)]
    pub kernel: bool,

    /// Use plain ASCII glyphs instead of the Unicode ones.
    #[arg(long)]
    pub ascii: bool,

    /// Exit on any key or mouse event; hide labels and the info box.
    #[arg(long)]
    pub screensaver: bool,

    /// Let `f` drop food; fish that eat get a small priority nudge (needs
    /// privileges to raise priority, and only ever touches your own processes).
    #[arg(long)]
    pub feed: bool,

    /// Fixed random seed, for a reproducible tank.
    #[arg(long, value_name = "N")]
    pub seed: Option<u64>,

    /// Path to a TOML config file (theme and defaults).
    #[arg(long, value_name = "PATH")]
    pub config: Option<std::path::PathBuf>,

    /// Don't capture the mouse, so text selection keeps working.
    #[arg(long)]
    pub no_mouse: bool,

    /// Write every snapshot as a JSON line to this file.
    #[arg(long, value_name = "PATH")]
    pub record: Option<std::path::PathBuf>,

    /// Replay snapshots from a recording instead of sampling.
    #[arg(long, value_name = "PATH")]
    pub replay: Option<std::path::PathBuf>,

    /// Allow `k` to send SIGTERM to the selected process (asks first).
    #[arg(long)]
    pub kill: bool,

    /// Colour handling: auto (default), truecolor, 256 or none.
    #[arg(long, value_name = "MODE", value_enum)]
    pub colors: Option<ColorMode>,

    /// Print one snapshot as a table and exit.
    #[arg(long, hide = true)]
    pub dump: bool,
}
