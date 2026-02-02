//! Shared UI library for Xteink X4 e-reader.
//! Works on ESP32, WASM, and desktop.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod app;
pub mod file_browser;
pub mod filesystem;
pub mod input;

#[cfg(feature = "std")]
pub mod epub_render;

#[cfg(feature = "std")]
pub mod mock_filesystem;

pub use app::App;
pub use file_browser::{FileBrowser, TextViewer};
pub use filesystem::{FileInfo, FileSystem, FileSystemError};
pub use input::{Button, InputEvent};

#[cfg(feature = "std")]
pub use epub_render::EpubRenderer;

#[cfg(feature = "std")]
pub use mock_filesystem::MockFileSystem;

/// UI Display dimensions (portrait mode)
/// Physical display is 800x480 landscape, but UI is 480x800 portrait
pub const DISPLAY_WIDTH: u32 = 480;
pub const DISPLAY_HEIGHT: u32 = 800;
