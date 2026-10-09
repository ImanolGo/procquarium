# Changelog

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/ImanolGo/procquarium/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/ImanolGo/procquarium/releases/tag/v0.1.0
