//! procquarium: your running processes, as fish.
//!
//! This is the binary crate; the reusable pieces live in the library.

use std::time::{Duration, Instant};

use anyhow::Result;
use clap::Parser;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    MouseButton, MouseEventKind,
};
use crossterm::execute;
use rand::Rng;

use procquarium::app::App;
use procquarium::config::Config;
use procquarium::render::{self, human_bytes};
use procquarium::sampler;
use procquarium::source::record;
use procquarium::source::sysinfo_source::SysinfoSource;
use procquarium::source::{PriorityBoost, ProcStatus, ProcessSource, Snapshot};
use procquarium::theme;

const AFTER_HELP: &str = "\
Examples:
  procquarium                      open the tank
  procquarium --screensaver        any key or mouse event exits
  procquarium --user $USER         only your own processes
  procquarium --max-fish 120       a crowded tank
  procquarium --filter '^rust' --interval 0.5
  procquarium --feed               drop food with f
  procquarium --seed 7             the same tank every time
  procquarium --config theme.toml  your own colours and sprites
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
struct Cli {
    /// Seconds between process samples.
    #[arg(long, value_name = "SECS", default_value_t = 1.0)]
    interval: f64,

    /// Maximum number of fish in the tank.
    #[arg(
        long,
        value_name = "N",
        default_value_t = 25,
        value_parser = clap::builder::RangedU64ValueParser::<usize>::new().range(1..=500)
    )]
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

    /// Let `f` drop food; fish that eat get a small priority nudge (needs
    /// privileges to raise priority, and only ever touches your own processes).
    #[arg(long)]
    feed: bool,

    /// Fixed random seed, for a reproducible tank.
    #[arg(long, value_name = "N")]
    seed: Option<u64>,

    /// Path to a TOML theme file (palette and sprites).
    #[arg(long, value_name = "PATH")]
    config: Option<std::path::PathBuf>,

    /// Don't capture the mouse, so text selection keeps working.
    #[arg(long)]
    no_mouse: bool,

    /// Write every snapshot as a JSON line to this file.
    #[arg(long, value_name = "PATH")]
    record: Option<std::path::PathBuf>,

    /// Replay snapshots from a recording instead of sampling.
    #[arg(long, value_name = "PATH")]
    replay: Option<std::path::PathBuf>,

    /// Allow `k` to send SIGTERM to the selected process (asks first).
    #[arg(long)]
    kill: bool,

    /// Print one snapshot as a table and exit.
    #[arg(long, hide = true)]
    dump: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut config = Config::builder()
        .interval(cli.interval)
        .max_fish(cli.max_fish)
        .user(cli.user)
        .filter(cli.filter)
        .kernel(cli.kernel)
        .ascii(cli.ascii)
        .screensaver(cli.screensaver)
        .feed(cli.feed)
        .seed(cli.seed)
        .dump(cli.dump)
        .no_mouse(cli.no_mouse)
        .record(cli.record)
        .replay(cli.replay)
        .kill(cli.kill)
        .build()?;

    if config.dump {
        // Give the CPU counters a moment to become meaningful.
        let mut source = SysinfoSource::new();
        std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
        let snapshot = source.snapshot()?;
        print_dump(&snapshot);
        return Ok(());
    }

    config.theme = theme::Theme::load(cli.config.as_deref())?;

    let seed = config.seed.unwrap_or_else(|| rand::rng().random());
    let mut terminal = ratatui::init();

    // Capture the mouse so clicks select fish. Turned off with --no-mouse (to
    // keep text selection) and always on in screensaver mode, where any mouse
    // event exits. Release it on the way out (including a panic).
    let mouse = config.screensaver || !config.no_mouse;
    if mouse {
        let _ = execute!(std::io::stdout(), EnableMouseCapture);
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let _ = execute!(std::io::stdout(), DisableMouseCapture);
            previous(info);
        }));
    }

    let result = run(&mut terminal, config, seed);

    if mouse {
        let _ = execute!(std::io::stdout(), DisableMouseCapture);
    }
    ratatui::restore();
    result
}

fn run(terminal: &mut ratatui::DefaultTerminal, config: Config, seed: u64) -> Result<()> {
    let interval = config.interval;
    let recorder = match &config.record {
        Some(path) => Some(record::Recorder::create(path)?),
        None => None,
    };
    // Dropping the sampler stops its thread and restores any priorities it set.
    let sampler = match &config.replay {
        Some(path) => sampler::Sampler::spawn_with_recorder(
            interval,
            record::ReplaySource::open(path)?,
            recorder,
        )?,
        None => sampler::Sampler::spawn_with_recorder(interval, SysinfoSource::new(), recorder)?,
    };

    let size = terminal.size()?;
    let mut app = App::new(config, size.width, size.height, seed);
    let screensaver = app.config.screensaver;
    let frame_target = Duration::from_millis(33);

    let mut last_frame = Instant::now();
    // Only warn once per run when the kernel refuses to change priorities.
    let mut renice_denied = false;

    loop {
        // Take whatever the sampling thread has produced since last frame.
        while let Some(ev) = sampler.try_recv() {
            match ev {
                sampler::Event::Snapshot(snapshot) => app.apply_snapshot(snapshot),
                sampler::Event::Boost(PriorityBoost::Denied) if !renice_denied => {
                    renice_denied = true;
                    app.status =
                        Some("could not renice (need privileges); feeding has no effect".into());
                }
                sampler::Event::Boost(_) => {}
                sampler::Event::Error(error) => {
                    app.status = Some(format!("sample error: {error}"));
                }
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

        // Hand priority work to the sampling thread so the frame never blocks.
        for id in app.take_pending_boosts() {
            sampler.boost(id);
        }
        for id in app.take_pending_restores() {
            sampler.restore(id);
        }
        for pid in app.take_pending_kills() {
            sampler.kill(pid);
        }

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
            // While the search line is open, keys edit the query.
            if app.search_query().is_some() {
                match key.code {
                    KeyCode::Esc => app.search_cancel(),
                    KeyCode::Enter => app.search_commit(),
                    KeyCode::Backspace => app.search_backspace(),
                    KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                        app.search_push(c);
                    }
                    _ => {}
                }
                return false;
            }
            // A pending kill asks a yes/no question.
            if app.kill_prompt().is_some() {
                match key.code {
                    KeyCode::Char('y') => app.confirm_kill(),
                    KeyCode::Esc | KeyCode::Char('n') => app.cancel_kill(),
                    _ => {}
                }
                return false;
            }
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => return true,
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    return true;
                }
                KeyCode::Char(' ') => app.paused = !app.paused,
                KeyCode::Char('l') => app.show_labels = !app.show_labels,
                KeyCode::Char('f') => app.drop_food(),
                KeyCode::Char('/') => app.start_search(),
                KeyCode::Char('k') => app.start_kill(),
                KeyCode::Tab => app.select_next(key.modifiers.contains(KeyModifiers::SHIFT)),
                KeyCode::BackTab => app.select_next(true),
                KeyCode::Char('+') | KeyCode::Char('=') => app.adjust_max_fish(10),
                KeyCode::Char('-') | KeyCode::Char('_') => app.adjust_max_fish(-10),
                _ => {}
            }
        }
        Event::Resize(width, height) => app.resize(width, height),
        Event::Mouse(mouse) => {
            if screensaver {
                return true;
            }
            if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
                app.select_at(mouse.column, mouse.row);
            }
        }
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
