//! Reader Settings Activity for EPUB reading preferences.
//!
//! Provides comprehensive reading configuration including font settings,
//! margins, layout options, display preferences, and navigation controls.
//! Optimized for e-ink displays with minimal, high-contrast UI.

extern crate alloc;

use alloc::format;
use alloc::string::String;

use embedded_graphics::{
    mono_font::{ascii, MonoTextStyle, MonoTextStyleBuilder},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    text::Text,
};

use crate::input::{Button, InputEvent};
use crate::settings_activity::{FontFamily, FontSize};
use crate::ui::{Activity, ActivityResult, Modal, Theme, ThemeMetrics, Toast};

/// Line spacing options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineSpacing {
    Compact,
    #[default]
    Normal,
    Relaxed,
}

impl LineSpacing {
    /// All line spacing variants
    pub const ALL: [Self; 3] = [Self::Compact, Self::Normal, Self::Relaxed];

    /// Get display label
    pub const fn label(self) -> &'static str {
        match self {
            Self::Compact => "Compact",
            Self::Normal => "Normal",
            Self::Relaxed => "Relaxed",
        }
    }

    /// Get index in ALL array
    pub const fn index(self) -> usize {
        match self {
            Self::Compact => 0,
            Self::Normal => 1,
            Self::Relaxed => 2,
        }
    }

    /// Create from index
    pub const fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::Compact),
            1 => Some(Self::Normal),
            2 => Some(Self::Relaxed),
            _ => None,
        }
    }

    pub const fn next_wrapped(self) -> Self {
        match self {
            Self::Compact => Self::Normal,
            Self::Normal => Self::Relaxed,
            Self::Relaxed => Self::Compact,
        }
    }

    pub const fn prev_wrapped(self) -> Self {
        match self {
            Self::Compact => Self::Relaxed,
            Self::Normal => Self::Compact,
            Self::Relaxed => Self::Normal,
        }
    }

    /// Get line height multiplier (in tenths)
    pub const fn multiplier(self) -> u8 {
        match self {
            Self::Compact => 12, // 1.2x
            Self::Normal => 15,  // 1.5x
            Self::Relaxed => 20, // 2.0x
        }
    }
}

/// Margin size options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MarginSize {
    Small,
    #[default]
    Medium,
    Large,
}

impl MarginSize {
    /// All margin size variants
    pub const ALL: [Self; 3] = [Self::Small, Self::Medium, Self::Large];

    /// Get display label
    pub const fn label(self) -> &'static str {
        match self {
            Self::Small => "Small",
            Self::Medium => "Medium",
            Self::Large => "Large",
        }
    }

    /// Get index in ALL array
    pub const fn index(self) -> usize {
        match self {
            Self::Small => 0,
            Self::Medium => 1,
            Self::Large => 2,
        }
    }

    /// Create from index
    pub const fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::Small),
            1 => Some(Self::Medium),
            2 => Some(Self::Large),
            _ => None,
        }
    }

    pub const fn next_wrapped(self) -> Self {
        match self {
            Self::Small => Self::Medium,
            Self::Medium => Self::Large,
            Self::Large => Self::Small,
        }
    }

    pub const fn prev_wrapped(self) -> Self {
        match self {
            Self::Small => Self::Large,
            Self::Medium => Self::Small,
            Self::Large => Self::Medium,
        }
    }

    /// Get margin size in pixels
    pub const fn pixels(self) -> u32 {
        match self {
            Self::Small => 20,
            Self::Medium => 40,
            Self::Large => 60,
        }
    }
}

/// Text alignment options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAlignment {
    Left,
    #[default]
    Justified,
}

impl TextAlignment {
    /// All alignment variants
    pub const ALL: [Self; 2] = [Self::Left, Self::Justified];

    /// Get display label
    pub const fn label(self) -> &'static str {
        match self {
            Self::Left => "Left",
            Self::Justified => "Justified",
        }
    }

    /// Get index in ALL array
    pub const fn index(self) -> usize {
        match self {
            Self::Left => 0,
            Self::Justified => 1,
        }
    }

    /// Create from index
    pub const fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::Left),
            1 => Some(Self::Justified),
            _ => None,
        }
    }

    pub const fn next_wrapped(self) -> Self {
        match self {
            Self::Left => Self::Justified,
            Self::Justified => Self::Left,
        }
    }

    pub const fn prev_wrapped(self) -> Self {
        self.next_wrapped()
    }
}

/// Full refresh frequency options (in pages)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RefreshFrequency {
    Every1,
    Every5,
    #[default]
    Every10,
    Every15,
    Every30,
}

impl RefreshFrequency {
    /// All refresh frequency variants
    pub const ALL: [Self; 5] = [
        Self::Every1,
        Self::Every5,
        Self::Every10,
        Self::Every15,
        Self::Every30,
    ];

