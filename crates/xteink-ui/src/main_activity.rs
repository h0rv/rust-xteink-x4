//! Main Activity with 3-tab navigation for Xteink X4 e-reader.
//!
//! Clean, simple design with Bookerly-inspired typography using embedded fonts.
//! Tab indicator dots at bottom. No top bar - full content area.

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use embedded_graphics::{
    mono_font::{MonoTextStyle, MonoTextStyleBuilder},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{Circle, PrimitiveStyle, Rectangle},
    text::Text,
};

use crate::file_browser_activity::FileBrowserActivity;
use crate::filesystem::FileSystem;
use crate::input::{Button, InputEvent};
use crate::library_activity::BookInfo;
use crate::reader_settings_activity::{
    FooterAutoHide, FooterDensity, LineSpacing, MarginSize, ReaderSettings, RefreshFrequency,
    TapZoneConfig, TextAlignment, VolumeButtonAction,
};
use crate::settings_activity::{AutoSleepDuration, FontFamily, FontSize};
use crate::system_menu_activity::DeviceStatus;
use crate::ui::theme::layout::{
    self, BOTTOM_BAR_H, DOT_SIZE, DOT_SPACING, GAP_LG, GAP_MD, HEADER_TEXT_Y, HERO_H, INNER_PAD,
    MARGIN, SELECT_PAD_X,
};
use crate::ui::theme::{
    set_device_font_profile, ui_font_body, ui_font_small, ui_font_small_char_width, ui_font_title,
};
use crate::ui::{Activity, ActivityRefreshMode, ActivityResult};
use crate::DISPLAY_HEIGHT;
use crate::DISPLAY_WIDTH;

/// The three tabs in order
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Library = 0,
    Files = 1,
    Settings = 2,
}

impl Tab {
    pub const ALL: [Self; 3] = [Self::Library, Self::Files, Self::Settings];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::Library),
            1 => Some(Self::Files),
            2 => Some(Self::Settings),
            _ => None,
        }
    }
}

/// Main activity with 3-tab navigation
pub struct MainActivity {
    current_tab: usize,
    pub library_tab: LibraryTabContent,
    pub files_tab: FilesTabContent,
    settings_tab: SettingsTabContent,
    device_status: DeviceStatus,
}

/// Content for Library tab (Tab 0)
pub struct LibraryTabContent {
    books: Vec<BookInfo>,
    selected_index: usize,
    is_loading: bool,
    pending_open_path: Option<String>,
    refresh_request: bool,
}

/// Content for Files tab (Tab 1)
pub struct FilesTabContent {
    file_browser: FileBrowserActivity,
}

/// Unified setting item for Settings tab
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingItem {
    FontSize,
    FontFamily,
    AutoSleep,
    InvertColors,
    // --- Advanced divider rendered between these ---
    LineSpacing,
    MarginSize,
    TextAlignment,
    ShowPageNumbers,
    RefreshFrequency,
    VolumeButtonAction,
    TapZoneConfig,
}

impl SettingItem {
    /// Primary settings (most-used, always visible first).
    pub const PRIMARY: [Self; 4] = [
        Self::FontSize,
        Self::FontFamily,
        Self::AutoSleep,
        Self::InvertColors,
    ];

    /// Advanced / reader-specific settings.
    pub const ADVANCED: [Self; 7] = [
        Self::LineSpacing,
        Self::MarginSize,
        Self::TextAlignment,
        Self::ShowPageNumbers,
        Self::RefreshFrequency,
        Self::VolumeButtonAction,
        Self::TapZoneConfig,
    ];

    /// All settings in display order (primary then advanced).
    pub const ALL: [Self; 11] = [
        Self::FontSize,
        Self::FontFamily,
        Self::AutoSleep,
        Self::InvertColors,
        Self::LineSpacing,
        Self::MarginSize,
        Self::TextAlignment,
        Self::ShowPageNumbers,
        Self::RefreshFrequency,
        Self::VolumeButtonAction,
        Self::TapZoneConfig,
    ];

    /// Index of the first advanced item in ALL.
    pub const ADVANCED_START: usize = Self::PRIMARY.len();

