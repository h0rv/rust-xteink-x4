//! Shared UI library for Xteink X4 e-reader.
//! Works on ESP32, WASM, and desktop.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod app;
pub mod buffered_display;
pub mod diff;
pub mod eink;
pub mod file_browser;
pub mod filesystem;
pub mod font_render;
pub mod input;
pub mod library_activity;
pub mod reader_settings_activity;
pub mod settings_activity;
pub mod system_menu_activity;
pub mod ui;

// EPUB module is available when either std or quick-xml is enabled
#[cfg(any(feature = "std", feature = "quick-xml"))]
pub mod epub;

#[cfg(feature = "std")]
pub mod epub_render;

#[cfg(feature = "std")]
pub mod mock_filesystem;

pub use app::App;
pub use buffered_display::BufferedDisplay;
pub use diff::{compute_diff_region, extract_region, DiffRegion};
pub use eink::{
    Builder, Dimensions, DisplayInterface, EinkDisplay, EinkError, EinkInterface, RamXAddressing,
    RefreshMode, Region, Rotation, UpdateRegion,
};
pub use file_browser::{FileBrowser, TextViewer};
pub use filesystem::{FileInfo, FileSystem, FileSystemError};
pub use font_render::FontCache;
pub use input::{Button, InputEvent};
pub use library_activity::{create_mock_books, BookAction, BookInfo, LibraryActivity, SortOrder};
pub use reader_settings_activity::{
    LineSpacing, MarginSize, ReaderSettings, ReaderSettingsActivity, RefreshFrequency, SettingItem,
    TapZoneConfig, TextAlignment, VolumeButtonAction,
};
pub use settings_activity::{
    FontFamily, FontFamilySelector, FontSize, FontSizeSelector, Settings, SettingsActivity,
    SettingsGroup,
};
pub use system_menu_activity::{DeviceStatus, MenuItem, NavigationCallbacks, SystemMenuActivity};

#[cfg(feature = "std")]
pub use epub::{
    create_spine, extract_metadata, parse_container_xml, parse_opf, parse_spine, EpubMetadata,
    ManifestItem, Spine, SpineItem,
};

// Tokenizer is available with just quick-xml
#[cfg(feature = "quick-xml")]
pub use epub::{tokenize_html, Token, TokenizeError};

#[cfg(feature = "std")]
pub use epub_render::EpubRenderer;

#[cfg(feature = "std")]
pub use mock_filesystem::MockFileSystem;

/// UI Display dimensions (portrait mode)
/// SSD1677 panel is 480x800 pixels in its native orientation
pub const DISPLAY_WIDTH: u32 = 480;
pub const DISPLAY_HEIGHT: u32 = 800;

/// Normalize a draw target's size to portrait (width <= height).
pub fn portrait_dimensions<D: embedded_graphics::prelude::OriginDimensions>(
    display: &D,
) -> (u32, u32) {
    let size = display.size();
    let width = size.width.min(size.height);
    let height = size.width.max(size.height);
    (width, height)
}