    /// Get display label
    pub const fn label(self) -> &'static str {
        match self {
            Self::Every1 => "Every 1 page",
            Self::Every5 => "Every 5 pages",
            Self::Every10 => "Every 10 pages",
            Self::Every15 => "Every 15 pages",
            Self::Every30 => "Every 30 pages",
        }
    }

    /// Get index in ALL array
    pub const fn index(self) -> usize {
        match self {
            Self::Every1 => 0,
            Self::Every5 => 1,
            Self::Every10 => 2,
            Self::Every15 => 3,
            Self::Every30 => 4,
        }
    }

    /// Create from index
    pub const fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::Every1),
            1 => Some(Self::Every5),
            2 => Some(Self::Every10),
            3 => Some(Self::Every15),
            4 => Some(Self::Every30),
            _ => None,
        }
    }

    pub const fn next_wrapped(self) -> Self {
        match self {
            Self::Every1 => Self::Every5,
            Self::Every5 => Self::Every10,
            Self::Every10 => Self::Every15,
            Self::Every15 => Self::Every30,
            Self::Every30 => Self::Every1,
        }
    }

    pub const fn prev_wrapped(self) -> Self {
        match self {
            Self::Every1 => Self::Every30,
            Self::Every5 => Self::Every1,
            Self::Every10 => Self::Every5,
            Self::Every15 => Self::Every10,
            Self::Every30 => Self::Every15,
        }
    }

    /// Get number of pages between full refreshes
    pub const fn pages(self) -> u8 {
        match self {
            Self::Every1 => 1,
            Self::Every5 => 5,
            Self::Every10 => 10,
            Self::Every15 => 15,
            Self::Every30 => 30,
        }
    }
}

/// Tap zone configuration for page turning
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TapZoneConfig {
    #[default]
    LeftNext, // Tap left side for next page (default)
    RightNext, // Tap right side for next page
}

impl TapZoneConfig {
    /// All tap zone configurations
    pub const ALL: [Self; 2] = [Self::LeftNext, Self::RightNext];

    /// Get display label
    pub const fn label(self) -> &'static str {
        match self {
            Self::LeftNext => "Left = Next",
            Self::RightNext => "Right = Next",
        }
    }

    /// Get index in ALL array
    pub const fn index(self) -> usize {
        match self {
            Self::LeftNext => 0,
            Self::RightNext => 1,
        }
    }

    /// Create from index
    pub const fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::LeftNext),
            1 => Some(Self::RightNext),
            _ => None,
        }
    }

    pub const fn next_wrapped(self) -> Self {
        match self {
            Self::LeftNext => Self::RightNext,
            Self::RightNext => Self::LeftNext,
        }
    }

    pub const fn prev_wrapped(self) -> Self {
        self.next_wrapped()
    }
}

/// Volume button actions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VolumeButtonAction {
    PageTurn,
    #[default]
    Scroll,
}

impl VolumeButtonAction {
    /// All volume button actions
    pub const ALL: [Self; 2] = [Self::PageTurn, Self::Scroll];

    /// Get display label
    pub const fn label(self) -> &'static str {
        match self {
            Self::PageTurn => "Page Turn",
            Self::Scroll => "Scroll",
        }
    }

    /// Get index in ALL array
    pub const fn index(self) -> usize {
        match self {
            Self::PageTurn => 0,
            Self::Scroll => 1,
        }
    }

    /// Create from index
    pub const fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::PageTurn),
            1 => Some(Self::Scroll),
            _ => None,
        }
    }

    pub const fn next_wrapped(self) -> Self {
        match self {
            Self::PageTurn => Self::Scroll,
            Self::Scroll => Self::PageTurn,
        }
    }

    pub const fn prev_wrapped(self) -> Self {
        self.next_wrapped()
    }
}

/// Reader settings data container
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReaderSettings {
    // Font Settings
    pub font_size: FontSize,
    pub font_family: FontFamily,
    pub line_spacing: LineSpacing,
    // Margins & Layout
    pub margin_size: MarginSize,
    pub text_alignment: TextAlignment,
    pub show_page_numbers: bool,
    // Display
    pub refresh_frequency: RefreshFrequency,
    pub invert_colors: bool,
    // Navigation
    pub tap_zone_config: TapZoneConfig,
    pub volume_button_action: VolumeButtonAction,
}

impl Default for ReaderSettings {
    fn default() -> Self {
        Self {
            font_size: FontSize::default(),
            font_family: FontFamily::Serif,
            line_spacing: LineSpacing::default(),
            margin_size: MarginSize::default(),
            text_alignment: TextAlignment::default(),
            show_page_numbers: true,
            refresh_frequency: RefreshFrequency::default(),
            invert_colors: false,
            tap_zone_config: TapZoneConfig::default(),
            volume_button_action: VolumeButtonAction::default(),
        }
    }
}

impl ReaderSettings {
    /// Reset to factory defaults
    pub fn reset_to_defaults(&mut self) {
        *self = Self::default();
    }
}

/// Setting item types for the settings list
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingItem {
    FontSize,
    FontFamily,
    LineSpacing,
    MarginSize,
    TextAlignment,
    ShowPageNumbers,
    RefreshFrequency,
    InvertColors,
    TapZoneConfig,
    VolumeButtonAction,
    SaveButton,
}

impl SettingItem {
    /// All setting items in display order
    pub const ALL: [Self; 11] = [
        Self::FontSize,
        Self::FontFamily,
        Self::LineSpacing,
        Self::MarginSize,
        Self::TextAlignment,
        Self::ShowPageNumbers,
        Self::RefreshFrequency,
        Self::InvertColors,
        Self::TapZoneConfig,
        Self::VolumeButtonAction,
        Self::SaveButton,
    ];

    /// Get the section this item belongs to
    pub const fn section(self) -> &'static str {
        match self {
            Self::FontSize | Self::FontFamily | Self::LineSpacing => "Font",
            Self::MarginSize | Self::TextAlignment | Self::ShowPageNumbers => "Margins & Layout",
            Self::RefreshFrequency | Self::InvertColors => "Display",
            Self::TapZoneConfig | Self::VolumeButtonAction => "Navigation",
            Self::SaveButton => "",
        }
    }

    /// Get the label for this setting
    pub const fn label(self) -> &'static str {
        match self {
            Self::FontSize => "Size",
            Self::FontFamily => "Family",
            Self::LineSpacing => "Spacing",
            Self::MarginSize => "Margins",
            Self::TextAlignment => "Alignment",
            Self::ShowPageNumbers => "Page Numbers",
            Self::RefreshFrequency => "Refresh",
            Self::InvertColors => "Invert",
            Self::TapZoneConfig => "Tap Zone",
            Self::VolumeButtonAction => "Volume Keys",
            Self::SaveButton => "Save Changes",
        }
    }

    /// Check if this is a toggle setting (checkbox)
    pub const fn is_toggle(self) -> bool {
        matches!(self, Self::ShowPageNumbers | Self::InvertColors)
    }

    /// Check if this is the save button
    pub const fn is_save(self) -> bool {
        matches!(self, Self::SaveButton)
    }
}

