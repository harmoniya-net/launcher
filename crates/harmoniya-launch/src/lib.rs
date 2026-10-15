//! Game install + launch engine, free of any UI framework.
//!
//! - [`options`] reads a bundle's head and resolves the player's saved
//!   choices against its options into `vars` + `features`.
//! - [`pipeline`] resolves auth, runs the `opys_runtime` install streaming
//!   progress, then spawns the game; it reports everything over a channel the
//!   caller drives.
//! - [`game`] tracks launched processes by modpack id so they can be stopped
//!   individually, are all stopped on quit, and are reaped when they exit.

pub mod game;
pub mod options;
pub mod pipeline;