    pub fn label(self) -> &'static str {
        match self {
            Self::FontSize => "Font Size",
            Self::FontFamily => "Font Family",
            Self::AutoSleep => "Auto Sleep",
            Self::InvertColors => "Invert Colors",
            Self::LineSpacing => "Line Spacing",
            Self::MarginSize => "Margins",
            Self::TextAlignment => "Text Align",
            Self::ShowPageNumbers => "Page Numbers",
            Self::RefreshFrequency => "Refresh",
            Self::VolumeButtonAction => "Vol Buttons",
            Self::TapZoneConfig => "Tap Zones",
        }
    }
}

/// Unified settings state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnifiedSettings {
    pub font_size: FontSize,
    pub font_family: FontFamily,
    pub auto_sleep_duration: AutoSleepDuration,
    pub line_spacing: LineSpacing,
    pub margin_size: MarginSize,
    pub text_alignment: TextAlignment,
    pub show_page_numbers: bool,
    pub footer_density: FooterDensity,
    pub footer_auto_hide: FooterAutoHide,
    pub refresh_frequency: RefreshFrequency,
    pub invert_colors: bool,
    pub volume_button_action: VolumeButtonAction,
    pub tap_zone_config: TapZoneConfig,
}

impl Default for UnifiedSettings {
    fn default() -> Self {
        Self {
            font_size: FontSize::Medium,
            font_family: FontFamily::Serif,
            auto_sleep_duration: AutoSleepDuration::TenMinutes,
            line_spacing: LineSpacing::Normal,
            margin_size: MarginSize::Medium,
            text_alignment: TextAlignment::Justified,
            show_page_numbers: true,
            footer_density: FooterDensity::Detailed,
            footer_auto_hide: FooterAutoHide::Off,
            refresh_frequency: RefreshFrequency::Never,
            invert_colors: false,
            volume_button_action: VolumeButtonAction::Scroll,
            tap_zone_config: TapZoneConfig::LeftNext,
        }
    }
}

impl UnifiedSettings {
    pub fn to_reader_settings(self) -> ReaderSettings {
        ReaderSettings {
            font_size: self.font_size,
            font_family: self.font_family,
            line_spacing: self.line_spacing,
            margin_size: self.margin_size,
            text_alignment: self.text_alignment,
            show_page_numbers: self.show_page_numbers,
            footer_density: self.footer_density,
            footer_auto_hide: self.footer_auto_hide,
            refresh_frequency: self.refresh_frequency,
            invert_colors: self.invert_colors,
            tap_zone_config: self.tap_zone_config,
            volume_button_action: self.volume_button_action,
        }
    }
}

/// Content for Settings tab (Tab 2)
pub struct SettingsTabContent {
    settings: UnifiedSettings,
    selected_index: usize,
}

impl MainActivity {
    /// Create new main activity
    pub fn new() -> Self {
        Self {
            current_tab: 0,
            library_tab: LibraryTabContent::new(),
            files_tab: FilesTabContent::new(),
            settings_tab: SettingsTabContent::new(),
            device_status: DeviceStatus::default(),
        }
    }

    /// Set device status for battery display
    pub fn set_device_status(&mut self, status: DeviceStatus) {
        self.device_status = status;
        self.files_tab.set_battery_percent(status.battery_percent);
    }

    /// Get current tab
    pub fn current_tab(&self) -> Tab {
        Tab::from_index(self.current_tab).unwrap_or(Tab::Library)
    }

    /// Switch to a specific tab
    pub fn set_tab(&mut self, tab: Tab) {
        let new_index = tab.index();
        if new_index == self.current_tab {
            return;
        }

        // Exit current tab
        match self.current_tab {
            0 => self.library_tab.on_exit(),
            1 => self.files_tab.on_exit(),
            2 => self.settings_tab.on_exit(),
            _ => {}
        }

        self.current_tab = new_index;

        // Enter new tab
        match self.current_tab {
            0 => self.library_tab.on_enter(),
            1 => self.files_tab.on_enter(),
            2 => self.settings_tab.on_enter(),
            _ => {}
        }
    }

    pub fn switch_to_tab(&mut self, tab: Tab) {
        self.current_tab = tab.index();
    }

    pub fn settings(&self) -> UnifiedSettings {
        self.settings_tab.settings
    }

    pub fn apply_settings(&mut self, settings: UnifiedSettings) {
        self.settings_tab.settings = settings;
        set_device_font_profile(settings.font_size.index(), settings.font_family.index());
        self.files_tab
            .set_reader_settings(settings.to_reader_settings());
    }

