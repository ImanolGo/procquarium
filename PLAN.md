# procquarium implementation plan

This document is written for a coding agent implementing procquarium from scratch. Work through the milestones in order. Each milestone ends with acceptance criteria; don't start the next one until they all pass, and commit at the end of each milestone.

Read the README first for what the finished program should feel like.

## Ground rules

- Language: Rust, stable toolchain, edition 2024.
- Before every commit: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` must pass.
- **Check crate APIs on docs.rs before using them.** The versions below were current in October 2026, and `sysinfo` in particular changes its API between minor versions. If this plan disagrees with the docs, the docs win; note the difference in your commit message.
- No `unwrap()` / `expect()` outside tests and `main` setup. Use `anyhow::Result` at the edges.
- Keep the simulation free of I/O so it can be unit tested. Only `main.rs` and `source/sysinfo_source.rs` touch the real system or the terminal.
- Don't add dependencies beyond the list below without a good reason written in the commit message.

## Dependencies

```toml
[dependencies]
ratatui = "0.30"
crossterm = "0.29"
sysinfo = "0.39"
clap = { version = "4", features = ["derive"] }
anyhow = "1"
rand = "0.9"
rand_chacha = "0.9"   # seedable RNG for reproducible tanks and tests
regex = "1"

[dev-dependencies]
insta = "1"
```

## Architecture

```
src/
  main.rs            CLI parsing, terminal setup/teardown, main loop
  app.rs             App state: tank + UI state (selected fish, labels on/off, paused)
  config.rs          Config struct built from CLI args
  source/
    mod.rs           ProcessSource trait, ProcInfo, Snapshot
    sysinfo_source.rs  real implementation backed by sysinfo
    fake.rs          scripted implementation for tests (behind cfg(test) or a feature)
  diff.rs            Snapshot + Snapshot -> Vec<ProcEvent>
  tank/
    mod.rs           Tank: owns fish, bubbles, plants, corpses; update(dt) and apply(events)
    fish.rs          Fish struct, motion, easing towards targets
    mapping.rs       pure functions: memory -> size, cpu -> speed, name -> colour
    sprites.rs       ASCII sprites per size class, left/right variants
    decor.rs         seaweed, sand, bubbles, surface waves
  render.rs          draws App into a ratatui Frame
```

### Core types

```rust
pub struct ProcInfo {
    pub pid: u32,
    pub parent: Option<u32>,
    pub name: String,
    pub cpu: f32,          // percent of one core, can exceed 100 on multi-core
    pub memory: u64,       // resident bytes
    pub status: ProcStatus, // Running, Sleeping, Zombie, Other
    pub user: Option<String>,
}

pub struct Snapshot { pub procs: HashMap<u32, ProcInfo> }

pub trait ProcessSource {
    fn snapshot(&mut self) -> anyhow::Result<Snapshot>;
}