/// Section header height in pixels
const SECTION_HEADER_HEIGHT: i32 = 25;

/// Reader Settings Activity implementing the Activity trait
#[derive(Debug, Clone)]
pub struct ReaderSettingsActivity {
    settings: ReaderSettings,
    original_settings: ReaderSettings,
    selected_index: usize,
    scroll_offset: usize,
    visible_count: usize,
    show_toast: bool,
    toast_message: String,
    toast_frames_remaining: u32,
    show_cancel_modal: bool,
    /// Tracks which button is selected in the cancel modal (0=Keep, 1=Discard)
    modal_button: usize,
    theme: Theme,
}

impl ReaderSettingsActivity {
    /// Toast display duration in frames
    const TOAST_DURATION: u32 = 120; // ~2 seconds at 60fps

    /// Create a new reader settings activity with defaults
    pub fn new() -> Self {
        let settings = ReaderSettings::default();
        let theme = Theme::default();
        // Calculate how many items fit on screen
        let visible_count = Self::calculate_visible_count(&theme);
        Self {
            settings,
            original_settings: settings,
            selected_index: 0,
            scroll_offset: 0,
            visible_count,
            show_toast: false,
            toast_message: String::new(),
            toast_frames_remaining: 0,
            show_cancel_modal: false,
            modal_button: 0,
            theme,
        }
    }

    /// Calculate how many setting items fit on screen
    fn calculate_visible_count(theme: &Theme) -> usize {
        use crate::DISPLAY_HEIGHT;
        let content_height = DISPLAY_HEIGHT.saturating_sub(
            theme.metrics.header_height + theme.metrics.spacing_double()
        );
        let item_height = theme.metrics.list_item_height + SECTION_HEADER_HEIGHT as u32;
        (content_height / item_height) as usize
    }

    /// Create with specific initial settings
    pub fn with_settings(settings: ReaderSettings) -> Self {
        let theme = Theme::default();
        let visible_count = Self::calculate_visible_count(&theme);
        Self {
            settings,
            original_settings: settings,
            selected_index: 0,
            scroll_offset: 0,
            visible_count,
            show_toast: false,
            toast_message: String::new(),
            toast_frames_remaining: 0,
            show_cancel_modal: false,
            modal_button: 0,
            theme,
        }
    }

    /// Get current settings
    pub fn settings(&self) -> &ReaderSettings {
        &self.settings
    }

    /// Get original settings (before any modifications)
    pub fn original_settings(&self) -> &ReaderSettings {
        &self.original_settings
    }

    /// Check if settings were modified
    pub fn is_modified(&self) -> bool {
        self.settings != self.original_settings
    }

    /// Save settings and return to reader
    fn save_settings(&mut self) {
        self.show_toast("Settings saved");
    }

    /// Cancel changes and return to reader
    fn cancel_changes(&mut self) {
        if self.is_modified() {
            self.show_cancel_modal = true;
            self.modal_button = 0; // Start on Keep (safe default)
        }
    }

    /// Confirm cancel - discard changes
    fn confirm_cancel(&mut self) {
        self.settings = self.original_settings;
        self.show_cancel_modal = false;
        self.modal_button = 0;
    }

    /// Dismiss cancel modal
    fn dismiss_cancel(&mut self) {
        self.show_cancel_modal = false;
        self.modal_button = 0;
    }

    /// Show a toast notification
    fn show_toast(&mut self, message: impl Into<String>) {
        self.toast_message = message.into();
        self.show_toast = true;
        self.toast_frames_remaining = Self::TOAST_DURATION;
    }

    /// Update toast state (call once per frame)
    pub fn update(&mut self) {
        if self.show_toast && self.toast_frames_remaining > 0 {
            self.toast_frames_remaining -= 1;
            if self.toast_frames_remaining == 0 {
                self.show_toast = false;
            }
        }
    }

    /// Get currently selected item
    fn current_item(&self) -> SettingItem {
        SettingItem::ALL[self.selected_index]
    }

    /// Move selection to next item (wraps)
    fn select_next(&mut self) {
        self.selected_index = (self.selected_index + 1) % SettingItem::ALL.len();
        self.ensure_visible();
    }

    /// Move selection to previous item (wraps)
    fn select_prev(&mut self) {
        if self.selected_index == 0 {
            self.selected_index = SettingItem::ALL.len() - 1;
        } else {
            self.selected_index -= 1;
        }
        self.ensure_visible();
    }

