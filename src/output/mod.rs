//! Output backends. PNG export lives on [`crate::canvas::Canvas`]; the physical
//! e-paper panel is only available on `device` builds.

#[cfg(feature = "device")]
pub mod epaper;
