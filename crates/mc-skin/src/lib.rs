//! Minecraft skin rendering, free of any UI framework.
//!
//! - [`head`] renders a player head (face + hat overlay) to PNG bytes.
//! - [`render`] software-rasterizes the full character to an [`image::RgbaImage`].
//!
//! Both take raw skin PNG bytes and are pure functions of their input, so the
//! results are easy to cache and test.

pub mod head;
pub mod render;

pub use render::{rasterize, Model, HEIGHT, WIDTH};
