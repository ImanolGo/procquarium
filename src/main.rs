//! procquarium: your running processes, as fish.

mod app;
mod config;
mod diff;
mod render;
mod source;
mod tank;

use std::time::{Duration, Instant};

use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use rand::Rng;

use crate::app::App;
use crate::config::Config;
use crate::render::human_bytes;
use crate::source::sysinfo_source::SysinfoSource;
use crate::source::{ProcStatus, ProcessSource, Snapshot};

#[derive(Parser, Debug)]
#[command(
    name = "procquarium",
    version,
    about = "Your running processes, as fish."
)]
struct Cli {
    /// Print one snapshot as a table and exit.
    #[arg(long, hide = true)]
    dump: bool,

    /// Fixed random seed, for a reproducible tank.
    #[arg(long, value_name = "N")]
    seed: Option<u64>,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let mut source = SysinfoSource::new();

    if cli.dump {
        // Give the CPU counters a moment to become meaningful.
        std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
        let snapshot = source.snapshot()?;
        print_dump(&snapshot);
        return Ok(());
    }

    let config = Config::new(1.0, 60, None, None, false, false, cli.seed)?;
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
) -> anyhow::Result<()> {
    let size = terminal.size()?;
    let mut app = App::new(config, size.width, size.height, seed);
    let interval = app.config.interval;
    let frame_target = Duration::from_millis(33);
    let mut last_sample = Instant::now();
    let mut last_frame = Instant::now();

    loop {
        // Sample the process table on its own timer.
        if last_sample.elapsed() >= interval {
            app.apply_snapshot(source.snapshot()?);
            last_sample = Instant::now();
        }

        // Drain input, waiting at most until the next frame is due.
        loop {
            if !event::poll(frame_target)? {
                break;
            }
            if should_quit(&event::read()?) {
                return Ok(());
            }
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

fn should_quit(event: &Event) -> bool {
    if let Event::Key(key) = event
        && key.kind == KeyEventKind::Press
    {
        return matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
            || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL));
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
