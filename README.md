# procquarium

[![crates.io](https://img.shields.io/crates/v/procquarium.svg)](https://crates.io/crates/procquarium)
[![docs.rs](https://docs.rs/procquarium/badge.svg?v=2)](https://docs.rs/procquarium)
[![CI](https://github.com/ImanolGo/procquarium/actions/workflows/ci.yml/badge.svg)](https://github.com/ImanolGo/procquarium/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/ImanolGo/procquarium)](https://github.com/ImanolGo/procquarium/releases)
[![License: MIT](https://img.shields.io/crates/l/procquarium.svg)](LICENSE)

Your running processes, as fish.

procquarium is a terminal aquarium where every fish is a process on your machine. Big fish use a lot of memory, fast fish are burning CPU, and when a process exits its fish quietly floats to the surface. Leave it running in a spare pane or use it as a screensaver, and you'll start to recognise your machine's habits: the browser whale that never stops growing, the swarm of tiny shell fish that appear every time you run a build.

![procquarium: fish for processes, crabs for kernel threads, jellyfish for containers](https://raw.githubusercontent.com/ImanolGo/procquarium/main/demo.gif)

> **Status:** 0.3 — usable and still growing. The plan lives in [PLAN.md](PLAN.md); see [DEVELOPMENT.md](DEVELOPMENT.md) for how it is built.

> **Library:** The library exists for docs and tests; it has no stability guarantee. The product is the `procquarium` binary.

## What the fish mean

| In the tank | On your machine |
| --- | --- |
| Fish size | Resident memory (log scale, so one huge process doesn't dwarf everything) |
| Swimming speed | CPU usage |
| Colour | Process name, so the same program always looks the same |
| A school following a bigger fish | Child processes following their parent |
| An egg hatching on the sand | A process that just started |
| A belly-up fish drifting upwards | A process that just exited |
| A grey fish with ✕ eyes | A zombie process |
| A crab scuttling on the sand | A kernel thread (with `--kernel`) |
| A jellyfish pulsing | A process inside a container (Linux) |
| Water getting darker | Overall system load rising |
| A fish glowing after you feed it | You gave its process a small priority nudge (`--feed`) |
| A barnacle (`·`, then `:`) on a fish | A process that has been running for a day (then a week) |
| A fish trailing bubbles as it swims | A process doing disk I/O |

## Install

### Prebuilt binary

Each [release](https://github.com/ImanolGo/procquarium/releases) ships Linux,
macOS and Windows binaries plus installers:

```sh
# Linux / macOS
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/ImanolGo/procquarium/releases/latest/download/procquarium-installer.sh | sh
```

```powershell
# Windows (PowerShell)
powershell -ExecutionPolicy Bypass -c "irm https://github.com/ImanolGo/procquarium/releases/latest/download/procquarium-installer.ps1 | iex"
```

There is also a `.msi` for Windows and `.tar.xz` archives for each platform.

### Debian / Ubuntu

Each release includes an `amd64` `.deb` built by `cargo-deb`:

```sh
# download the procquarium_*.deb from the latest release, then:
sudo apt install ./procquarium_*.deb
```

### With a Rust toolchain

Requires Rust **1.95** or newer.

```sh
cargo install procquarium
```

## Usage

```sh
procquarium                 # open the tank
procquarium --screensaver   # any key or mouse event exits
procquarium --user $USER    # only your own processes
procquarium --max-fish 120  # crowded tank
procquarium --filter '^rust' --interval 0.5
procquarium --feed           # press f to drop food
procquarium --seed 7        # the same tank every time
procquarium --config tank.toml  # your own colours, sprites and defaults
procquarium --record run.jsonl   # save a session
procquarium --replay run.jsonl   # play it back later
```

Keys while it's running:

| Key | Does |
| --- | --- |
| `q` / `Esc` | Quit |
| `Space` | Pause the tank |
| `l` | Show or hide process names |
| `Tab` / `Shift+Tab` | Cycle through fish and show details (PID, CPU, memory) |
| `+` / `-` | More or fewer fish |
| `f` | Drop food (with `--feed`) |
| Click a fish | Select it (click empty water to clear) |
| `/` | Search for a process by name |
| `k` | Send SIGTERM to the selected process (with `--kill`) |

Click a fish to select it, or press `/` to search by name. The details box
shows the PID, CPU (with a one-row sparkline), memory and status, and a dotted
line links the selection to its parent, with children and parent highlighted.

Flags:

| Flag | Does |
| --- | --- |
| `--interval <SECS>` | Seconds between process samples (default 1) |
| `--max-fish <N>` | Maximum number of fish (default 25) |
| `--user <USER>` | Only show processes owned by this user |
| `--filter <REGEX>` | Only show processes whose name matches |
| `--kernel` | Include kernel threads (they appear as crabs) |
| `--ascii` | Use plain ASCII glyphs instead of the Unicode ones |
| `--screensaver` | Exit on any key or mouse event; no labels or info box |
| `--feed` | Let `f` drop food; fed fish get a small priority nudge |
| `--no-mouse` | Don't capture the mouse (keeps text selection working) |
| `--config <PATH>` | Load a config file: theme and defaults (see below) |
| `--record <PATH>` | Write each snapshot to a file as a JSON line |
| `--replay <PATH>` | Replay a recording instead of sampling |
| `--kill` | Allow `k` to send SIGTERM to the selected process (asks first; re-checks the pid just before signalling) |
| `--colors <MODE>` | Colour handling: `auto` (default), `truecolor`, `256` or `none` |
| `--seed <N>` | Fixed random seed, for a reproducible tank |

Colours adapt to the terminal: `auto` uses 24-bit RGB when `COLORTERM` is
`truecolor` or `24bit`, and otherwise snaps every colour to the nearest of the
256 xterm colours. `--colors 256` forces that mapping, `--colors truecolor`
forces RGB, and `--colors none` (or `NO_COLOR`) draws without colour.

`procquarium --dump` (hidden) prints the process table as a plain table and
exits; it's handy for checking what the aquarium would see on another machine.

### Configuration

Give the tank your own colours, sprites and defaults with a TOML file
(`--config`, or `$XDG_CONFIG_HOME/procquarium/config.toml`). Everything is
optional; anything you leave out keeps its default. Top-level keys are defaults
that the matching command-line flags override, and the theme lives under a
`[theme]` table.

```toml
# Defaults, overridden by the matching flags.
max_fish = 40
interval = 2.0
user = "you"
filter = "^rust"
kernel = true

[theme]
# Fish colours are picked from this list by process name.
palette = ["#ff6b6b", "#4ecdc4", "#ffe66d"]

[theme.sprites]
# Four fish, smallest to largest. The eye (o/°) becomes ✕ for zombies.
fish = ["><>", "><(°>", "><((°>", "><(((°>"]
fish_ascii = ["><>", "><(o>", "><((o>", "><(((o>"]
crab = "><°°><"
jellyfish = "~(°°)~"
```

A demo GIF is recorded from [`demo.tape`](demo.tape) with
[vhs](https://github.com/charmbracelet/vhs):

```sh
vhs demo.tape
```

### As a real screensaver

Inside tmux you can have it start automatically after a few minutes of inactivity:

```tmux
set -g lock-after-time 300
set -g lock-command "procquarium --screensaver"
```

## Platforms

Linux and macOS are the main targets. Windows should work, but zombie fish won't show up there because Windows doesn't really have zombie processes.

## How it works

Once a second procquarium takes a snapshot of the process table with [`sysinfo`](https://crates.io/crates/sysinfo), compares it with the previous one, and turns the differences into events: births, deaths, things getting hungrier. The tank itself runs at around 30 frames per second and draws with [`ratatui`](https://ratatui.rs). Sampling (and any renice or signal work) happens on a background thread, so a slow sample never stutters a frame. Fish ease towards their new size and speed rather than jumping, so the picture stays calm even when your machine isn't.

For demos and bug reports, `--record` writes every snapshot to a file as a line of JSON and `--replay` plays one back through the same pipeline. The file opens with a version line (`{"procquarium_recording": 1}`), and `--replay` refuses a recording from a newer format rather than guessing, so recordings stay replayable across 1.x releases.

On Unix, procquarium catches `SIGHUP`, `SIGTERM` and `SIGINT` and turns them into a normal shutdown, so any priority changes from `--feed` are undone and the terminal is restored cleanly — including when you simply close the terminal window. `SIGKILL` cannot be caught, so `kill -9` can leave a fed process boosted.

procquarium tries hard to be a polite guest. It should stay under a couple of percent of one core, and yes, it shows up in its own tank.

## Contributing

Issues and pull requests are welcome. If you have an idea for a new creature (a crab for kernel threads? a jellyfish for containers?), open an issue first so we can talk it through.

See [DEVELOPMENT.md](DEVELOPMENT.md) for the build stages, design notes and how the milestones fit together.

## License

MIT, see [LICENSE](LICENSE).