pub enum ProcEvent {
    Spawned(ProcInfo),
    Exited { pid: u32 },
    Changed(ProcInfo),   // cpu/memory/status/parent changed
}
```

A process is identified by `(pid, start_time)` if the platform provides start time, so a reused PID is treated as a new process (an exit plus a spawn), not a change.

## Behaviour spec

**Sampling.** Take a snapshot once per second (configurable with `--interval`). sysinfo needs two refreshes separated by at least `sysinfo::MINIMUM_CPU_UPDATE_INTERVAL` before CPU numbers are meaningful, so do a warm-up refresh at startup and don't spawn fish until the second sample arrives (show a "filling the tank…" line meanwhile, which also looks nice).

**Which processes get a fish.** Score = `cpu + log2(memory in MiB + 1) * 2`. Show the top `--max-fish` (default 60) by score. Apply `--user` and `--filter <regex>` (on process name) before ranking. Kernel threads (Linux: no executable path and parent 2 or pid 2) are excluded unless `--kernel` is passed. A fish that drops out of the top N leaves by swimming off-screen, not by dying; dying is only for real exits.

**Mapping** (all in `mapping.rs`, all pure, all unit tested):
- Size class 0–3 from memory on a log scale: <16 MiB, <128 MiB, <1 GiB, ≥1 GiB.
- Target speed in cells/second: `2.0 + 18.0 * (cpu / 100).min(1.0).sqrt()`. Idle fish still drift slowly.
- Colour: hash the process name (FNV-1a or similar, stable across runs, not `DefaultHasher`) into a fixed palette of ~12 colours that read well on dark and light terminals.
- Fish change size and speed by easing towards the target over ~1 second, never instantly.

**Sprites** (`sprites.rs`). Right-facing versions below; left-facing versions are mirrored with bracket/arrow characters swapped, so write a `mirror()` function and test it.

```
0: ><>
1: ><(°>
2: ><((°>
3: ><(((°>
```

The eye character becomes `✕` for zombies and the whole sprite is drawn grey. Fall back to ASCII (`o`, `x`) when `--ascii` is passed.

**Motion.**
- Fish have a float position and velocity. Each frame: wander (small random steering), keep inside the water area (soft repulsion from the walls, never clip), face the direction of horizontal travel.
- Children school with their parent: if the parent has a fish, steer gently towards a point slightly behind it. Keep it to simple boids-style cohesion; no separation or alignment needed at first.
- Vertical movement is gentler than horizontal so fish don't look like they're bouncing.

**Lifecycle.**
- `Spawned`: an egg `°` appears on the sand, wobbles for ~1 s, then the fish hatches and swims up.
- `Exited`: the fish flips (sprite drawn upside down is hard in text, so just replace the eye with `✕` and remove the tail), stops swimming, and floats to the surface over ~3 s, then fades out (dim → gone).
- `Changed` with status Zombie: grey fish, eye `✕`, slow speed, stays in the tank until the zombie is reaped.

**Decor.** Surface line with slowly shifting `~`, sand row at the bottom, 3–6 strands of seaweed whose segments sway with a sine wave, bubbles that rise from random points and from fast-swimming fish. Decor density scales with terminal width.

**UI.**
- Keys: `q`/`Esc` quit, `Space` pause, `l` toggle labels (name under each fish, truncated to 10 chars), `Tab`/`Shift+Tab` cycle the selected fish, `+`/`-` adjust max fish by 10.
- The selected fish gets a highlight and a small info box in a corner: name, PID, parent, CPU %, memory (human readable), status.
- `--screensaver`: any key or mouse event exits, and no labels or info box.
- Resizing the terminal re-clamps all positions into the new bounds; a terminal smaller than 20×8 shows a "make me bigger" message instead of the tank.

**Main loop.** Fixed-ish timestep: poll crossterm events with a timeout of the remaining frame time (target 30 fps), update the tank with the real `dt`, render. Sampling happens on the same thread on its own timer; if a sample takes longer than ~50 ms on some machine, move it to a background thread with a channel. Use `ratatui::init()` / `ratatui::restore()` so a panic still restores the terminal.

**Performance.** procquarium itself should stay under 2% of one core with 60 fish on a typical laptop. Don't allocate per fish per frame; reuse buffers.

## Milestones

### M0: Project skeleton
- `cargo new --bin procquarium`, add dependencies, `.gitignore`, MIT license already in repo.
- GitHub Actions workflow running fmt, clippy and tests on ubuntu-latest, macos-latest and windows-latest.
- `main.rs` opens the alternate screen, draws a blue background and a "procquarium" title, and exits on `q`.

**Done when:** CI is green on all three OSes and the app opens and closes cleanly, leaving the terminal usable.

### M1: Process source and diffing
- Implement `ProcessSource`, `SysinfoSource`, `FakeSource` (returns a scripted list of snapshots).
- Implement `diff.rs` with tests covering: spawn, exit, change, PID reuse, parent change.
- Add a hidden `--dump` flag that prints one snapshot as a table and exits. Useful for debugging on other platforms.

**Done when:** `procquarium --dump` lists real processes with plausible CPU and memory, and diff tests pass.

### M2: Static tank
- Implement `mapping.rs`, `sprites.rs` and `render.rs`. Spawn one fish per process at a random position, no motion yet.
- Snapshot test with ratatui's `TestBackend` and `insta`, using `FakeSource` and a fixed RNG seed.

**Done when:** running it shows a tank full of correctly sized and coloured fish, and the snapshot test is stable across runs.

### M3: Motion and decor
- Wander, wall avoidance, facing direction, easing of size and speed, parent schooling.
- Seaweed, bubbles, surface, sand.
- `--seed <u64>` flag for reproducible tanks.

**Done when:** the tank looks alive, fish never leave the water or overlap the sand, and a 10-minute run shows no growth in memory use.

### M4: Lifecycle
- Eggs, death float, zombie fish, fish leaving when they drop out of the top N.
- Test with `FakeSource`: a scripted spawn produces an egg, then a fish; a scripted exit produces a floating corpse that is removed after it fades.
- Manual check on Linux: `sleep 30 &` hatches a fish; `kill %1` makes it float up. Make a zombie with a small C or Python script and confirm the grey fish.

**Done when:** lifecycle tests pass and the manual checks behave as described.

### M5: Interaction and CLI
- All keys from the UI section, info box, labels, `--screensaver`, `--user`, `--filter`, `--max-fish`, `--interval`, `--kernel`, `--ascii`.
- `--help` text written for humans, with an example or two.

**Done when:** every flag and key works, and `--help` reads well.

### M6: Polish and release prep
- Measure CPU use of procquarium itself; fix anything over budget.
- Tiny-terminal message, resize handling, light-terminal colour check.
- Record a short GIF with [vhs](https://github.com/charmbracelet/vhs) (`demo.tape` in the repo) and embed it at the top of the README, replacing the ASCII mock.
- Update the README so it matches what was actually built (flags, keys, platform notes).
- Add `CHANGELOG.md` and tag `v0.1.0`.

**Done when:** a fresh `cargo install --path .` works and the README is accurate.

## Ideas for after 0.1 (don't build these yet)

- Day/night: the water gets darker as system load rises.
- Crabs on the sand for kernel threads, jellyfish for container processes.
- Feeding: press `f` to drop food; fish you feed get a tiny priority boost (`renice`). Opt-in only, behind a flag, and never for processes the user doesn't own.
- A config file for custom palettes and sprites.