    /// Ensure selected item is visible by adjusting scroll offset
    fn ensure_visible(&mut self) {
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        } else if self.selected_index >= self.scroll_offset + self.visible_count {
            self.scroll_offset = self.selected_index.saturating_sub(self.visible_count - 1);
        }
    }

    /// Handle confirm press on current item
    fn handle_confirm(&mut self) -> ActivityResult {
        match self.current_item() {
            SettingItem::FontSize => {
                if let Some(next) = self.settings.font_size.next() {
                    self.settings.font_size = next;
                    self.show_toast(format!("Font size: {}", next.label()));
                }
            }
            SettingItem::FontFamily => {
                self.settings.font_family = self.settings.font_family.next_wrapped();
                self.show_toast(format!("Font: {}", self.settings.font_family.label()));
            }
            SettingItem::LineSpacing => {
                self.settings.line_spacing = self.settings.line_spacing.next_wrapped();
                self.show_toast(format!("Spacing: {}", self.settings.line_spacing.label()));
            }
            SettingItem::MarginSize => {
                self.settings.margin_size = self.settings.margin_size.next_wrapped();
                self.show_toast(format!("Margins: {}", self.settings.margin_size.label()));
            }
            SettingItem::TextAlignment => {
                self.settings.text_alignment = self.settings.text_alignment.next_wrapped();
                self.show_toast(format!(
                    "Alignment: {}",
                    self.settings.text_alignment.label()
                ));
            }
            SettingItem::ShowPageNumbers => {
                self.settings.show_page_numbers = !self.settings.show_page_numbers;
                let status = if self.settings.show_page_numbers {
                    "On"
                } else {
                    "Off"
                };
                self.show_toast(format!("Page numbers: {}", status));
            }
            SettingItem::RefreshFrequency => {
                self.settings.refresh_frequency = self.settings.refresh_frequency.next_wrapped();
                self.show_toast(format!(
                    "Refresh: {}",
                    self.settings.refresh_frequency.label()
                ));
            }
            SettingItem::InvertColors => {
                self.settings.invert_colors = !self.settings.invert_colors;
                let status = if self.settings.invert_colors {
                    "On"
                } else {
                    "Off"
                };
                self.show_toast(format!("Invert: {}", status));
            }
            SettingItem::TapZoneConfig => {
                self.settings.tap_zone_config = self.settings.tap_zone_config.next_wrapped();
                self.show_toast(format!("Tap: {}", self.settings.tap_zone_config.label()));
            }
            SettingItem::VolumeButtonAction => {
                self.settings.volume_button_action =
                    self.settings.volume_button_action.next_wrapped();
                self.show_toast(format!(
                    "Volume: {}",
                    self.settings.volume_button_action.label()
                ));
            }
            SettingItem::SaveButton => {
                self.save_settings();
                return ActivityResult::NavigateBack;
            }
        }
        ActivityResult::Consumed
    }

    /// Get current value label for a setting item
    fn get_value_label(&self, item: SettingItem) -> String {
        match item {
            SettingItem::FontSize => self.settings.font_size.label().into(),
            SettingItem::FontFamily => self.settings.font_family.label().into(),
            SettingItem::LineSpacing => self.settings.line_spacing.label().into(),
            SettingItem::MarginSize => self.settings.margin_size.label().into(),
            SettingItem::TextAlignment => self.settings.text_alignment.label().into(),
            SettingItem::ShowPageNumbers => {
                if self.settings.show_page_numbers {
                    "[✓]".into()
                } else {
                    "[ ]".into()
                }
            }
            SettingItem::RefreshFrequency => self.settings.refresh_frequency.label().into(),
            SettingItem::InvertColors => {
                if self.settings.invert_colors {
                    "[✓]".into()
                } else {
                    "[ ]".into()
                }
            }
            SettingItem::TapZoneConfig => self.settings.tap_zone_config.label().into(),
            SettingItem::VolumeButtonAction => self.settings.volume_button_action.label().into(),
            SettingItem::SaveButton => String::new(),
        }
    }

    /// Render header bar
    fn render_header<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
        theme: &Theme,
    ) -> Result<(), D::Error> {
        use crate::ui::Header;

        let header = Header::new("Reader Settings");
        header.render(display, theme)
    }

    /// Render settings list
    fn render_settings_list<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
        theme: &Theme,
    ) -> Result<(), D::Error> {
        use crate::ui::theme::{ui_font, ui_font_bold};

        let display_width = display.bounding_box().size.width;
        let content_width = theme.metrics.content_width(display_width);
        let x = theme.metrics.side_padding as i32;
        let mut y = theme.metrics.header_height as i32 + theme.metrics.spacing_double() as i32;

        let mut last_section = "";

        // Only render visible items based on scroll offset
        for (i, item) in SettingItem::ALL.iter()
            .enumerate()
            .skip(self.scroll_offset)
            .take(self.visible_count)
        {
            let item = *item;

            // Render section header if new section
            let section = item.section();
            if !section.is_empty() && section != last_section {
                if !last_section.is_empty() {
                    y += theme.metrics.spacing as i32; // Extra spacing between sections
                }
                self.render_section_header(display, x, y, section)?;
                y += SECTION_HEADER_HEIGHT;
                last_section = section;
            }

            let is_selected = i == self.selected_index;
            let item_height = if item.is_save() {
                theme.metrics.button_height
            } else {
                theme.metrics.list_item_height
            };
            let text_y = ThemeMetrics::text_y_offset(item_height);

            // Background
            let bg_color = if is_selected {
                BinaryColor::On
            } else {
                BinaryColor::Off
            };
            Rectangle::new(Point::new(x, y), Size::new(content_width, item_height))
                .into_styled(PrimitiveStyle::with_fill(bg_color))
                .draw(display)?;

            // Border for save button
            if item.is_save() {
                Rectangle::new(Point::new(x, y), Size::new(content_width, item_height))
                    .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
                    .draw(display)?;
            }

            // Text color
            let text_color = if is_selected {
                BinaryColor::Off
            } else {
                BinaryColor::On
            };

            if item.is_save() {
                // Center the save button text
                let label = item.label();
                let label_width = ThemeMetrics::text_width(label.len());
                let label_x = x + (content_width as i32 - label_width) / 2;
                Text::new(
                    label,
                    Point::new(label_x, y + text_y),
                    MonoTextStyle::new(ui_font_bold(), text_color),
                )
                .draw(display)?;
            } else {
                // Label on left
                Text::new(
                    item.label(),
                    Point::new(x + theme.metrics.side_padding as i32, y + text_y),
                    MonoTextStyle::new(ui_font(), text_color),
                )
                .draw(display)?;

                // Value on right with [>] indicator
                let value_label = self.get_value_label(item);
                let value_text = if item.is_toggle() {
                    value_label
                } else {
                    format!("{} >", value_label)
                };
                let value_width = ThemeMetrics::text_width(value_text.len());
                let value_x =
                    x + content_width as i32 - value_width - theme.metrics.side_padding as i32;

                Text::new(
                    &value_text,
                    Point::new(value_x, y + text_y),
                    MonoTextStyle::new(ui_font(), text_color),
                )
                .draw(display)?;
            }

            y += item_height as i32;
        }

        Ok(())
    }

    /// Render section header
    fn render_section_header<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
        x: i32,
        y: i32,
        title: &str,
    ) -> Result<(), D::Error> {
        use crate::ui::theme::ui_font_bold;

        let title_style = MonoTextStyleBuilder::new()
            .font(ui_font_bold())
            .text_color(BinaryColor::On)
            .build();

        Text::new(title, Point::new(x, y + 15), title_style).draw(display)?;

        Ok(())
    }
}

