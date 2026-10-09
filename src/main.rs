//! procquarium: your running processes, as fish.

// The diff is only wired into the tank from M2 onwards; keep the module (and its
// tests) here so M1 stands on its own without dead-code warnings.
#[allow(dead_code)]
mod diff;
mod source;

use std::io;

use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::Frame;
use ratatui::style::{Color, Style};
use ratatui::widgets::Block;

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

    let mut terminal = ratatui::init();
    let result = run(&mut terminal);
    ratatui::restore();
    result?;
    Ok(())
}

fn run(terminal: &mut ratatui::DefaultTerminal) -> io::Result<()> {
    loop {
        terminal.draw(draw)?;
        if let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
            && matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
        {
            return Ok(());
        }
    }
}

fn draw(frame: &mut Frame) {
    let area = frame.area();
    frame.render_widget(
        Block::bordered()
            .style(Style::default().bg(Color::Blue))
            .title("procquarium"),
        area,
    );
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

fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}
