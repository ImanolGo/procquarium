//! procquarium: your running processes, as fish.
//!
//! procquarium is a terminal aquarium where every fish is a process: size
//! follows resident memory, speed follows CPU, and colour follows the process
//! name. This library holds the pieces that are pure enough to reuse and test —
//! the process source and diffing, the tank simulation with its sprites and
//! mappings, the renderer, the configuration and the theme. The `procquarium`
//! binary is a thin shell around it that owns the terminal and the sampling
//! loop.
//!
//! # Stability
//!
//! The library exists for docs and tests; it has no stability guarantee. The
//! product is the `procquarium` binary, and only its command-line interface,
//! flags, configuration format and recording format are covered by the 1.0
//! compatibility promise. These modules are an implementation detail and may
//! change in any release.

#[doc(hidden)]
pub mod app;
#[doc(hidden)]
pub mod cli;
#[doc(hidden)]
pub mod config;
#[doc(hidden)]
pub mod diff;
#[doc(hidden)]
pub mod render;
#[doc(hidden)]
pub mod sampler;
#[doc(hidden)]
pub mod source;
#[doc(hidden)]
pub mod tank;
#[doc(hidden)]
pub mod theme;
