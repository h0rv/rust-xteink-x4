//! Shared UI library for Xteink X4 e-reader.
//! Works on ESP32, WASM, and desktop.

#![cfg_attr(not(feature = "std"), no_std)]

pub mod app;
pub mod input;

pub use app::App;
pub use input::{Button, InputEvent};

/// Display: 800x480 @ 220 PPI (4.3" diagonal, landscape)
pub const DISPLAY_WIDTH: u32 = 800;
pub const DISPLAY_HEIGHT: u32 = 480;
