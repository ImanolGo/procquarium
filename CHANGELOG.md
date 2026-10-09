# Changelog

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
- crates.io publishing metadata and an Arch `PKGBUILD` under `packaging/aur`.

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

[Unreleased]: https://github.com/ImanolGo/procquarium/compare/v0.2.2...HEAD
[0.2.2]: https://github.com/ImanolGo/procquarium/releases/tag/v0.2.2
[0.2.1]: https://github.com/ImanolGo/procquarium/releases/tag/v0.2.1
[0.2.0]: https://github.com/ImanolGo/procquarium/releases/tag/v0.2.0
[0.1.0]: https://github.com/ImanolGo/procquarium/releases/tag/v0.1.0