    pub fn auto_sleep_duration_ms(&self) -> u32 {
        self.settings_tab
            .settings
            .auto_sleep_duration
            .milliseconds()
    }

    /// Cycle to next tab (right)
    fn next_tab(&mut self) {
        let next = (self.current_tab + 1) % 3;
        if let Some(tab) = Tab::from_index(next) {
            self.set_tab(tab);
        }
    }

    /// Cycle to previous tab (left)
    fn prev_tab(&mut self) {
        let prev = (self.current_tab + 2) % 3;
        if let Some(tab) = Tab::from_index(prev) {
            self.set_tab(tab);
        }
    }

    /// Render tab indicator dots at bottom center with battery
    fn render_bottom_bar<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
    ) -> Result<(), D::Error> {
        let bar_y = DISPLAY_HEIGHT as i32 - BOTTOM_BAR_H;

        // Clear the bottom bar area first
        Rectangle::new(
            Point::new(0, bar_y),
            Size::new(DISPLAY_WIDTH, BOTTOM_BAR_H as u32),
        )
        .into_styled(PrimitiveStyle::with_fill(BinaryColor::Off))
        .draw(display)?;

        // Calculate center position for dots
        let center_x = (DISPLAY_WIDTH as i32) / 2;
        let dot_y = bar_y + BOTTOM_BAR_H / 2;

        // Draw 3 dots centered
        for i in 0..3i32 {
            let x = center_x + (i - 1) * DOT_SPACING;
            let center = Point::new(x, dot_y);
            let top_left = center - Point::new(DOT_SIZE as i32 / 2, DOT_SIZE as i32 / 2);

            if i as usize == self.current_tab {
                Circle::new(top_left, DOT_SIZE)
                    .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
                    .draw(display)?;
            } else {
                Circle::new(top_left, DOT_SIZE)
                    .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 2))
                    .draw(display)?;
            }
        }

        // Draw battery percentage on bottom right
        let battery_text = format!("{}%", self.device_status.battery_percent);
        let battery_style = MonoTextStyle::new(ui_font_small(), BinaryColor::On);
        let text_width = battery_text.len() as i32 * ui_font_small_char_width();
        Text::new(
            &battery_text,
            Point::new(DISPLAY_WIDTH as i32 - MARGIN - text_width, dot_y + 4),
            battery_style,
        )
        .draw(display)?;

        Ok(())
    }

    /// Delegate input to current tab
    fn delegate_input(&mut self, event: InputEvent) -> ActivityResult {
        match self.current_tab {
            0 => self.library_tab.handle_input(event),
            1 => self.files_tab.handle_input(event),
            2 => self.settings_tab.handle_input(event),
            _ => ActivityResult::Ignored,
        }
    }

    fn should_show_bottom_bar(&self) -> bool {
        if self.current_tab == Tab::Files.index() && self.files_tab.is_reading() {
            return false;
        }

        true
    }
}

impl Activity for MainActivity {
    fn on_enter(&mut self) {
        self.library_tab.on_enter();
        self.files_tab.on_enter();
        self.settings_tab.on_enter();
    }

    fn on_exit(&mut self) {
        self.library_tab.on_exit();
        self.files_tab.on_exit();
        self.settings_tab.on_exit();
    }

    fn handle_input(&mut self, event: InputEvent) -> ActivityResult {
        let settings_before = self.settings_tab.settings;
        let result = match event {
            InputEvent::Press(Button::Left) => {
                if self.current_tab == Tab::Files.index() && self.files_tab.is_reading() {
                    self.delegate_input(event)
                } else {
                    self.prev_tab();
                    ActivityResult::Consumed
                }
            }
            InputEvent::Press(Button::Right) => {
                if self.current_tab == Tab::Files.index() && self.files_tab.is_reading() {
                    self.delegate_input(event)
                } else {
                    self.next_tab();
                    ActivityResult::Consumed
                }
            }
            _ => self.delegate_input(event),
        };
        if self.settings_tab.settings != settings_before {
            self.files_tab
                .set_reader_settings(self.settings_tab.settings.to_reader_settings());
        }
        result
    }

