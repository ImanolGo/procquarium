# Development notes and stages

This document records how procquarium was built, milestone by milestone, and
the decisions taken along the way. The plan itself lives in [PLAN.md](PLAN.md);
this file is the after-the-fact log.

## Ground rules

Every commit in this history is meant to stand on its own:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

all pass at each stage. The simulation is free of I/O (only `main.rs` and
`source/sysinfo_source.rs` touch the system), `unwrap`/`expect` are confined to
tests and setup, and each stage carries the unit tests for what it introduces.

## The stages

| Stage | Commit subject | What it delivers |
| --- | --- | --- |
| M0 | project skeleton | Crate, dependencies, CI on Linux/macOS/Windows, and a terminal app that paints a blue background with a title and quits on `q`. |
| M1 | process source, diffing and `--dump` | `ProcessSource` (sysinfo + scripted `FakeSource`), `diff.rs` into `Spawned`/`Exited`/`Changed`, identity by `(pid, start_time)`, and a hidden `--dump` table. |
| M2 | static tank | Size/colour mapping, sprites, process selection (kernel/user/regex/top-N), a static `Fish`/`Tank`, `App`, and a ratatui render with a stable insta snapshot. |
| M3 | motion and decor | Wander, wall avoidance, easing of size/speed, parent schooling; surface, sand, seaweed and bubbles; `--seed`. |
| M4 | lifecycle | Eggs that hatch, deaths that float up and fade, zombie fish, fish that leave when they drop out of the top N; one sprite buffer reused across fish. |
| M5 | interaction and CLI | All flags and keys, the details box and labels, pause, selection, and screensaver mode. |
| M6 | polish and release prep | Tiny-terminal message, resize handling, the `without_tasks` performance fix, README/CHANGELOG, the vhs tape, and this document. |

The last four rows are where most of the feel of the program lives: M3 makes it
move, M4 makes it live, M5 makes it a usable tool.

## How this history was reconstructed

The program was written end-to-end first as a single change, then replayed as
the milestone commits above. During the replay each stage was rebuilt so the
tree actually compiles and its tests pass; the final stage restores the
finished tree. In other words the *content* is authentic to each stage, and the
ordering is a deliberate reconstruction rather than the literal order the code
was typed in.

Where a stage needed a module that conceptually belongs later, it was kept out
until its stage, even if that meant a small transitional concession. The one
worth calling out is the `diff` module in M1: nothing consumes it until the tank
arrives in M2, so M1 temporarily carries `#[allow(dead_code)]` (removed in M2).

## Notable decisions and deviations from PLAN.md

- **`ProcInfo` grew beyond the plan's sketch**: `start_time` (detects a reused
  PID) and `kernel` (the source, not the simulation, decides what a kernel
  thread is), plus `container` (cgroup heuristic), `io` (disk bytes per sample,
  for the I/O bubbles) and `run_time` (for the barnacles on old fish). All are
  computed in `source/sysinfo_source.rs`.
- **Threads are not processes.** `sysinfo`'s `ProcessRefreshKind::nothing()`
  still enables `tasks`, so on Linux every thread shows up as a process. M6 adds
  `.without_tasks()`; before that a busy machine put ~4× too many fish on
  screen. This is both a correctness fix and the single biggest performance win.
- **Deterministic diffing.** `diff` iterates PIDs in sorted order because
  `HashMap` iteration order is not stable; without this a seeded tank would not
  be reproducible.
- **Rendering reuses one sprite buffer.** `sprites::sprite_into` /
  `dead_sprite_into` write into a caller-owned `Vec<char>` so the render loop
  does not allocate per fish per frame.
- **Degenerate terminal sizes.** A fresh pty reports 0×0; the decor code guards
  its random ranges and there is a regression test for tiny sizes.
- **Fish roam the whole water column.** Each fish is given a slowly drifting
  `home_y` it steers towards. Without it, fish hatched on the sand, got one
  small upward impulse and then settled along the bottom instead of spreading
  through the tank.
- **The demo GIF is committed.** `demo.gif` is recorded from `demo.tape` with
  [vhs](https://github.com/charmbracelet/vhs) and the crate excludes it (see
  `Cargo.toml`) so it does not bloat the published package.

## Performance

Measured with a release build on a 425-process machine, draining a 120×40 pty:

- rendering alone: ~3.3% of one core (and roughly linear in terminal area;
  ~1.8% at 80×24);
- sampling once a second: ~25 ms per snapshot, since `sysinfo` reads
  `/proc/<pid>/{stat,statm,status}` for every process.

The plan's target of under 2% of one core is generous for a light machine but
tight here, because the cost is dominated by `sysinfo` scanning the process
table. Sampling now runs on a background thread (see below), so a slow sample
no longer stutters a frame, but the CPU cost is the same. If it needs to come
down further, the options are refreshing memory on alternate samples or adapting
the sample interval — neither changes the picture much.

## The review round (0.2.6–0.3.0)

A code review (kept out of the repo) listed seventeen items, fixed in order, one
commit each:

- **Bugs:** sprites mirrored correctly; selection reconciled against the
  snapshot with identity `(pid, start_time)`; mouse capture in screensaver mode;
  `--interval`/`--max-fish` validation; feeding capped and reversible.
- **Improvements:** sampling (and renice) moved to a background thread;
  hysteresis at the top-N cut-off; eggs mean "just started" again; `NO_COLOR`,
  a `Config::builder()` and dead-state removal.
- **New features:** click to select (`--no-mouse`), family highlight and a dotted
  line to the parent, a CPU sparkline, disk-I/O bubbles, `/` search,
  `--record`/`--replay`, barnacles on old fish, and `--kill` with confirmation.

The Windows build for the 0.2.6 tag failed because the errno helper referenced
the Unix-only `libc` crate; 0.3.0 carries the fix. That is why 0.2.6 was never
released on GitHub (and is yanked on crates.io).

## After 0.1

These were listed in PLAN.md under "Ideas for after 0.1" and built afterwards,
one commit each:

- **Day/night.** The source sums per-process CPU against the logical core count
  to get a 0..=1 load, and the renderer dims the water by it (capped so a busy
  box looks like dusk, not a black screen).
- **Crabs and jellyfish.** Kernel threads become crabs glued to the sand
  (shown with `--kernel`), and container processes become pulsing jellyfish.
  Kernel threads score near zero so they get reserved slots rather than
  competing for fish; container detection reads `/proc/<pid>/cgroup` on Linux
  and is cached per process, so it costs nothing after the first sample.
- **Feeding.** With `--feed`, `f` drops a pellet that sinks; a fish that reaches
  one eats it and glows, and its process gets a small `setpriority` nudge (via
  `libc`). Boosts are capped at two steps below the original niceness and are
  undone when the fish leaves or on exit; it only ever touches processes owned
  by you, and reports when the kernel refuses the change (`Denied`).
- **Themes.** `--config <PATH>` (or `$XDG_CONFIG_HOME/procquarium/config.toml`)
  loads a TOML palette and sprite overrides under a `[theme]` table. The renderer
  reads the palette and a `Sprites` set out of the config, so custom sprites and
  colours flow everywhere with no globals.
- **Config file.** The same file can set defaults (`interval`, `max_fish`,
  `user`, `filter`, `kernel`, `ascii`, `feed`) that the command-line flags
  override; `FileConfig` parses it and `ConfigBuilder::from_file` merges it.

## Releasing

A release is just a version tag:

1. Bump `version` in `Cargo.toml`, add a `CHANGELOG.md` entry, and commit.
2. `git tag -a vX.Y.Z -m "procquarium X.Y.Z"` and `git push origin vX.Y.Z`.

The tag drives the rest:

- **Release** (`dist`) builds the Linux/macOS/Windows archives with checksums
  and the shell/PowerShell/MSI installers, and creates the GitHub Release.
- **Debian package** builds the `.deb` and attaches it (it waits for the
  release to appear first).
- **Publish to crates.io** waits for the Release workflow to succeed, then
  publishes the crate through Trusted Publishing (OIDC), so a failed build never
  reaches crates.io. The one-time crates.io setup is documented in
  `.github/workflows/publish.yml`.
- **CI** runs fmt/clippy/tests on branch pushes, plus `cargo publish --dry-run`
  so a broken package is caught before the tag.

docs.rs builds once the crate is on crates.io.

## Running it

```sh
cargo run                 # the tank
cargo run -- --dump       # one snapshot as a table, then exit
cargo run -- --seed 7     # a reproducible tank
```