impl Activity for ReaderSettingsActivity {
    fn on_enter(&mut self) {
        self.original_settings = self.settings;
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.show_toast = false;
        self.show_cancel_modal = false;
        self.modal_button = 0;
    }

    fn on_exit(&mut self) {
        // Settings are persisted in memory
    }

    fn handle_input(&mut self, event: InputEvent) -> ActivityResult {
        if self.show_cancel_modal {
            return self.handle_modal_input(event);
        }

        match event {
            InputEvent::Press(Button::Back) => {
                if self.is_modified() {
                    self.cancel_changes();
                    ActivityResult::Consumed
                } else {
                    ActivityResult::NavigateBack
                }
            }
            InputEvent::Press(Button::VolumeUp) | InputEvent::Press(Button::Up) => {
                self.select_prev();
                ActivityResult::Consumed
            }
            InputEvent::Press(Button::VolumeDown) | InputEvent::Press(Button::Down) => {
                self.select_next();
                ActivityResult::Consumed
            }
            InputEvent::Press(Button::Confirm) | InputEvent::Press(Button::Right) => {
                self.handle_confirm()
            }
            InputEvent::Press(Button::Left) => {
                // Cycle backwards on some settings
                self.handle_left_press()
            }
            _ => ActivityResult::Ignored,
        }
    }

    fn render<D: DrawTarget<Color = BinaryColor>>(&self, display: &mut D) -> Result<(), D::Error> {
        // Clear background
        Rectangle::new(
            Point::new(0, 0),
            Size::new(
                display.bounding_box().size.width,
                display.bounding_box().size.height,
            ),
        )
        .into_styled(PrimitiveStyle::with_fill(BinaryColor::Off))
        .draw(display)?;

        // Header
        self.render_header(display, &self.theme)?;

        // Settings list
        self.render_settings_list(display, &self.theme)?;

        // Toast notification
        if self.show_toast {
            let display_width = display.bounding_box().size.width;
            let display_height = display.bounding_box().size.height;
            let toast = Toast::bottom_center(&self.toast_message, display_width, display_height);
            toast.render(display)?;
        }

        // Cancel modal dialog — use tracked modal_button for selection
        if self.show_cancel_modal {
            let mut modal = Modal::new("Discard Changes?", "You have unsaved changes.")
                .with_button("Keep")
                .with_button("Discard");
            modal.selected_button = self.modal_button;
            modal.render(display, &self.theme)?;
        }

        Ok(())
    }
}

impl ReaderSettingsActivity {
    /// Handle input when modal is shown.
    /// Left/Right cycle buttons, Confirm executes selected, Back cancels.
    fn handle_modal_input(&mut self, event: InputEvent) -> ActivityResult {
        match event {
            InputEvent::Press(Button::Left) | InputEvent::Press(Button::VolumeUp) => {
                if self.modal_button > 0 {
                    self.modal_button -= 1;
                } else {
                    self.modal_button = 1;
                }
                ActivityResult::Consumed
            }
            InputEvent::Press(Button::Right) | InputEvent::Press(Button::VolumeDown) => {
                self.modal_button = (self.modal_button + 1) % 2;
                ActivityResult::Consumed
            }
            InputEvent::Press(Button::Confirm) => {
                if self.modal_button == 1 {
                    // Discard
                    self.confirm_cancel();
                    ActivityResult::NavigateBack
                } else {
                    // Keep
                    self.dismiss_cancel();
                    ActivityResult::Consumed
                }
            }
            InputEvent::Press(Button::Back) => {
                self.dismiss_cancel();
                ActivityResult::Consumed
            }
            _ => ActivityResult::Ignored,
        }
    }