    fn render<D: DrawTarget<Color = BinaryColor>>(&self, display: &mut D) -> Result<(), D::Error> {
        // Clear display
        Rectangle::new(Point::new(0, 0), Size::new(DISPLAY_WIDTH, DISPLAY_HEIGHT))
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::Off))
            .draw(display)?;

        // Render current tab content
        match self.current_tab {
            0 => self.library_tab.render(display)?,
            1 => self.files_tab.render(display)?,
            2 => self.settings_tab.render(display)?,
            _ => {}
        }

        // Render bottom bar with dots and battery
        if self.should_show_bottom_bar() {
            self.render_bottom_bar(display)?;
        }

        Ok(())
    }

    fn refresh_mode(&self) -> ActivityRefreshMode {
        ActivityRefreshMode::Fast
    }
}

// ============================================================================
// Library Tab Implementation
// ============================================================================

impl LibraryTabContent {
    fn new() -> Self {
        Self {
            books: Vec::new(),
            selected_index: 0,
            is_loading: false,
            pending_open_path: None,
            refresh_request: false,
        }
    }

    pub fn begin_loading_scan(&mut self) {
        self.is_loading = true;
    }

    pub fn finish_loading_scan(&mut self) {
        self.is_loading = false;
    }

    pub fn set_books(&mut self, books: Vec<BookInfo>) {
        self.books = books;
        self.selected_index = 0;
    }

    pub fn take_refresh_request(&mut self) -> bool {
        let requested = self.refresh_request;
        self.refresh_request = false;
        requested
    }

    /// Take the pending open path (called by App to process book opening)
    pub fn take_open_request(&mut self) -> Option<String> {
        self.pending_open_path.take()
    }

    fn on_enter(&mut self) {}
    fn on_exit(&mut self) {}

