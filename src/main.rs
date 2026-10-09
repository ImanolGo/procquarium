//! procquarium: your running processes, as fish.

mod app;
mod config;
mod diff;
mod render;
mod source;
mod tank;

use std::time::{Duration, Instant};

use anyhow::Result;
use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use rand::Rng;

use crate::app::App;
use crate::config::Config;
use crate::render::human_bytes;
use crate::source::sysinfo_source::SysinfoSource;
use crate::source::{ProcStatus, ProcessSource, Snapshot};

const EXAMPLES: &str = "\
Examples:
  procquarium                 open the tank
  procquarium --screensaver   any key or mouse event exits
  procquarium --user $USER    only your own processes
  procquarium --max-fish 120  a crowded tank";

#[derive(Parser, Debug)]
#[command(
    name = "procquarium",
    version,
    about = "Your running processes, as fish.",
    long_about = "A terminal aquarium where every fish is a process on your machine. \
Big fish use a lot of memory, fast fish are burning CPU, and when a process exits \
its fish quietly floats to the surface. Leave it running in a spare pane, or use \
it as a screensaver.",
    after_help = EXAMPLES
)]
struct Cli {
    /// Seconds between process samples.
    #[arg(long, value_name = "SECS", default_value_t = 1.0)]
    interval: f64,

    /// Maximum number of fish in the tank.
    #[arg(long, value_name = "N", default_value_t = 60)]
    max_fish: usize,

    /// Only show processes owned by this user.
    #[arg(long, value_name = "USER")]
    user: Option<String>,

    /// Only show processes whose name matches this regex.
    #[arg(long, value_name = "REGEX")]
    filter: Option<String>,

    /// Include kernel threads.
    #[arg(long)]
    kernel: bool,

    /// Use plain ASCII glyphs instead of the Unicode ones.
    #[arg(long)]
    ascii: bool,

    /// Exit on any key or mouse event; hide labels and the info box.
    #[arg(long)]
    screensaver: bool,

    /// Fixed random seed, for a reproducible tank.
    #[arg(long, value_name = "N")]
    seed: Option<u64>,

    /// Print one snapshot as a table and exit.
    #[arg(long, hide = true)]
    dump: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = Config::new(
        cli.interval,
        cli.max_fish,
        cli.user,
        cli.filter,
        cli.kernel,
        cli.ascii,
        cli.screensaver,
        cli.seed,
        cli.dump,
    )?;

    let mut source = SysinfoSource::new();

    if config.dump {
        // Give the CPU counters a moment to become meaningful.
        std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
        let snapshot = source.snapshot()?;
        print_dump(&snapshot);
        return Ok(());
    }

    let seed = config.seed.unwrap_or_else(|| rand::rng().random());
    let mut terminal = ratatui::init();
    let result = run(&mut terminal, source, config, seed);
    ratatui::restore();
    result
}

fn run(
    terminal: &mut ratatui::DefaultTerminal,
    mut source: SysinfoSource,
    config: Config,
    seed: u64,
) -> Result<()> {
    let size = terminal.size()?;
    let mut app = App::new(config, size.width, size.height, seed);
    let interval = app.config.interval;
    let screensaver = app.config.screensaver;
    let frame_target = Duration::from_millis(33);

    let mut last_frame = Instant::now();
    let mut last_sample = Instant::now();

    loop {
        // Sample the process table on its own timer.
        if last_sample.elapsed() >= interval {
            let started = Instant::now();
            match source.snapshot() {
                Ok(snapshot) => app.apply_snapshot(snapshot),
                Err(error) => app.status = Some(format!("sample error: {error}")),
            }
            last_sample = Instant::now();
            let took = started.elapsed();
            if took > Duration::from_millis(50) {
                // Recorded rather than logged: a slow sample is worth watching
                // but must not interrupt the picture.
                app.status = Some(format!("slow sample: {} ms", took.as_millis()));
            }
        }

        // Drain input, waiting at most until the next frame is due.
        let timeout = frame_target.saturating_sub(last_frame.elapsed());
        loop {
            if !event::poll(timeout)? {
                break;
            }
            if handle_event(&mut app, event::read()?, screensaver) {
                return Ok(());
            }
            // After the first event, don't wait for more.
            if !event::poll(Duration::ZERO)? {
                break;
            }
        }

        let now = Instant::now();
        let dt = now.duration_since(last_frame).as_secs_f32().min(0.1);
        last_frame = now;
        app.update(dt);

        terminal.draw(|frame| render::draw(frame, &app))?;
    }
}

/// Returns true when the app should quit.
fn handle_event(app: &mut App, event: Event, screensaver: bool) -> bool {
    match event {
        Event::Key(key) => {
            if key.kind != KeyEventKind::Press {
                return false;
            }
            if screensaver {
                return true;
            }
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => return true,
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    return true;
                }
                KeyCode::Char(' ') => app.paused = !app.paused,
                KeyCode::Char('l') => app.show_labels = !app.show_labels,
                KeyCode::Tab => app.select_next(key.modifiers.contains(KeyModifiers::SHIFT)),
                KeyCode::BackTab => app.select_next(true),
                KeyCode::Char('+') | KeyCode::Char('=') => app.adjust_max_fish(10),
                KeyCode::Char('-') | KeyCode::Char('_') => app.adjust_max_fish(-10),
                _ => {}
            }
        }
        Event::Resize(width, height) => app.resize(width, height),
        Event::Mouse(_) if screensaver => return true,
        _ => {}
    }
    false
}

fn print_dump(snapshot: &Snapshot) {
    let mut procs: Vec<_> = snapshot.procs.values().collect();
    procs.sort_by(|a, b| {
        b.score()
            .partial_cmp(&a.score())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    println!(
        "{:>7}  {:>7}  {:<24}  {:>6}  {:>10}  {:<8}  USER",
        "PID", "PPID", "NAME", "CPU%", "MEM", "STATUS"
    );
    for p in procs {
        let parent = p
            .parent
            .map(|v| v.to_string())
            .unwrap_or_else(|| "-".into());
        let user = p.user.clone().unwrap_or_else(|| "-".into());
        let status = match p.status {
            ProcStatus::Running => "running",
            ProcStatus::Sleeping => "sleeping",
            ProcStatus::Zombie => "zombie",
            ProcStatus::Other => "other",
        };
        println!(
            "{:>7}  {:>7}  {:<24}  {:>6.1}  {:>10}  {:<8}  {}",
            p.pid,
            parent,
            truncate(&p.name, 24),
            p.cpu,
            human_bytes(p.memory),
            status,
            user
        );
    }
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        text.chars().take(max.saturating_sub(1)).collect::<String>() + "…"
    }
}