    /// Handle left button press (cycle backwards)
    fn handle_left_press(&mut self) -> ActivityResult {
        match self.current_item() {
            SettingItem::FontSize => {
                if let Some(prev) = self.settings.font_size.prev() {
                    self.settings.font_size = prev;
                    self.show_toast(format!("Font size: {}", prev.label()));
                }
            }
            SettingItem::FontFamily => {
                self.settings.font_family = self.settings.font_family.prev_wrapped();
                self.show_toast(format!("Font: {}", self.settings.font_family.label()));
            }
            SettingItem::LineSpacing => {
                self.settings.line_spacing = self.settings.line_spacing.prev_wrapped();
                self.show_toast(format!("Spacing: {}", self.settings.line_spacing.label()));
            }
            SettingItem::MarginSize => {
                self.settings.margin_size = self.settings.margin_size.prev_wrapped();
                self.show_toast(format!("Margins: {}", self.settings.margin_size.label()));
            }
            SettingItem::TextAlignment => {
                self.settings.text_alignment = self.settings.text_alignment.prev_wrapped();
                self.show_toast(format!(
                    "Alignment: {}",
                    self.settings.text_alignment.label()
                ));
            }
            SettingItem::RefreshFrequency => {
                self.settings.refresh_frequency = self.settings.refresh_frequency.prev_wrapped();
                self.show_toast(format!(
                    "Refresh: {}",
                    self.settings.refresh_frequency.label()
                ));
            }
            SettingItem::TapZoneConfig => {
                self.settings.tap_zone_config = self.settings.tap_zone_config.prev_wrapped();
                self.show_toast(format!("Tap: {}", self.settings.tap_zone_config.label()));
            }
            SettingItem::VolumeButtonAction => {
                self.settings.volume_button_action =
                    self.settings.volume_button_action.prev_wrapped();
                self.show_toast(format!(
                    "Volume: {}",
                    self.settings.volume_button_action.label()
                ));
            }
            _ => return ActivityResult::Ignored,
        }
        ActivityResult::Consumed
    }
}

impl Default for ReaderSettingsActivity {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_size_cycling() {
        assert_eq!(FontSize::Small.next(), Some(FontSize::Medium));
        assert_eq!(FontSize::Medium.next(), Some(FontSize::Large));
        assert_eq!(FontSize::Large.next(), Some(FontSize::ExtraLarge));
        assert_eq!(FontSize::ExtraLarge.next(), None);

        assert_eq!(FontSize::Medium.prev(), Some(FontSize::Small));
        assert_eq!(FontSize::Small.prev(), None);
    }

    #[test]
    fn font_size_labels() {
        assert_eq!(FontSize::Small.label(), "Small");
        assert_eq!(FontSize::Medium.label(), "Medium");
        assert_eq!(FontSize::Large.label(), "Large");
        assert_eq!(FontSize::ExtraLarge.label(), "Extra Large");
    }

    #[test]
    fn font_family_labels() {
        assert_eq!(FontFamily::Serif.label(), "Serif");
        assert_eq!(FontFamily::SansSerif.label(), "Sans-serif");
        assert_eq!(FontFamily::Monospace.label(), "Monospace");
    }

    #[test]
    fn line_spacing_labels_and_multiplier() {
        assert_eq!(LineSpacing::Compact.label(), "Compact");
        assert_eq!(LineSpacing::Normal.label(), "Normal");
        assert_eq!(LineSpacing::Relaxed.label(), "Relaxed");

        assert_eq!(LineSpacing::Compact.multiplier(), 12);
        assert_eq!(LineSpacing::Normal.multiplier(), 15);
        assert_eq!(LineSpacing::Relaxed.multiplier(), 20);
    }

    #[test]
    fn margin_size_pixels() {
        assert_eq!(MarginSize::Small.pixels(), 20);
        assert_eq!(MarginSize::Medium.pixels(), 40);
        assert_eq!(MarginSize::Large.pixels(), 60);
    }

    #[test]
    fn text_alignment_labels() {
        assert_eq!(TextAlignment::Left.label(), "Left");
        assert_eq!(TextAlignment::Justified.label(), "Justified");
    }

    #[test]
    fn refresh_frequency_pages() {
        assert_eq!(RefreshFrequency::Every1.pages(), 1);
        assert_eq!(RefreshFrequency::Every5.pages(), 5);
        assert_eq!(RefreshFrequency::Every10.pages(), 10);
        assert_eq!(RefreshFrequency::Every15.pages(), 15);
        assert_eq!(RefreshFrequency::Every30.pages(), 30);
    }

    #[test]
    fn tap_zone_config_labels() {
        assert_eq!(TapZoneConfig::LeftNext.label(), "Left = Next");
        assert_eq!(TapZoneConfig::RightNext.label(), "Right = Next");
    }

    #[test]
    fn volume_button_action_labels() {
        assert_eq!(VolumeButtonAction::PageTurn.label(), "Page Turn");
        assert_eq!(VolumeButtonAction::Scroll.label(), "Scroll");
    }

    #[test]
    fn reader_settings_defaults() {
        let settings = ReaderSettings::default();
        assert_eq!(settings.font_size, FontSize::Medium);
        assert_eq!(settings.font_family, FontFamily::Serif);
        assert_eq!(settings.line_spacing, LineSpacing::Normal);
        assert_eq!(settings.margin_size, MarginSize::Medium);
        assert_eq!(settings.text_alignment, TextAlignment::Justified);
        assert!(settings.show_page_numbers);
        assert_eq!(settings.refresh_frequency, RefreshFrequency::Every10);
        assert!(!settings.invert_colors);
        assert_eq!(settings.tap_zone_config, TapZoneConfig::LeftNext);
        assert_eq!(settings.volume_button_action, VolumeButtonAction::Scroll);
    }

