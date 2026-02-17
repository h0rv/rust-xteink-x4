//! Main Activity with 3-tab navigation for Xteink X4 e-reader.
//!
//! Clean, simple design with Bookerly-inspired typography using embedded fonts.
//! Tab indicator dots at bottom. No top bar - full content area.

extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
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
    set_device_font_profile, ui_font_body, ui_font_body_char_width, ui_font_small,
    ui_font_small_char_width, ui_font_title,
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
    hero_selected: bool,
    transfer_selected: bool,
    transfer_screen_open: bool,
    file_transfer_active: bool,
    is_loading: bool,
    pending_open_path: Option<String>,
    pending_file_transfer_request: Option<bool>,
    transfer_mode: String,
    transfer_ssid: String,
    transfer_password_hint: String,
    transfer_url: String,
    transfer_message: String,
    transfer_menu_index: usize,
    transfer_editor: Option<TransferEditorKind>,
    transfer_edit_buffer: String,
    transfer_edit_cursor: usize,
    transfer_ap_ssid_value: String,
    transfer_ap_password_value: String,
    pending_wifi_mode_ap: Option<bool>,
    pending_wifi_ap_config: Option<(String, String)>,
    refresh_request: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TransferEditorKind {
    ApSsid,
    ApPassword,
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

    /// Queue opening a content path (epub/text/image) in the reader subsystem.
    pub fn queue_open_content_path(&mut self, path: impl Into<String>) {
        self.files_tab.request_open_path(path);
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

        // Library-only quick action hint.
        if self.current_tab == Tab::Library.index() {
            let chip_w = 136u32;
            let chip_h = 18u32;
            let chip_x = MARGIN - 2;
            let chip_y = bar_y + ((BOTTOM_BAR_H - chip_h as i32) / 2);
            Rectangle::new(Point::new(chip_x, chip_y), Size::new(chip_w, chip_h))
                .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
                .draw(display)?;
            let label = if self.library_tab.is_transfer_screen_open() {
                "Back: Exit Transfer"
            } else {
                "Back: Rescan"
            };
            Text::new(
                label,
                Point::new(chip_x + 6, chip_y + 13),
                MonoTextStyle::new(ui_font_small(), BinaryColor::On),
            )
            .draw(display)?;
        }

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
        let library_transfer_modal_open =
            self.current_tab == Tab::Library.index() && self.library_tab.is_transfer_screen_open();
        let result = match event {
            InputEvent::Press(Button::Left) => {
                if (self.current_tab == Tab::Files.index() && self.files_tab.is_reading())
                    || library_transfer_modal_open
                {
                    self.delegate_input(event)
                } else {
                    self.prev_tab();
                    ActivityResult::Consumed
                }
            }
            InputEvent::Press(Button::Right) => {
                if (self.current_tab == Tab::Files.index() && self.files_tab.is_reading())
                    || library_transfer_modal_open
                {
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
            hero_selected: true,
            transfer_selected: false,
            transfer_screen_open: false,
            file_transfer_active: false,
            is_loading: false,
            pending_open_path: None,
            pending_file_transfer_request: None,
            transfer_mode: String::from("Hotspot"),
            transfer_ssid: String::new(),
            transfer_password_hint: String::new(),
            transfer_url: String::new(),
            transfer_message: String::new(),
            transfer_menu_index: 0,
            transfer_editor: None,
            transfer_edit_buffer: String::new(),
            transfer_edit_cursor: 0,
            transfer_ap_ssid_value: String::from("Xteink-X4"),
            transfer_ap_password_value: String::from("xteink2026"),
            pending_wifi_mode_ap: None,
            pending_wifi_ap_config: None,
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
        self.hero_selected = true;
        self.transfer_selected = false;
    }

    pub fn update_book_progress(&mut self, path: &str, progress_percent: u8, last_read: u64) {
        if let Some(book) = self.books.iter_mut().find(|book| book.path == path) {
            book.progress_percent = progress_percent.min(100);
            book.last_read = Some(last_read);
        }
    }

    pub fn take_refresh_request(&mut self) -> bool {
        let requested = self.refresh_request;
        self.refresh_request = false;
        requested
    }

    pub fn set_file_transfer_active(&mut self, active: bool) {
        self.file_transfer_active = active;
    }

    pub fn set_file_transfer_network_details(
        &mut self,
        mode: String,
        ssid: String,
        password_hint: String,
        url: String,
        message: String,
    ) {
        self.transfer_mode = mode;
        self.transfer_ssid = ssid;
        self.transfer_password_hint = password_hint;
        self.transfer_url = url;
        self.transfer_message = message;
        if !self.transfer_ssid.is_empty() {
            self.transfer_ap_ssid_value = self.transfer_ssid.clone();
        }
        if let Some(password) = self.transfer_password_hint.strip_prefix("Password: ") {
            self.transfer_ap_password_value = password.to_string();
        }
    }

    pub fn take_file_transfer_request(&mut self) -> Option<bool> {
        self.pending_file_transfer_request.take()
    }

    pub fn take_wifi_mode_request(&mut self) -> Option<bool> {
        self.pending_wifi_mode_ap.take()
    }

    pub fn take_wifi_ap_config_request(&mut self) -> Option<(String, String)> {
        self.pending_wifi_ap_config.take()
    }

    pub fn is_transfer_screen_open(&self) -> bool {
        self.transfer_screen_open
    }

    /// Take the pending open path (called by App to process book opening)
    pub fn take_open_request(&mut self) -> Option<String> {
        self.pending_open_path.take()
    }

    fn on_enter(&mut self) {}
    fn on_exit(&mut self) {
        if self.transfer_screen_open || self.file_transfer_active {
            self.pending_file_transfer_request = Some(false);
        }
        self.transfer_screen_open = false;
        self.transfer_selected = false;
        self.transfer_editor = None;
    }

    fn transfer_charset() -> &'static [u8] {
        b" abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_.:/"
    }

    fn cycle_editor_char(&mut self, step: isize) {
        let charset = Self::transfer_charset();
        if charset.is_empty() {
            return;
        }
        if self.transfer_edit_buffer.is_empty() {
            self.transfer_edit_buffer.push(charset[0] as char);
            self.transfer_edit_cursor = 0;
            return;
        }
        if self.transfer_edit_cursor >= self.transfer_edit_buffer.len() {
            self.transfer_edit_cursor = self.transfer_edit_buffer.len().saturating_sub(1);
        }
        let mut bytes = self.transfer_edit_buffer.clone().into_bytes();
        let current = bytes[self.transfer_edit_cursor];
        let mut idx = charset.iter().position(|ch| *ch == current).unwrap_or(0) as isize;
        idx = (idx + step).rem_euclid(charset.len() as isize);
        bytes[self.transfer_edit_cursor] = charset[idx as usize];
        if let Ok(next) = String::from_utf8(bytes) {
            self.transfer_edit_buffer = next;
        }
    }

    fn begin_transfer_editor(&mut self, kind: TransferEditorKind) {
        self.transfer_editor = Some(kind);
        self.transfer_edit_buffer = match kind {
            TransferEditorKind::ApSsid => self.transfer_ap_ssid_value.clone(),
            TransferEditorKind::ApPassword => self.transfer_ap_password_value.clone(),
        };
        if self.transfer_edit_buffer.is_empty() {
            self.transfer_edit_buffer.push(' ');
        }
        self.transfer_edit_cursor = self.transfer_edit_buffer.len().saturating_sub(1);
    }

    fn commit_transfer_editor(&mut self) {
        let Some(kind) = self.transfer_editor else {
            return;
        };
        let trimmed = self.transfer_edit_buffer.trim().to_string();
        match kind {
            TransferEditorKind::ApSsid => {
                if !trimmed.is_empty() {
                    self.transfer_ap_ssid_value = trimmed;
                }
            }
            TransferEditorKind::ApPassword => {
                self.transfer_ap_password_value = trimmed;
            }
        }
        self.pending_wifi_ap_config = Some((
            self.transfer_ap_ssid_value.clone(),
            self.transfer_ap_password_value.clone(),
        ));
        self.transfer_editor = None;
    }

    fn handle_input(&mut self, event: InputEvent) -> ActivityResult {
        if self.transfer_screen_open {
            if self.transfer_editor.is_some() {
                match event {
                    InputEvent::Press(Button::Up) | InputEvent::Press(Button::VolumeUp) => {
                        self.cycle_editor_char(1);
                        return ActivityResult::Consumed;
                    }
                    InputEvent::Press(Button::Down) | InputEvent::Press(Button::VolumeDown) => {
                        self.cycle_editor_char(-1);
                        return ActivityResult::Consumed;
                    }
                    InputEvent::Press(Button::Left) => {
                        self.transfer_edit_cursor = self.transfer_edit_cursor.saturating_sub(1);
                        return ActivityResult::Consumed;
                    }
                    InputEvent::Press(Button::Right) => {
                        if self.transfer_edit_cursor + 1 < self.transfer_edit_buffer.len() {
                            self.transfer_edit_cursor += 1;
                        } else if self.transfer_edit_buffer.len() < 64 {
                            self.transfer_edit_buffer.push(' ');
                            self.transfer_edit_cursor = self.transfer_edit_buffer.len() - 1;
                        }
                        return ActivityResult::Consumed;
                    }
                    InputEvent::Press(Button::Power) => {
                        if !self.transfer_edit_buffer.is_empty()
                            && self.transfer_edit_cursor < self.transfer_edit_buffer.len()
                        {
                            self.transfer_edit_buffer.remove(self.transfer_edit_cursor);
                            if self.transfer_edit_buffer.is_empty() {
                                self.transfer_edit_buffer.push(' ');
                                self.transfer_edit_cursor = 0;
                            } else if self.transfer_edit_cursor >= self.transfer_edit_buffer.len() {
                                self.transfer_edit_cursor =
                                    self.transfer_edit_buffer.len().saturating_sub(1);
                            }
                        }
                        return ActivityResult::Consumed;
                    }
                    InputEvent::Press(Button::Confirm) => {
                        self.commit_transfer_editor();
                        return ActivityResult::Consumed;
                    }
                    InputEvent::Press(Button::Back) => {
                        self.transfer_editor = None;
                        return ActivityResult::Consumed;
                    }
                }
            }
            match event {
                InputEvent::Press(Button::Back) => {
                    self.transfer_screen_open = false;
                    self.transfer_selected = false;
                    self.hero_selected = true;
                    if self.file_transfer_active {
                        self.pending_file_transfer_request = Some(false);
                    }
                    return ActivityResult::Consumed;
                }
                InputEvent::Press(Button::Up) | InputEvent::Press(Button::VolumeUp) => {
                    self.transfer_menu_index = self.transfer_menu_index.saturating_sub(1);
                    return ActivityResult::Consumed;
                }
                InputEvent::Press(Button::Down) | InputEvent::Press(Button::VolumeDown) => {
                    self.transfer_menu_index = (self.transfer_menu_index + 1).min(4);
                    return ActivityResult::Consumed;
                }
                InputEvent::Press(Button::Confirm) => {
                    match self.transfer_menu_index {
                        0 => self.begin_transfer_editor(TransferEditorKind::ApSsid),
                        1 => self.begin_transfer_editor(TransferEditorKind::ApPassword),
                        2 => self.pending_wifi_mode_ap = Some(true),
                        3 => self.pending_wifi_mode_ap = Some(false),
                        4 => self.pending_file_transfer_request = Some(true),
                        _ => {}
                    }
                    return ActivityResult::Consumed;
                }
                _ => return ActivityResult::Consumed,
            }
        }

        match event {
            InputEvent::Press(Button::Up) | InputEvent::Press(Button::VolumeUp) => {
                if self.hero_selected {
                    // already at top row
                } else if self.transfer_selected {
                    if self.books.is_empty() {
                        self.hero_selected = true;
                        self.transfer_selected = false;
                    } else {
                        self.transfer_selected = false;
                        self.selected_index = self.books.len().saturating_sub(1);
                    }
                } else if self.selected_index == 0 {
                    self.hero_selected = true;
                } else {
                    self.selected_index -= 1;
                }
                ActivityResult::Consumed
            }
            InputEvent::Press(Button::Down) | InputEvent::Press(Button::VolumeDown) => {
                if self.hero_selected {
                    if !self.books.is_empty() {
                        self.hero_selected = false;
                        self.transfer_selected = false;
                        self.selected_index = 0;
                    } else {
                        self.hero_selected = false;
                        self.transfer_selected = true;
                    }
                } else if self.transfer_selected {
                    // already at bottom action row
                } else if self.selected_index + 1 < self.books.len() {
                    self.selected_index += 1;
                } else {
                    self.transfer_selected = true;
                }
                ActivityResult::Consumed
            }
            InputEvent::Press(Button::Confirm) => {
                if self.hero_selected {
                    if let Some(idx) = self.currently_reading_index() {
                        self.pending_open_path = Some(self.books[idx].path.clone());
                    }
                } else if self.transfer_selected {
                    self.transfer_screen_open = true;
                    self.pending_file_transfer_request = Some(true);
                } else if let Some(book) = self.books.get(self.selected_index) {
                    self.pending_open_path = Some(book.path.clone());
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

    fn currently_reading_index(&self) -> Option<usize> {
        self.books
            .iter()
            .enumerate()
            .max_by_key(|(_, book)| (book.last_read.unwrap_or(0), book.progress_percent))
            .map(|(idx, _)| idx)
    }

    fn progress_label(progress_percent: u8) -> String {
        match progress_percent {
            0 => String::from("Not started"),
            100 => String::from("Finished"),
            value => format!("{}%", value),
        }
    }

    fn truncate_with_ellipsis(text: &str, max_chars: usize) -> String {
        if max_chars == 0 {
            return String::new();
        }
        let chars: Vec<char> = text.chars().collect();
        if chars.len() <= max_chars {
            return text.to_string();
        }
        if max_chars <= 1 {
            return String::from("…");
        }
        let mut out = String::with_capacity(max_chars);
        for ch in chars.into_iter().take(max_chars - 1) {
            out.push(ch);
        }
        out.push('…');
        out
    }

    fn wrap_to_two_lines(text: &str, max_chars: usize) -> (String, Option<String>) {
        if max_chars == 0 {
            return (String::new(), None);
        }
        let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if normalized.is_empty() {
            return (String::new(), None);
        }
        if normalized.chars().count() <= max_chars {
            return (normalized, None);
        }

        let mut line1 = String::new();
        let mut used_words = 0usize;
        for word in normalized.split(' ') {
            let candidate_len = if line1.is_empty() {
                word.chars().count()
            } else {
                line1.chars().count() + 1 + word.chars().count()
            };
            if candidate_len > max_chars {
                break;
            }
            if !line1.is_empty() {
                line1.push(' ');
            }
            line1.push_str(word);
            used_words += 1;
        }

        if line1.is_empty() {
            line1 = Self::truncate_with_ellipsis(&normalized, max_chars);
            return (line1, None);
        }

        let remainder = normalized
            .split(' ')
            .skip(used_words)
            .collect::<Vec<_>>()
            .join(" ");
        if remainder.is_empty() {
            (line1, None)
        } else {
            (
                line1,
                Some(Self::truncate_with_ellipsis(remainder.trim(), max_chars)),
            )
        }
    }

    fn render_cover_slot<D: DrawTarget<Color = BinaryColor>>(
        display: &mut D,
        book: Option<&BookInfo>,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> Result<(), D::Error> {
        Rectangle::new(Point::new(x, y), Size::new(width, height))
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::Off))
            .draw(display)?;
        Rectangle::new(Point::new(x, y), Size::new(width, height))
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
            .draw(display)?;
        if let Some(book) = book {
            if book.has_cover_thumbnail() {
                let _ =
                    book.draw_cover_thumbnail_scaled(display, x + 1, y + 1, width - 2, height - 2)?;
            } else {
                let icon_w = width.saturating_sub(14).clamp(12, 28) as i32;
                let icon_h = height.saturating_sub(14).clamp(16, 36) as i32;
                let icon_x = x + ((width as i32 - icon_w) / 2);
                let icon_y = y + ((height as i32 - icon_h) / 2);
                Rectangle::new(
                    Point::new(icon_x, icon_y),
                    Size::new(icon_w as u32, icon_h as u32),
                )
                .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
                .draw(display)?;
                Rectangle::new(
                    Point::new(icon_x + 2, icon_y + 2),
                    Size::new((icon_w as u32).saturating_sub(4).max(1), 2),
                )
                .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
                .draw(display)?;
                Rectangle::new(
                    Point::new(icon_x + 3, icon_y + 8),
                    Size::new((icon_w as u32).saturating_sub(6).max(1), 1),
                )
                .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
                .draw(display)?;
                Rectangle::new(
                    Point::new(icon_x + 3, icon_y + 12),
                    Size::new((icon_w as u32).saturating_sub(8).max(1), 1),
                )
                .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
                .draw(display)?;
            }
        }
        Ok(())
    }

    fn render<D: DrawTarget<Color = BinaryColor>>(&self, display: &mut D) -> Result<(), D::Error> {
        let title_style = MonoTextStyle::new(ui_font_title(), BinaryColor::On);
        let body_style = MonoTextStyle::new(ui_font_body(), BinaryColor::On);
        let small_style = MonoTextStyle::new(ui_font_small(), BinaryColor::On);
        let body_h = ui_font_body().character_size.height as i32;
        let small_h = ui_font_small().character_size.height as i32;

        if self.transfer_screen_open {
            Text::new(
                "File Transfer",
                Point::new(MARGIN, HEADER_TEXT_Y),
                title_style,
            )
            .draw(display)?;
            let status = if self.file_transfer_active {
                "Status: Running"
            } else {
                "Status: Stopped"
            };
            Text::new(
                status,
                Point::new(MARGIN, HEADER_TEXT_Y + GAP_LG),
                body_style,
            )
            .draw(display)?;
            Text::new(
                &format!("Mode: {}", self.transfer_mode),
                Point::new(MARGIN, HEADER_TEXT_Y + GAP_LG + body_h + 4),
                small_style,
            )
            .draw(display)?;
            if !self.transfer_ssid.is_empty() {
                Text::new(
                    &format!("SSID: {}", self.transfer_ssid),
                    Point::new(MARGIN, HEADER_TEXT_Y + GAP_LG + body_h + small_h + 8),
                    small_style,
                )
                .draw(display)?;
            }
            if !self.transfer_password_hint.is_empty() {
                Text::new(
                    &self.transfer_password_hint,
                    Point::new(MARGIN, HEADER_TEXT_Y + GAP_LG + body_h + small_h * 2 + 12),
                    small_style,
                )
                .draw(display)?;
            }
            if !self.transfer_url.is_empty() {
                Text::new(
                    &self.transfer_url,
                    Point::new(MARGIN, HEADER_TEXT_Y + GAP_LG + body_h + small_h * 3 + 16),
                    small_style,
                )
                .draw(display)?;
            }
            if !self.transfer_message.is_empty() {
                Text::new(
                    &self.transfer_message,
                    Point::new(MARGIN, HEADER_TEXT_Y + GAP_LG + body_h + small_h * 4 + 20),
                    small_style,
                )
                .draw(display)?;
            }
            Text::new(
                "Open Calibre and use",
                Point::new(MARGIN, HEADER_TEXT_Y + GAP_LG * 5),
                small_style,
            )
            .draw(display)?;
            Text::new(
                "\"Send to device\"",
                Point::new(MARGIN, HEADER_TEXT_Y + GAP_LG * 5 + small_h + 2),
                small_style,
            )
            .draw(display)?;
            Text::new(
                "Keep this screen open.",
                Point::new(MARGIN, HEADER_TEXT_Y + GAP_LG * 6 - 2),
                small_style,
            )
            .draw(display)?;
            Text::new(
                "Back: Exit  Confirm: Select",
                Point::new(MARGIN, DISPLAY_HEIGHT as i32 - BOTTOM_BAR_H - 8),
                small_style,
            )
            .draw(display)?;
            if let Some(kind) = self.transfer_editor {
                let overlay_top = DISPLAY_HEIGHT as i32 - BOTTOM_BAR_H - 92;
                Rectangle::new(
                    Point::new(MARGIN - 2, overlay_top),
                    Size::new(DISPLAY_WIDTH - (MARGIN as u32 * 2) + 4, 84),
                )
                .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
                .draw(display)?;
                let label = match kind {
                    TransferEditorKind::ApSsid => "Edit AP SSID",
                    TransferEditorKind::ApPassword => "Edit AP Password",
                };
                Text::new(label, Point::new(MARGIN + 4, overlay_top + 14), small_style)
                    .draw(display)?;
                Text::new(
                    &self.transfer_edit_buffer,
                    Point::new(MARGIN + 4, overlay_top + 30),
                    body_style,
                )
                .draw(display)?;
                let cursor_x =
                    MARGIN + 4 + (self.transfer_edit_cursor as i32 * ui_font_body_char_width());
                Rectangle::new(
                    Point::new(cursor_x, overlay_top + 34),
                    Size::new(ui_font_body_char_width() as u32, 2),
                )
                .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
                .draw(display)?;
                Text::new(
                    "U/D:Char L/R:Move P:Del C:Save B:Cancel",
                    Point::new(MARGIN + 4, overlay_top + 48),
                    small_style,
                )
                .draw(display)?;
            } else {
                let menu_top = DISPLAY_HEIGHT as i32 - BOTTOM_BAR_H - 102;
                let items = [
                    "Edit AP SSID",
                    "Edit AP Password",
                    "Use Hotspot Mode",
                    "Use Wi-Fi Mode",
                    "Start/Restart",
                ];
                for (idx, item) in items.iter().enumerate() {
                    let y = menu_top + (idx as i32 * (small_h + 3));
                    if idx == self.transfer_menu_index {
                        Rectangle::new(
                            Point::new(MARGIN - 2, y - small_h + 1),
                            Size::new(220, (small_h + 3) as u32),
                        )
                        .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
                        .draw(display)?;
                        let selected_small_style = MonoTextStyleBuilder::new()
                            .font(ui_font_small())
                            .text_color(BinaryColor::Off)
                            .background_color(BinaryColor::On)
                            .build();
                        Text::new(item, Point::new(MARGIN, y), selected_small_style)
                            .draw(display)?;
                    } else {
                        Text::new(item, Point::new(MARGIN, y), small_style).draw(display)?;
                    }
                }
            }
            return Ok(());
        }

        // Title
        Text::new("Library", Point::new(MARGIN, HEADER_TEXT_Y), title_style).draw(display)?;

        // Hero card area
        let hero_y = HEADER_TEXT_Y + GAP_MD;
        let hero_height = HERO_H;
        let hero_w = DISPLAY_WIDTH - (MARGIN as u32 * 2);
        let hero_x = MARGIN;
        let hero_bg = if self.hero_selected {
            BinaryColor::On
        } else {
            BinaryColor::Off
        };
        let hero_fg = if self.hero_selected {
            BinaryColor::Off
        } else {
            BinaryColor::On
        };

        Rectangle::new(
            Point::new(hero_x, hero_y),
            Size::new(hero_w, hero_height as u32),
        )
        .into_styled(PrimitiveStyle::with_fill(hero_bg))
        .draw(display)?;
        Rectangle::new(
            Point::new(hero_x, hero_y),
            Size::new(hero_w, hero_height as u32),
        )
        .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 2))
        .draw(display)?;

        let cover_w = 62u32;
        let cover_h = (hero_height as u32).saturating_sub((INNER_PAD as u32) * 2);
        let cover_x = hero_x + INNER_PAD;
        let cover_y = hero_y + INNER_PAD;

        let current_book = self
            .currently_reading_index()
            .and_then(|idx| self.books.get(idx));
        Self::render_cover_slot(display, current_book, cover_x, cover_y, cover_w, cover_h)?;

        let text_x = cover_x + cover_w as i32 + INNER_PAD;
        let text_right = hero_x + hero_w as i32 - INNER_PAD;
        let text_w_chars = ((text_right - text_x) / ui_font_body_char_width()).max(8) as usize;
        let hero_label_style = MonoTextStyle::new(ui_font_body(), hero_fg);
        let hero_small_style = MonoTextStyle::new(ui_font_small(), hero_fg);
        Text::new(
            "Currently Reading",
            Point::new(text_x, hero_y + INNER_PAD + body_h),
            hero_label_style,
        )
        .draw(display)?;

        if self.is_loading {
            Text::new(
                "Loading...",
                Point::new(text_x, hero_y + INNER_PAD + body_h + GAP_MD),
                hero_small_style,
            )
            .draw(display)?;
        } else if current_book.is_none() {
            Text::new(
                "No book in progress",
                Point::new(text_x, hero_y + INNER_PAD + body_h + GAP_MD),
                hero_small_style,
            )
            .draw(display)?;
        } else if let Some(book) = current_book {
            let (line1, line2) = Self::wrap_to_two_lines(&book.title, text_w_chars);
            let title_y = hero_y + INNER_PAD + body_h + GAP_MD;
            Text::new(&line1, Point::new(text_x, title_y), hero_small_style).draw(display)?;
            if let Some(line2) = line2.as_ref() {
                Text::new(
                    line2,
                    Point::new(text_x, title_y + small_h + 2),
                    hero_small_style,
                )
                .draw(display)?;
            }
            if !book.author.is_empty() {
                let author = Self::truncate_with_ellipsis(&book.author, text_w_chars);
                Text::new(
                    &author,
                    Point::new(text_x, hero_y + hero_height - (small_h * 2) - 4),
                    hero_small_style,
                )
                .draw(display)?;
            }
            let progress_text = Self::progress_label(book.progress_percent);
            Text::new(
                &progress_text,
                Point::new(text_x, hero_y + hero_height - 4),
                hero_small_style,
            )
            .draw(display)?;
        }

        // Library section
        let list_y = hero_y + hero_height + GAP_MD;
        Text::new("Your Books", Point::new(MARGIN, list_y), title_style).draw(display)?;

        if self.is_loading {
            Text::new(
                "Scanning...",
                Point::new(MARGIN, list_y + GAP_LG),
                small_style,
            )
            .draw(display)?;
            let transfer_y = DISPLAY_HEIGHT as i32 - BOTTOM_BAR_H - (small_h + 10);
            if self.transfer_selected {
                Rectangle::new(
                    Point::new(MARGIN - SELECT_PAD_X, transfer_y - body_h),
                    Size::new(
                        DISPLAY_WIDTH - (MARGIN as u32 * 2) + (SELECT_PAD_X as u32 * 2),
                        (body_h + small_h + 10) as u32,
                    ),
                )
                .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
                .draw(display)?;
                let selected_style = MonoTextStyleBuilder::new()
                    .font(ui_font_body())
                    .text_color(BinaryColor::Off)
                    .background_color(BinaryColor::On)
                    .build();
                let selected_small_style = MonoTextStyleBuilder::new()
                    .font(ui_font_small())
                    .text_color(BinaryColor::Off)
                    .background_color(BinaryColor::On)
                    .build();
                Text::new(
                    "File Transfer",
                    Point::new(MARGIN, transfer_y),
                    selected_style,
                )
                .draw(display)?;
                Text::new(
                    if self.file_transfer_active {
                        "Running"
                    } else {
                        "Open"
                    },
                    Point::new(MARGIN, transfer_y + small_h + 1),
                    selected_small_style,
                )
                .draw(display)?;
            } else {
                Text::new("File Transfer", Point::new(MARGIN, transfer_y), body_style)
                    .draw(display)?;
                Text::new(
                    if self.file_transfer_active {
                        "Running"
                    } else {
                        "Open"
                    },
                    Point::new(MARGIN, transfer_y + small_h + 1),
                    small_style,
                )
                .draw(display)?;
            }
        } else if self.books.is_empty() {
            Text::new(
                "No books found",
                Point::new(MARGIN, list_y + GAP_LG),
                small_style,
            )
            .draw(display)?;
            let transfer_y = DISPLAY_HEIGHT as i32 - BOTTOM_BAR_H - (small_h + 10);
            if self.transfer_selected {
                Rectangle::new(
                    Point::new(MARGIN - SELECT_PAD_X, transfer_y - body_h),
                    Size::new(
                        DISPLAY_WIDTH - (MARGIN as u32 * 2) + (SELECT_PAD_X as u32 * 2),
                        (body_h + small_h + 10) as u32,
                    ),
                )
                .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
                .draw(display)?;
                let selected_style = MonoTextStyleBuilder::new()
                    .font(ui_font_body())
                    .text_color(BinaryColor::Off)
                    .background_color(BinaryColor::On)
                    .build();
                let selected_small_style = MonoTextStyleBuilder::new()
                    .font(ui_font_small())
                    .text_color(BinaryColor::Off)
                    .background_color(BinaryColor::On)
                    .build();
                Text::new(
                    "File Transfer",
                    Point::new(MARGIN, transfer_y),
                    selected_style,
                )
                .draw(display)?;
                Text::new(
                    if self.file_transfer_active {
                        "Running"
                    } else {
                        "Open"
                    },
                    Point::new(MARGIN, transfer_y + small_h + 1),
                    selected_small_style,
                )
                .draw(display)?;
            } else {
                Text::new("File Transfer", Point::new(MARGIN, transfer_y), body_style)
                    .draw(display)?;
                Text::new(
                    if self.file_transfer_active {
                        "Running"
                    } else {
                        "Open"
                    },
                    Point::new(MARGIN, transfer_y + small_h + 1),
                    small_style,
                )
                .draw(display)?;
            }
        } else {
            let item_h = (body_h + small_h + INNER_PAD + 8).max(40);
            let start_y = list_y + body_h + 2;
            let bottom_limit = DISPLAY_HEIGHT as i32 - BOTTOM_BAR_H - INNER_PAD;
            let max_items = ((bottom_limit - start_y) / item_h).max(2) as usize;
            let book_rows = max_items.saturating_sub(1);
            let selected = if self.hero_selected {
                0
            } else {
                self.selected_index.min(self.books.len().saturating_sub(1))
            };
            let scroll_offset = selected.saturating_sub(book_rows / 2);
            for (row, idx) in (scroll_offset..self.books.len())
                .take(book_rows)
                .enumerate()
            {
                let y = start_y + (row as i32) * item_h;
                let book = &self.books[idx];
                let is_selected =
                    !self.hero_selected && !self.transfer_selected && idx == self.selected_index;
                if is_selected {
                    Rectangle::new(
                        Point::new(MARGIN - SELECT_PAD_X, y - body_h),
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
                    let selected_small_style = MonoTextStyleBuilder::new()
                        .font(ui_font_small())
                        .text_color(BinaryColor::Off)
                        .background_color(BinaryColor::On)
                        .build();
                    let row_text_x = MARGIN + 4;
                    let row_right = DISPLAY_WIDTH as i32 - MARGIN;
                    let progress_text = Self::progress_label(book.progress_percent);
                    let progress_w = progress_text.len() as i32 * ui_font_small_char_width();
                    let title_max_chars = ((row_right - row_text_x - progress_w - 6)
                        / ui_font_body_char_width())
                    .max(6) as usize;
                    let title = Self::truncate_with_ellipsis(&book.title, title_max_chars);
                    Text::new(&title, Point::new(row_text_x, y), selected_style).draw(display)?;
                    let author = if book.author.is_empty() {
                        String::from("Unknown")
                    } else {
                        Self::truncate_with_ellipsis(&book.author, title_max_chars)
                    };
                    Text::new(
                        &author,
                        Point::new(row_text_x, y + small_h + 1),
                        selected_small_style,
                    )
                    .draw(display)?;
                    Text::new(
                        &progress_text,
                        Point::new(row_right - progress_w, y + small_h + 1),
                        selected_small_style,
                    )
                    .draw(display)?;
                } else {
                    let row_text_x = MARGIN + 4;
                    let row_right = DISPLAY_WIDTH as i32 - MARGIN;
                    let progress_text = Self::progress_label(book.progress_percent);
                    let progress_w = progress_text.len() as i32 * ui_font_small_char_width();
                    let title_max_chars = ((row_right - row_text_x - progress_w - 6)
                        / ui_font_body_char_width())
                    .max(6) as usize;
                    let title = Self::truncate_with_ellipsis(&book.title, title_max_chars);
                    let author = if book.author.is_empty() {
                        String::from("Unknown")
                    } else {
                        Self::truncate_with_ellipsis(&book.author, title_max_chars)
                    };
                    Text::new(&title, Point::new(row_text_x, y), body_style).draw(display)?;
                    Text::new(
                        &author,
                        Point::new(row_text_x, y + small_h + 1),
                        small_style,
                    )
                    .draw(display)?;
                    Text::new(
                        &progress_text,
                        Point::new(row_right - progress_w, y + small_h + 1),
                        small_style,
                    )
                    .draw(display)?;
                }
            }

            let transfer_y = start_y + (book_rows as i32) * item_h;
            let transfer_selected = self.transfer_selected;
            if transfer_selected {
                Rectangle::new(
                    Point::new(MARGIN - SELECT_PAD_X, transfer_y - body_h),
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
                let selected_small_style = MonoTextStyleBuilder::new()
                    .font(ui_font_small())
                    .text_color(BinaryColor::Off)
                    .background_color(BinaryColor::On)
                    .build();
                Text::new(
                    "File Transfer",
                    Point::new(MARGIN, transfer_y),
                    selected_style,
                )
                .draw(display)?;
                let action = if self.file_transfer_active {
                    "Running"
                } else {
                    "Open"
                };
                Text::new(
                    action,
                    Point::new(MARGIN, transfer_y + small_h + 1),
                    selected_small_style,
                )
                .draw(display)?;
            } else {
                Text::new("File Transfer", Point::new(MARGIN, transfer_y), body_style)
                    .draw(display)?;
                let action = if self.file_transfer_active {
                    "Running"
                } else {
                    "Open"
                };
                Text::new(
                    action,
                    Point::new(MARGIN, transfer_y + small_h + 1),
                    small_style,
                )
                .draw(display)?;
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

    pub fn is_reading_image(&self) -> bool {
        self.file_browser.is_viewing_image()
    }

    pub fn is_reading_epub(&self) -> bool {
        self.file_browser.is_viewing_epub()
    }

    pub fn is_reading(&self) -> bool {
        self.is_reading_text() || self.is_reading_image() || self.is_reading_epub()
    }

    pub fn has_pending_task(&self) -> bool {
        self.file_browser.has_pending_task()
    }

    pub fn epub_position(&self) -> Option<(usize, usize, usize, usize)> {
        self.file_browser.epub_position()
    }

    pub fn epub_book_progress_percent(&self) -> Option<u8> {
        self.file_browser.epub_book_progress_percent()
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
