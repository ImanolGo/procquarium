# Changelog

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- `--config` now loads a full config file: the theme lives under a `[theme]`
  table, and top-level keys (`interval`, `max_fish`, `user`, `filter`, `kernel`,
  `ascii`, `feed`) set defaults that command-line flags override. The old
  flat theme layout is no longer accepted.

## [0.3.2] - 2026-10-09

### Changed

- `--help` (and `-h`) now list the full set of usage examples and the key
  bindings, matching the README.

## [0.3.1] - 2026-10-09

### Changed

- README and DEVELOPMENT.md brought up to date with the 0.3 features, and a
  fresh demo GIF showing search, the details box and feeding.
- `0.2.6` is yanked on crates.io: its Windows binary never built (a Unix-only
  `libc` reference), and `0.3.0` supersedes it.

## [0.3.0] - 2026-10-09

### Added

- Click a fish to select it, and `--no-mouse` to keep text selection.
- Highlight the selected fish's family and draw a dotted line to its parent.
- A CPU sparkline in the details box.
- Bubbles from processes doing disk I/O, in proportion to their throughput.
- `/` to search for a process by name.
- `--record` and `--replay` to save and replay snapshots as JSON lines.
- Barnacles on long-running fish (`·` after a day, `:` after a week).
- `--kill` and `k` to send SIGTERM to the selected process, with confirmation
  and only for your own processes.

### Fixed

- Non-Unix builds: the errno helper no longer references the Unix-only `libc`
  crate, which had broken the Windows binary build for 0.2.6.

## [0.2.6] - 2026-10-09

### Fixed

- Left-facing fish are mirrored correctly: `mirror` now reverses the glyphs
  as well as swapping them.
- A selected process that enters the top N without a change of its own (a
  bigger one exited) now gets a fish instead of leaving an empty slot.
- Creatures are matched by `(pid, start_time)`, so a reused PID gets a fresh
  fish while the old one is still floating up.
- Mouse events reach `--screensaver` (the mouse is now captured), so moving or
  clicking the mouse exits as the README promises.
- `--interval` no longer panics on huge values and rejects values below
  sysinfo's minimum; `--max-fish` is limited to 1..=500.
- Feeding caps its priority nudge at two steps and restores the original nice
  value when a fish leaves or on exit.

### Changed

- Sampling (and renice) runs on a background thread, so a slow sample no longer
  hitches a frame or flashes a "slow sample" message.
- Hysteresis at the top-N cut-off stops fish churning when processes swap
  places around the boundary.
- Eggs mean "just started" again: the first sample places fish at depth, a
  promoted process swims in from a wall, and only real births lay eggs.
- `Config` is built with `Config::builder()` instead of ten positional
  arguments.

### Added

- `NO_COLOR` support.

## [0.2.5] - 2026-10-09

### Fixed

- Declare the actual MSRV: `rust-version = "1.95"` instead of 1.88. The
  dependency tree (every `sysinfo` 0.39.x) requires 1.95, so `cargo install`
  on an older toolchain failed with a confusing dependency error rather than a
  clear "needs rustc 1.95". Add an MSRV CI job so the claim stays honest.

## [0.2.4] - 2026-10-09

### Added

- A library target exposing the pure parts of the project (process source,
  diffing, tank simulation, rendering, config and theme), so the crate can be
  used as a dependency and docs.rs can build API documentation. Previously
  docs.rs failed with "no library targets found in package".

### Changed

- The `procquarium` binary is now a thin shell over the library.

## [0.2.3] - 2026-10-09

### Added

- Automatic crates.io publishing on version tags via Trusted Publishing, and
  crates.io / docs.rs / CI / release / license badges in the README.

### Changed

- Documentation: a "Releasing" section describing the tag-driven release flow.

## [0.2.2] - 2026-10-09

### Fixed

- The `.deb` is now built with `cargo-deb` metadata (concise package
  description, `$auto` dependency, docs and example config installed), so it
  no longer carries the whole README as its description. The Debian workflow
  runs automatically when a release is published.

## [0.2.1] - 2026-10-09

### Added

- Prebuilt binaries and installers (shell, PowerShell, MSI) for Linux, macOS
  and Windows, built by [dist](https://github.com/axodotdev/cargo-dist) on
  every version tag alongside source tarballs and checksums.
- crates.io publishing metadata.

## [0.2.0] - 2026-10-09

### Added

- Day/night: the water gets darker as overall system load rises.
- Crabs scuttle along the sand for kernel threads (shown with `--kernel`), and
  container processes appear as pulsing jellyfish.
- `--feed`: press `f` to drop food; fish that eat get a small, opt-in priority
  nudge (`setpriority`), only ever for processes you own.
- `--config <PATH>` (or `$XDG_CONFIG_HOME/procquarium/config.toml`) for custom
  palettes and sprites, with an example file in `procquarium.example.toml`.
- A recorded demo GIF at the top of the README (`demo.gif`, `demo.tape`).

### Changed

- Process names (labels) now shown by default; `l` still toggles them.
- Default `--max-fish` lowered from 60 to 25 for a calmer tank.
- Fish spread through the whole water column instead of settling on the sand.

### Fixed

- Threads are no longer mistaken for processes: `sysinfo`'s
  `ProcessRefreshKind::nothing()` enables tasks by default, so on Linux every
  thread showed up as a fish. One fish is now one process, which also cut
  sampling CPU by roughly four times.
- Guard against degenerate terminal sizes (a fresh pty reports 0×0), which
  could panic the decor.

## [0.1.0] - 2026-10-09

### Added

- A terminal aquarium where every fish is a process: size follows resident
  memory, speed follows CPU, colour follows the process name.
- One sample per second (configurable with `--interval`), diffed into spawn,
  exit and change events.
- Fish lifecycle: eggs hatch on the sand, exiting processes float to the
  surface and fade, zombies swim slowly in grey with `✕` eyes, and fish that
  drop out of the top N swim off screen.
- Motion with wandering, wall avoidance, facing direction and parent schooling.
- Decor: surface waves, sand, swaying seaweed and rising bubbles.
- Interactive UI: pause, labels, fish selection with an info box, and `+`/`-`
  to change the fish budget.
- Screensaver mode, `--user`/`--filter`/`--kernel` filters, ASCII fallback and
  a `--seed` flag for reproducible tanks.
- A hidden `--dump` flag that prints the process table as a table.
- CI on Linux, macOS and Windows.

[Unreleased]: https://github.com/ImanolGo/procquarium/compare/v0.3.2...HEAD
[0.3.2]: https://github.com/ImanolGo/procquarium/releases/tag/v0.3.2
[0.3.1]: https://github.com/ImanolGo/procquarium/releases/tag/v0.3.1
[0.3.0]: https://github.com/ImanolGo/procquarium/releases/tag/v0.3.0
[0.2.6]: https://github.com/ImanolGo/procquarium/releases/tag/v0.2.6
[0.2.5]: https://github.com/ImanolGo/procquarium/releases/tag/v0.2.5
[0.2.4]: https://github.com/ImanolGo/procquarium/releases/tag/v0.2.4
[0.2.3]: https://github.com/ImanolGo/procquarium/releases/tag/v0.2.3
[0.2.2]: https://github.com/ImanolGo/procquarium/releases/tag/v0.2.2
[0.2.1]: https://github.com/ImanolGo/procquarium/releases/tag/v0.2.1
[0.2.0]: https://github.com/ImanolGo/procquarium/releases/tag/v0.2.0
[0.1.0]: https://github.com/ImanolGo/procquarium/releases/tag/v0.1.0