    #[test]
    fn reader_settings_reset() {
        let mut settings = ReaderSettings {
            font_size: FontSize::ExtraLarge,
            font_family: FontFamily::Monospace,
            line_spacing: LineSpacing::Relaxed,
            margin_size: MarginSize::Large,
            text_alignment: TextAlignment::Left,
            show_page_numbers: false,
            refresh_frequency: RefreshFrequency::Every10,
            invert_colors: true,
            tap_zone_config: TapZoneConfig::RightNext,
            volume_button_action: VolumeButtonAction::PageTurn,
        };

        settings.reset_to_defaults();

        assert_eq!(settings.font_size, FontSize::Medium);
        assert_eq!(settings.font_family, FontFamily::Serif);
        assert!(settings.show_page_numbers);
        assert!(!settings.invert_colors);
    }

    #[test]
    fn setting_item_sections() {
        assert_eq!(SettingItem::FontSize.section(), "Font");
        assert_eq!(SettingItem::FontFamily.section(), "Font");
        assert_eq!(SettingItem::MarginSize.section(), "Margins & Layout");
        assert_eq!(SettingItem::RefreshFrequency.section(), "Display");
        assert_eq!(SettingItem::TapZoneConfig.section(), "Navigation");
    }

    #[test]
    fn setting_item_is_toggle() {
        assert!(SettingItem::ShowPageNumbers.is_toggle());
        assert!(SettingItem::InvertColors.is_toggle());
        assert!(!SettingItem::FontSize.is_toggle());
        assert!(!SettingItem::SaveButton.is_toggle());
    }

    #[test]
    fn reader_settings_activity_lifecycle() {
        let mut activity = ReaderSettingsActivity::new();

        activity.on_enter();
        assert_eq!(activity.selected_index, 0);
        assert!(!activity.show_cancel_modal);

        activity.on_exit();
        // Settings should still be accessible
        assert_eq!(activity.settings().font_size, FontSize::Medium);
    }

    #[test]
    fn reader_settings_activity_with_custom_settings() {
        let custom = ReaderSettings {
            font_size: FontSize::Large,
            font_family: FontFamily::Monospace,
            ..ReaderSettings::default()
        };

        let activity = ReaderSettingsActivity::with_settings(custom);

        assert_eq!(activity.settings().font_size, FontSize::Large);
        assert_eq!(activity.settings().font_family, FontFamily::Monospace);
    }

    #[test]
    fn reader_settings_activity_navigation() {
        let mut activity = ReaderSettingsActivity::new();
        activity.on_enter();

        // Initial selection
        assert_eq!(activity.selected_index, 0);
        assert_eq!(activity.current_item(), SettingItem::FontSize);

        // Navigate down
        activity.select_next();
        assert_eq!(activity.selected_index, 1);
        assert_eq!(activity.current_item(), SettingItem::FontFamily);

        // Navigate up
        activity.select_prev();
        assert_eq!(activity.selected_index, 0);
        assert_eq!(activity.current_item(), SettingItem::FontSize);

        // Wrap around forward
        for _ in 0..SettingItem::ALL.len() - 1 {
            activity.select_next();
        }
        assert_eq!(activity.selected_index, SettingItem::ALL.len() - 1);

        // Wrap around backward
        activity.select_next();
        assert_eq!(activity.selected_index, 0);
    }

    #[test]
    fn reader_settings_activity_font_size_change() {
        let mut activity = ReaderSettingsActivity::new();
        activity.on_enter();

        // Initial state
        assert_eq!(activity.settings().font_size, FontSize::Medium);

        // Increase font size
        activity.handle_confirm();
        assert_eq!(activity.settings().font_size, FontSize::Large);
        assert!(activity.show_toast);
        assert_eq!(activity.toast_message, "Font size: Large");

        // Increase again
        activity.handle_confirm();
        assert_eq!(activity.settings().font_size, FontSize::ExtraLarge);

        // At max - should not change
        activity.handle_confirm();
        assert_eq!(activity.settings().font_size, FontSize::ExtraLarge);
    }

    #[test]
    fn reader_settings_activity_font_size_decrease() {
        let mut activity = ReaderSettingsActivity::new();
        activity.on_enter();

        // Decrease font size using left button
        activity.handle_left_press();
        assert_eq!(activity.settings().font_size, FontSize::Small);

        // At min - should not change
        activity.handle_left_press();
        assert_eq!(activity.settings().font_size, FontSize::Small);
    }

    #[test]
    fn reader_settings_activity_toggle_settings() {
        let mut activity = ReaderSettingsActivity::new();
        activity.on_enter();

        // Navigate to page numbers setting
        for _ in 0..5 {
            activity.select_next();
        }
        assert_eq!(activity.current_item(), SettingItem::ShowPageNumbers);

        assert!(activity.settings().show_page_numbers);

        // Toggle off
        activity.handle_confirm();
        assert!(!activity.settings().show_page_numbers);
        assert_eq!(activity.toast_message, "Page numbers: Off");

        // Toggle on
        activity.handle_confirm();
        assert!(activity.settings().show_page_numbers);
        assert_eq!(activity.toast_message, "Page numbers: On");
    }

    #[test]
    fn reader_settings_activity_modified_check() {
        let mut activity = ReaderSettingsActivity::new();
        activity.on_enter();

        assert!(!activity.is_modified());

        // Change a setting
        activity.handle_confirm();

        assert!(activity.is_modified());
    }

    #[test]
    fn reader_settings_activity_save_navigates_back() {
        let mut activity = ReaderSettingsActivity::new();
        activity.on_enter();

        // Navigate to save button
        for _ in 0..10 {
            activity.select_next();
        }
        assert_eq!(activity.current_item(), SettingItem::SaveButton);

        // Pressing save should return NavigateBack
        let result = activity.handle_confirm();
        assert!(matches!(result, ActivityResult::NavigateBack));
    }