    fn handle_input(&mut self, event: InputEvent) -> ActivityResult {
        match event {
            InputEvent::Press(Button::Up) | InputEvent::Press(Button::VolumeUp) => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                }
                ActivityResult::Consumed
            }
            InputEvent::Press(Button::Down) | InputEvent::Press(Button::VolumeDown) => {
                if self.selected_index + 1 < self.books.len() {
                    self.selected_index += 1;
                }
                ActivityResult::Consumed
            }
            InputEvent::Press(Button::Confirm) => {
                // Open the selected book
                if let Some(book) = self.books.get(self.selected_index) {
                    self.pending_open_path = Some(book.path.clone());
                }
                ActivityResult::Consumed
            }
            InputEvent::Press(Button::Power) => {
                // Quick resume: open most recent book from hero card in one press.
                if let Some(book) = self.books.first() {
                    self.pending_open_path = Some(book.path.clone());
                } else {
                    self.refresh_request = true;
                    self.begin_loading_scan();
                }
                ActivityResult::Consumed
            }
            InputEvent::Press(Button::Back) => {
                self.refresh_request = true;
                self.begin_loading_scan();
                ActivityResult::Consumed
            }
            _ => ActivityResult::Ignored,
        }
    }

    fn render<D: DrawTarget<Color = BinaryColor>>(&self, display: &mut D) -> Result<(), D::Error> {
        let title_style = MonoTextStyle::new(ui_font_title(), BinaryColor::On);
        let body_style = MonoTextStyle::new(ui_font_body(), BinaryColor::On);
        let small_style = MonoTextStyle::new(ui_font_small(), BinaryColor::On);

        // Title
        Text::new("Library", Point::new(MARGIN, HEADER_TEXT_Y), title_style).draw(display)?;

        // Hero card area
        let hero_y = HEADER_TEXT_Y + GAP_MD;
        let hero_height = HERO_H;

        // Hero card border
        Rectangle::new(
            Point::new(MARGIN, hero_y),
            Size::new(DISPLAY_WIDTH - (MARGIN as u32 * 2), hero_height as u32),
        )
        .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 2))
        .draw(display)?;

        // Vertical positions inside hero card (evenly spaced)
        let hero_line1 = hero_y + GAP_MD + 2; // "Currently Reading" label
        let hero_line2 = hero_line1 + GAP_LG; // Title / loading text
        let hero_line3 = hero_line2 + GAP_LG - 5; // Author
        let hero_line4 = hero_line3 + GAP_LG - 5; // Progress

        Text::new(
            "Currently Reading",
            Point::new(MARGIN + INNER_PAD, hero_line1),
            body_style,
        )
        .draw(display)?;

        if self.is_loading {
            Text::new(
                "Loading...",
                Point::new(MARGIN + INNER_PAD, hero_line2),
                small_style,
            )
            .draw(display)?;
        } else if self.books.is_empty() {
            Text::new(
                "No book in progress",
                Point::new(MARGIN + INNER_PAD, hero_line2),
                small_style,
            )
            .draw(display)?;
        } else if let Some(book) = self.books.first() {
            Text::new(
                &book.title,
                Point::new(MARGIN + INNER_PAD, hero_line2),
                body_style,
            )
            .draw(display)?;
            if !book.author.is_empty() {
                Text::new(
                    &book.author,
                    Point::new(MARGIN + INNER_PAD, hero_line3),
                    small_style,
                )
                .draw(display)?;
            }
            let progress_text = format!("{}%", book.progress_percent);
            Text::new(
                &progress_text,
                Point::new(MARGIN + INNER_PAD, hero_line4),
                small_style,
            )
            .draw(display)?;
        }

        // Library section
        let list_y = hero_y + hero_height + GAP_LG;
        Text::new("Your Books", Point::new(MARGIN, list_y), title_style).draw(display)?;

        if self.is_loading {
            Text::new(
                "Scanning...",
                Point::new(MARGIN, list_y + GAP_LG),
                small_style,
            )
            .draw(display)?;
        } else if self.books.is_empty() {
            Text::new(
                "No books found",
                Point::new(MARGIN, list_y + GAP_LG),
                small_style,
            )
            .draw(display)?;
        } else {
            let font_h = ui_font_body().character_size.height as i32;
            let item_h = font_h + INNER_PAD;
            let start_y = list_y + GAP_LG;
            let max_items =
                layout::max_items(start_y, item_h, DISPLAY_HEIGHT as i32).max(1) as usize;
            for (i, book) in self.books.iter().enumerate().take(max_items) {
                let y = start_y + (i as i32) * item_h;
                if i == self.selected_index {
                    Rectangle::new(
                        Point::new(MARGIN - SELECT_PAD_X, y - font_h),
                        Size::new(
                            DISPLAY_WIDTH - (MARGIN as u32 * 2) + (SELECT_PAD_X as u32 * 2),
                            item_h as u32,
                        ),
                    )
                    .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
                    .draw(display)?;
                    let selected_style = MonoTextStyleBuilder::new()
                        .font(ui_font_body())
                        .text_color(BinaryColor::Off)
                        .background_color(BinaryColor::On)
                        .build();
                    Text::new(&book.title, Point::new(MARGIN, y), selected_style).draw(display)?;
                } else {
                    Text::new(&book.title, Point::new(MARGIN, y), body_style).draw(display)?;
                }
            }
        }

        Ok(())
    }
}

// ============================================================================
// Files Tab Implementation
// ============================================================================

impl FilesTabContent {
    fn new() -> Self {
        Self {
            file_browser: FileBrowserActivity::new(),
        }
    }

    pub fn process_pending_task(&mut self, fs: &mut dyn FileSystem) -> bool {
        self.file_browser.process_pending_task(fs)
    }

    pub fn set_reader_settings(&mut self, settings: ReaderSettings) {
        self.file_browser.set_reader_settings(settings);
    }

    pub fn set_battery_percent(&mut self, battery_percent: u8) {
        self.file_browser.set_battery_percent(battery_percent);
    }

    pub fn request_open_path(&mut self, path: impl Into<String>) {
        self.file_browser.request_open_path(path);
    }

    pub fn is_opening_epub(&self) -> bool {
        self.file_browser.is_opening_epub()
    }

    pub fn is_reading_text(&self) -> bool {
        self.file_browser.is_viewing_text()
    }

    pub fn is_reading_epub(&self) -> bool {
        self.file_browser.is_viewing_epub()
    }

    pub fn is_reading(&self) -> bool {
        self.is_reading_text() || self.is_reading_epub()
    }

    pub fn epub_position(&self) -> Option<(usize, usize, usize, usize)> {
        self.file_browser.epub_position()
    }

    #[cfg(feature = "std")]
    pub fn active_epub_path(&self) -> Option<&str> {
        self.file_browser.active_epub_path()
    }

    fn on_enter(&mut self) {
        self.file_browser.on_enter();
    }

