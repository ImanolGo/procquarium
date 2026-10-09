//! procquarium: your running processes, as fish.
//!
//! procquarium is a terminal aquarium where every fish is a process: size
//! follows resident memory, speed follows CPU, and colour follows the process
//! name. This library holds the pieces that are pure enough to reuse and test —
//! the process source and diffing, the tank simulation with its sprites and
//! mappings, the renderer, the configuration and the theme. The `procquarium`
//! binary is a thin shell around it that owns the terminal and the sampling
//! loop.

pub mod app;
pub mod config;
pub mod diff;
pub mod render;
pub mod source;
pub mod tank;
pub mod theme;
