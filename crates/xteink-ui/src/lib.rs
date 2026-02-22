//! Shared UI library for Xteink X4 e-reader.
//! Works on ESP32, WASM, and desktop.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::panic,
        clippy::todo,
        clippy::unimplemented,
        clippy::unreachable,
        clippy::unwrap_used
    )
)]

extern crate alloc;

pub mod app;
pub mod buffered_display;
pub mod diff;
pub mod eink;
pub mod embedded_fonts;
pub mod file_browser;
pub mod file_browser_activity;
pub mod filesystem;
pub mod font_render;
pub mod information_activity;
pub mod input;
pub mod library_activity;
pub mod main_activity;
pub mod reader_settings_activity;
pub mod settings_activity;
pub mod system_menu_activity;
pub mod test_display;
pub mod ui;

#[cfg(all(feature = "std", feature = "fontdue"))]
pub mod epub_font_backend;
#[cfg(feature = "std")]
pub mod epub_prep;

#[cfg(feature = "std")]
pub mod mock_filesystem;

pub use app::{App, AppScreen};
pub use buffered_display::BufferedDisplay;
pub use diff::{compute_diff_region, extract_region, DiffRegion};
pub use eink::{
    Builder, Dimensions, DisplayInterface, EinkDisplay, EinkError, EinkInterface, RamXAddressing,
    RefreshMode, Region, Rotation, UpdateRegion,
};
pub use file_browser::{FileBrowser, TextViewer};
pub use file_browser_activity::FileBrowserActivity;
pub use filesystem::{FileInfo, FileSystem, FileSystemError};
pub use font_render::FontCache;
pub use information_activity::{InfoField, InformationActivity};
pub use input::{Button, InputEvent};
pub use library_activity::{create_mock_books, BookAction, BookInfo, LibraryActivity, SortOrder};
pub use main_activity::{MainActivity, SettingItem as MainSettingItem, Tab, UnifiedSettings};
pub use reader_settings_activity::{
    LineSpacing, MarginSize, ReaderSettings, ReaderSettingsActivity, RefreshFrequency,
    TapZoneConfig, TextAlignment, VolumeButtonAction,
};
pub use settings_activity::{FontFamily, FontSize, SettingRow, Settings, SettingsActivity};
pub use system_menu_activity::{DeviceStatus, MenuItem, SystemMenuActivity};

#[cfg(feature = "std")]
pub use epub_stream::layout::{Line as EpubLine, Page as EpubPage, TextStyle as EpubTextStyle};

#[cfg(feature = "std")]
pub use epub_stream::metadata::{
    extract_metadata, parse_container_xml, parse_opf, EpubMetadata, ManifestItem,
};

#[cfg(feature = "std")]
pub use epub_stream::spine::{create_spine, parse_spine, Spine, SpineItem};

#[cfg(feature = "std")]
pub use epub_stream::tokenizer::{tokenize_html, Token, TokenizeError};

#[cfg(feature = "std")]
pub use epub_stream::zip::{CdEntry, StreamingZip, ZipError};

#[cfg(feature = "std")]
pub use epub_prep::{
    BlockRole, ChapterStylesheets, ComputedTextStyle, EmbeddedFontFace, EmbeddedFontStyle,
    FontLimits, FontPolicy, FontResolutionTrace, FontResolver, LayoutHints, PreparedChapter,
    RenderPrep, RenderPrepError, RenderPrepOptions, ResolvedFontFace, StyleConfig, StyleLimits,
    StyledChapter, StyledEvent, StyledEventOrRun, StyledRun, Styler, StylesheetSource,
};

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