    fn on_exit(&mut self) {
        self.file_browser.on_exit();
    }

    fn handle_input(&mut self, event: InputEvent) -> ActivityResult {
        self.file_browser.handle_input(event)
    }

    fn render<D: DrawTarget<Color = BinaryColor>>(&self, display: &mut D) -> Result<(), D::Error> {
        self.file_browser.render(display)
    }
}

// ============================================================================
// Settings Tab Implementation
// ============================================================================

impl SettingsTabContent {
    fn new() -> Self {
        Self {
            settings: UnifiedSettings::default(),
            selected_index: 0,
        }
    }

    fn on_enter(&mut self) {}
    fn on_exit(&mut self) {}

    fn handle_input(&mut self, event: InputEvent) -> ActivityResult {
        match event {
            InputEvent::Press(Button::Up) | InputEvent::Press(Button::VolumeUp) => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                }
                ActivityResult::Consumed
            }
            InputEvent::Press(Button::Down) | InputEvent::Press(Button::VolumeDown) => {
                if self.selected_index < SettingItem::ALL.len() - 1 {
                    self.selected_index += 1;
                }
                ActivityResult::Consumed
            }
            InputEvent::Press(Button::Confirm) => {
                self.toggle_current_setting();
                ActivityResult::Consumed
            }
            _ => ActivityResult::Ignored,
        }
    }

    fn toggle_current_setting(&mut self) {
        let item = SettingItem::ALL[self.selected_index];
        match item {
            SettingItem::FontSize => {
                self.settings.font_size = Self::cycle_font_size(self.settings.font_size);
            }
            SettingItem::FontFamily => {
                self.settings.font_family = Self::cycle_font_family(self.settings.font_family);
            }
            SettingItem::AutoSleep => {
                self.settings.auto_sleep_duration =
                    self.settings.auto_sleep_duration.next_wrapped();
            }
            SettingItem::LineSpacing => {
                self.settings.line_spacing = self.settings.line_spacing.next_wrapped();
            }
            SettingItem::MarginSize => {
                self.settings.margin_size = self.settings.margin_size.next_wrapped();
            }
            SettingItem::TextAlignment => {
                self.settings.text_alignment = self.settings.text_alignment.next_wrapped();
            }
            SettingItem::ShowPageNumbers => {
                self.settings.show_page_numbers = !self.settings.show_page_numbers;
            }
            SettingItem::RefreshFrequency => {
                self.settings.refresh_frequency = self.settings.refresh_frequency.next_wrapped();
            }
            SettingItem::InvertColors => {
                self.settings.invert_colors = !self.settings.invert_colors;
            }
            SettingItem::VolumeButtonAction => {
                self.settings.volume_button_action =
                    self.settings.volume_button_action.next_wrapped();
            }
            SettingItem::TapZoneConfig => {
                self.settings.tap_zone_config = self.settings.tap_zone_config.next_wrapped();
            }
        }
    }

    fn cycle_font_size(current: FontSize) -> FontSize {
        match current {
            FontSize::Small => FontSize::Medium,
            FontSize::Medium => FontSize::Large,
            FontSize::Large => FontSize::ExtraLarge,
            FontSize::ExtraLarge => FontSize::Huge,
            FontSize::Huge => FontSize::Max,
            FontSize::Max => FontSize::Small,
        }
    }

    fn cycle_font_family(current: FontFamily) -> FontFamily {
        match current {
            FontFamily::Monospace => FontFamily::Serif,
            FontFamily::Serif => FontFamily::SansSerif,
            FontFamily::SansSerif => FontFamily::Monospace,
        }
    }

    fn get_setting_value_text(&self, item: SettingItem) -> String {
        match item {
            SettingItem::FontSize => format!("{:?}", self.settings.font_size),
            SettingItem::FontFamily => format!("{:?}", self.settings.font_family),
            SettingItem::AutoSleep => self.settings.auto_sleep_duration.label().into(),
            SettingItem::LineSpacing => format!("{:?}", self.settings.line_spacing),
            SettingItem::MarginSize => format!("{:?}", self.settings.margin_size),
            SettingItem::TextAlignment => format!("{:?}", self.settings.text_alignment),
            SettingItem::ShowPageNumbers => {
                if self.settings.show_page_numbers {
                    "On".into()
                } else {
                    "Off".into()
                }
            }
            SettingItem::RefreshFrequency => self.settings.refresh_frequency.label().into(),
            SettingItem::InvertColors => {
                if self.settings.invert_colors {
                    "On".into()
                } else {
                    "Off".into()
                }
            }
            SettingItem::VolumeButtonAction => {
                format!("{:?}", self.settings.volume_button_action)
            }
            SettingItem::TapZoneConfig => format!("{:?}", self.settings.tap_zone_config),
        }
    }

    fn render<D: DrawTarget<Color = BinaryColor>>(&self, display: &mut D) -> Result<(), D::Error> {
        let title_style = MonoTextStyle::new(ui_font_title(), BinaryColor::On);
        let label_style = MonoTextStyle::new(ui_font_body(), BinaryColor::On);
        let value_style = MonoTextStyle::new(ui_font_small(), BinaryColor::On);
        let selected_bg_style = MonoTextStyleBuilder::new()
            .font(ui_font_body())
            .text_color(BinaryColor::Off)
            .background_color(BinaryColor::On)
            .build();
        let selected_value_style = MonoTextStyleBuilder::new()
            .font(ui_font_small())
            .text_color(BinaryColor::Off)
            .background_color(BinaryColor::On)
            .build();

        // Title
        Text::new("Settings", Point::new(MARGIN, HEADER_TEXT_Y), title_style).draw(display)?;

        // Settings list — derive item height from body font
        let font_h = ui_font_body().character_size.height as i32;
        let item_height = font_h + GAP_MD;
        let start_y = HEADER_TEXT_Y + GAP_MD;
        let max_visible =
            layout::max_items(start_y, item_height, DISPLAY_HEIGHT as i32).max(1) as usize;

        let scroll_offset = if self.selected_index >= max_visible {
            self.selected_index - max_visible + 1
        } else {
            0
        };

        let section_style = MonoTextStyle::new(ui_font_small(), BinaryColor::On);
        // Extra vertical offset accumulated for section dividers
        let mut extra_y = 0i32;

        for (i, item) in SettingItem::ALL.iter().enumerate().skip(scroll_offset) {
            let display_idx = i - scroll_offset;

            // Section divider before the first advanced item
            if i == SettingItem::ADVANCED_START && display_idx > 0 {
                let div_y = start_y + (display_idx as i32) * item_height + extra_y;
                // Separator line
                Rectangle::new(
                    Point::new(MARGIN, div_y - font_h / 2),
                    Size::new(DISPLAY_WIDTH - MARGIN as u32 * 2, 1),
                )
                .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
                .draw(display)?;
                // "Advanced" label
                Text::new(
                    "Advanced",
                    Point::new(MARGIN, div_y - font_h / 2 + font_h),
                    section_style,
                )
                .draw(display)?;
                extra_y += item_height;
            }

            let y = start_y + (display_idx as i32) * item_height + extra_y;

            if y > DISPLAY_HEIGHT as i32 - BOTTOM_BAR_H - INNER_PAD {
                break;
            }

            let is_selected = i == self.selected_index;
            let label = item.label();
            let value = self.get_setting_value_text(*item);

            if is_selected {
                Rectangle::new(
                    Point::new(MARGIN - SELECT_PAD_X, y - font_h),
                    Size::new(
                        DISPLAY_WIDTH - (MARGIN as u32 * 2) + (SELECT_PAD_X as u32 * 2),
                        item_height as u32,
                    ),
                )
                .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
                .draw(display)?;

                Text::new(label, Point::new(MARGIN, y), selected_bg_style).draw(display)?;

                // Right-align value
                let value_width = value.len() as i32 * ui_font_small_char_width();
                Text::new(
                    &value,
                    Point::new((DISPLAY_WIDTH as i32) - MARGIN - value_width, y),
                    selected_value_style,
                )
                .draw(display)?;
            } else {
                Text::new(label, Point::new(MARGIN, y), label_style).draw(display)?;

                // Right-align value
                let value_width = value.len() as i32 * ui_font_small_char_width();
                Text::new(
                    &value,
                    Point::new((DISPLAY_WIDTH as i32) - MARGIN - value_width, y),
                    value_style,
                )
                .draw(display)?;
            }
        }

        Ok(())
    }
}

impl Default for MainActivity {
    fn default() -> Self {
        Self::new()
    }
}
