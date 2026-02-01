//! Shared UI library for Xteink X4 e-reader.
//! Works on ESP32, WASM, and desktop.

#![cfg_attr(not(feature = "std"), no_std)]

pub mod app;
pub mod input;

pub use app::App;
pub use input::{Button, InputEvent};

/// UI Display dimensions (portrait mode)
/// Physical display is 800x480 landscape, but UI is 480x800 portrait
pub const DISPLAY_WIDTH: u32 = 480;
pub const DISPLAY_HEIGHT: u32 = 800;
