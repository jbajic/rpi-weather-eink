//! Weather e-ink display core.
//!
//! The crate is split into a reusable library plus thin binaries so the same
//! fetch + render pipeline can later back a long-running daemon and web UI.
//!
//! Rendering always targets an in-memory [`embedded_graphics`] draw target, so
//! the exact same layout code feeds either a PNG (host preview) or the physical
//! e-paper panel (device build).

pub mod canvas;
pub mod config;
pub mod output;
pub mod render;
pub mod weather;

pub use config::Config;
pub use weather::Forecast;
