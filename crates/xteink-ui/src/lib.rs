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
pub mod eink;
pub mod embedded_fonts;
pub mod file_browser;
pub mod file_browser_activity;
pub mod filesystem;
pub mod font_render;
pub mod information_activity;
pub mod library_activity;
pub mod main_activity;
pub mod reader_settings_activity;
pub mod settings_activity;
pub mod system_menu_activity;
pub mod test_display;

pub use einked::diff;
pub use einked::input;

pub mod ui {
    use embedded_graphics::{pixelcolor::BinaryColor, prelude::*};

    use crate::app::AppScreen;
    use crate::input::InputEvent;

    pub mod components {
        pub use einked::ui::components::*;
    }

    pub mod helpers {
        pub(crate) use einked::ui::helpers::*;
    }

    pub mod theme {
        pub use einked::ui::theme::*;
    }

    pub use components::{Button, Header, List, Modal, Toast};
    pub use einked::ui::ActivityRefreshMode;
    pub use theme::{Theme, ThemeMetrics};

    /// Result of handling an input event in Xteink app activities.
    pub type ActivityResult = einked::ui::ActivityResult<AppScreen>;

    /// Activity trait for screen-based UI architecture.
    pub trait Activity {
        fn on_enter(&mut self);
        fn on_exit(&mut self);
        fn handle_input(&mut self, event: InputEvent) -> ActivityResult;
        fn render<D: DrawTarget<Color = BinaryColor>>(
            &self,
            display: &mut D,
        ) -> Result<(), D::Error>;
        fn refresh_mode(&self) -> ActivityRefreshMode {
            ActivityRefreshMode::default()
        }
    }
}

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
pub use einked_ereader::{
    all_preloaded_sources, get_reader_url, BrowserState, FeedBrowserActivity, FeedSource, FeedType,
    OpdsCatalog, OpdsEntry, OpdsLink, JINA_READER_BASE, PRELOADED_OPDS_SOURCES,
    PRELOADED_RSS_SOURCES,
};
pub use file_browser::{FileBrowser, TextViewer};
pub use file_browser_activity::FileBrowserActivity;
pub use filesystem::{FileInfo, FileSystem, FileSystemError};
pub use font_render::FontCache;
pub use information_activity::{InfoField, InformationActivity};
pub use input::{Button, ButtonConfig, InputEvent};
pub use library_activity::{create_mock_books, BookAction, BookInfo, LibraryActivity, SortOrder};
pub use main_activity::{MainActivity, SettingItem as MainSettingItem, Tab, UnifiedSettings};
pub use reader_settings_activity::{
    LineSpacing, MarginSize, ReaderSettings, ReaderSettingsActivity, RefreshFrequency,
    TapZoneConfig, TextAlignment, VolumeButtonAction,
};
pub use settings_activity::{
    FontFamily, FontSize, SettingRow, Settings, SettingsActivity, SleepScreenMode,
};
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

/// UI display dimensions (portrait mode).
/// SSD1677 panel is 480x800 pixels in its native orientation.
pub const DISPLAY_WIDTH: u32 = 480;
pub const DISPLAY_HEIGHT: u32 = 800;

pub use einked::portrait_dimensions;
