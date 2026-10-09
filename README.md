# procquarium

Your running processes, as fish.

procquarium is a terminal aquarium where every fish is a process on your machine. Big fish use a lot of memory, fast fish are burning CPU, and when a process exits its fish quietly floats to the surface. Leave it running in a spare pane or use it as a screensaver, and you'll start to recognise your machine's habits: the browser whale that never stops growing, the swarm of tiny shell fish that appear every time you run a build.

![procquarium: fish for processes, crabs for kernel threads, jellyfish for containers](https://raw.githubusercontent.com/ImanolGo/procquarium/main/demo.gif)

> **Status:** early days. The plan lives in [PLAN.md](PLAN.md) and things will move around a lot until 0.1.

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

### With a Rust toolchain

```sh
cargo install procquarium
```

### Arch Linux

The `PKGBUILD` lives in [`packaging/aur`](packaging/aur); an AUR package is
coming.

## Usage

```sh
procquarium                 # open the tank
procquarium --screensaver   # any key or mouse event exits
procquarium --user $USER    # only your own processes
procquarium --max-fish 120  # crowded tank
procquarium --filter '^rust' --interval 0.5
procquarium --feed           # press f to drop food
procquarium --seed 7        # the same tank every time
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
| `--config <PATH>` | Load a theme from a TOML file |
| `--seed <N>` | Fixed random seed, for a reproducible tank |

`procquarium --dump` (hidden) prints the process table as a plain table and
exits; it's handy for checking what the aquarium would see on another machine.

### Themes

Give the tank your own colours and sprites with a TOML file (`--config`, or
`$XDG_CONFIG_HOME/procquarium/config.toml`). Everything is optional; anything
you leave out keeps its default.

```toml
# Fish colours are picked from this list by process name.
palette = ["#ff6b6b", "#4ecdc4", "#ffe66d"]

[sprites]
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

Once a second procquarium takes a snapshot of the process table with [`sysinfo`](https://crates.io/crates/sysinfo), compares it with the previous one, and turns the differences into events: births, deaths, things getting hungrier. The tank itself runs at around 30 frames per second and draws with [`ratatui`](https://ratatui.rs). Fish ease towards their new size and speed rather than jumping, so the picture stays calm even when your machine isn't.

procquarium tries hard to be a polite guest. It should stay under a couple of percent of one core, and yes, it shows up in its own tank.

## Contributing

Issues and pull requests are welcome. If you have an idea for a new creature (a crab for kernel threads? a jellyfish for containers?), open an issue first so we can talk it through.

See [DEVELOPMENT.md](DEVELOPMENT.md) for the build stages, design notes and how the milestones fit together.

## License

MIT, see [LICENSE](LICENSE).