    #[test]
    fn reader_settings_activity_cancel_modal() {
        let mut activity = ReaderSettingsActivity::new();
        activity.on_enter();

        // Make a change
        activity.handle_confirm();
        assert!(activity.is_modified());

        // Cancel should show modal
        activity.cancel_changes();
        assert!(activity.show_cancel_modal);
        assert_eq!(activity.modal_button, 0); // Starts on Keep

        // Dismiss modal
        activity.dismiss_cancel();
        assert!(!activity.show_cancel_modal);

        // Reopen, navigate to Discard, confirm
        activity.cancel_changes();
        assert!(activity.show_cancel_modal);

        activity.modal_button = 1; // Discard
        activity.confirm_cancel();
        assert!(!activity.show_cancel_modal);
        assert!(!activity.is_modified()); // Changes discarded
    }

    #[test]
    fn reader_settings_activity_modal_button_navigation() {
        let mut activity = ReaderSettingsActivity::new();
        activity.on_enter();

        // Make a change and open modal
        activity.handle_confirm();
        activity.cancel_changes();
        assert!(activity.show_cancel_modal);

        // Test button navigation
        assert_eq!(activity.modal_button, 0);
        activity.handle_input(InputEvent::Press(Button::Right));
        assert_eq!(activity.modal_button, 1);
        activity.handle_input(InputEvent::Press(Button::Left));
        assert_eq!(activity.modal_button, 0);

        // VolumeDown/Up
        activity.handle_input(InputEvent::Press(Button::VolumeDown));
        assert_eq!(activity.modal_button, 1);
        activity.handle_input(InputEvent::Press(Button::VolumeUp));
        assert_eq!(activity.modal_button, 0);

        // Wrapping
        activity.handle_input(InputEvent::Press(Button::Left));
        assert_eq!(activity.modal_button, 1);

        // Confirm on Keep (0) dismisses
        activity.modal_button = 0;
        let result = activity.handle_input(InputEvent::Press(Button::Confirm));
        assert_eq!(result, ActivityResult::Consumed);
        assert!(!activity.show_cancel_modal);
    }

    #[test]
    fn reader_settings_activity_input_handling() {
        let mut activity = ReaderSettingsActivity::new();
        activity.on_enter();

        // Back button without changes
        let result = activity.handle_input(InputEvent::Press(Button::Back));
        assert!(matches!(result, ActivityResult::NavigateBack));

        // Make a change
        activity.handle_confirm();

        // Back button with changes should show modal, not navigate
        let result = activity.handle_input(InputEvent::Press(Button::Back));
        assert!(matches!(result, ActivityResult::Consumed));
        assert!(activity.show_cancel_modal);
    }

    #[test]
    fn reader_settings_activity_render() {
        let mut activity = ReaderSettingsActivity::new();
        activity.on_enter();

        let mut display = crate::test_display::TestDisplay::default_size();
        let result = activity.render(&mut display);
        assert!(result.is_ok());
    }

    #[test]
    fn toast_timing() {
        let mut activity = ReaderSettingsActivity::new();

        activity.show_toast("Test message");
        assert!(activity.show_toast);
        assert_eq!(
            activity.toast_frames_remaining,
            ReaderSettingsActivity::TOAST_DURATION
        );

        // Simulate frame updates
        for _ in 0..ReaderSettingsActivity::TOAST_DURATION {
            activity.update();
        }

        assert!(!activity.show_toast);
    }

    #[test]
    fn volume_buttons_navigation() {
        let mut activity = ReaderSettingsActivity::new();
        activity.on_enter();

        // Volume down to navigate next
        let result = activity.handle_input(InputEvent::Press(Button::VolumeDown));
        assert!(matches!(result, ActivityResult::Consumed));
        assert_eq!(activity.selected_index, 1);

        // Volume up to navigate previous
        let result = activity.handle_input(InputEvent::Press(Button::VolumeUp));
        assert!(matches!(result, ActivityResult::Consumed));
        assert_eq!(activity.selected_index, 0);
    }

    #[test]
    fn get_value_label_for_toggles() {
        let activity = ReaderSettingsActivity::new();

        let mut settings = *activity.settings();
        settings.show_page_numbers = true;
        settings.invert_colors = false;

        let activity = ReaderSettingsActivity::with_settings(settings);

        assert!(activity
            .get_value_label(SettingItem::ShowPageNumbers)
            .contains("✓"));
        assert!(activity
            .get_value_label(SettingItem::InvertColors)
            .contains(" "));
    }

    #[test]
    fn enum_index_roundtrips() {
        // Test all enums roundtrip correctly through index/from_index
        for i in 0..4 {
            let size = FontSize::from_index(i).unwrap();
            assert_eq!(size.index(), i);
        }

        for i in 0..3 {
            let family = FontFamily::from_index(i).unwrap();
            assert_eq!(family.index(), i);
        }

        for i in 0..3 {
            let spacing = LineSpacing::from_index(i).unwrap();
            assert_eq!(spacing.index(), i);
        }

        for i in 0..3 {
            let margin = MarginSize::from_index(i).unwrap();
            assert_eq!(margin.index(), i);
        }

        for i in 0..2 {
            let align = TextAlignment::from_index(i).unwrap();
            assert_eq!(align.index(), i);
        }

        for i in 0..4 {
            let freq = RefreshFrequency::from_index(i).unwrap();
            assert_eq!(freq.index(), i);
        }

        for i in 0..2 {
            let tap = TapZoneConfig::from_index(i).unwrap();
            assert_eq!(tap.index(), i);
        }

        for i in 0..2 {
            let vol = VolumeButtonAction::from_index(i).unwrap();
            assert_eq!(vol.index(), i);
        }
    }
}
