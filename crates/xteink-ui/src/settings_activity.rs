//! Settings Activity for Xteink X4 e-reader.
//!
//! Provides font size and font family configuration with
//! modern, minimal design optimized for e-ink displays.

extern crate alloc;

use alloc::format;
use alloc::string::String;

use embedded_graphics::{
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
};

use crate::input::{Button, InputEvent};
use crate::ui::helpers::{
    enum_from_index, handle_two_button_modal_input, TwoButtonModalInputResult,
};
use crate::ui::theme::ui_text;
use crate::ui::{Activity, ActivityResult, Modal, Theme, Toast};

/// Font size options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FontSize {
    Small,
    #[default]
    Medium,
    Large,
    ExtraLarge,
    Huge,
    Max,
}

impl FontSize {
    /// All font size variants in order
    pub const ALL: [Self; 6] = [
        Self::Small,
        Self::Medium,
        Self::Large,
        Self::ExtraLarge,
        Self::Huge,
        Self::Max,
    ];

    /// Get display label for the font size
    pub const fn label(self) -> &'static str {
        match self {
            Self::Small => "Small",
            Self::Medium => "Medium",
            Self::Large => "Large",
            Self::ExtraLarge => "Extra Large",
            Self::Huge => "Huge",
            Self::Max => "Max",
        }
    }

    /// Get the font size in points
    pub const fn points(self) -> u8 {
        match self {
            Self::Small => 12,
            Self::Medium => 16,
            Self::Large => 20,
            Self::ExtraLarge => 24,
            Self::Huge => 28,
            Self::Max => 32,
        }
    }

    /// Base EPUB body text size in CSS px used by renderer defaults.
    /// All sizes increased by 25% for better readability.
    pub const fn epub_base_px(self) -> f32 {
        match self {
            Self::Small => 20.0,
            Self::Medium => 24.0,
            Self::Large => 28.0,
            Self::ExtraLarge => 33.0,
            Self::Huge => 38.0,
            Self::Max => 43.0,
        }
    }

    /// Global EPUB text scaling factor (applies to px/em/default text sizes).
    pub const fn epub_text_scale(self) -> f32 {
        match self {
            Self::Small => 0.90,
            Self::Medium => 1.00,
            Self::Large => 1.18,
            Self::ExtraLarge => 1.34,
            Self::Huge => 1.52,
            Self::Max => 1.72,
        }
    }

    /// Get the next larger font size
    pub const fn next(self) -> Option<Self> {
        match self {
            Self::Small => Some(Self::Medium),
            Self::Medium => Some(Self::Large),
            Self::Large => Some(Self::ExtraLarge),
            Self::ExtraLarge => Some(Self::Huge),
            Self::Huge => Some(Self::Max),
            Self::Max => None,
        }
    }

    /// Get the previous smaller font size
    pub const fn prev(self) -> Option<Self> {
        match self {
            Self::Small => None,
            Self::Medium => Some(Self::Small),
            Self::Large => Some(Self::Medium),
            Self::ExtraLarge => Some(Self::Large),
            Self::Huge => Some(Self::ExtraLarge),
            Self::Max => Some(Self::Huge),
        }
    }

    /// Get index in ALL array
    pub const fn index(self) -> usize {
        match self {
            Self::Small => 0,
            Self::Medium => 1,
            Self::Large => 2,
            Self::ExtraLarge => 3,
            Self::Huge => 4,
            Self::Max => 5,
        }
    }

    /// Create from index
    pub const fn from_index(index: usize) -> Option<Self> {
        enum_from_index(&Self::ALL, index)
    }
}

/// Font family options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FontFamily {
    Monospace,
    #[default]
    Serif,
    SansSerif,
}

/// Auto-sleep duration options for power management
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AutoSleepDuration {
    OneMinute,
    ThreeMinutes,
    FiveMinutes,
    #[default]
    TenMinutes,
    FifteenMinutes,
    ThirtyMinutes,
    Never,
}

/// Sleep screen display mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SleepScreenMode {
    #[default]
    Default,
    Custom,
    Cover,
}

impl SleepScreenMode {
    pub const ALL: [Self; 3] = [Self::Default, Self::Custom, Self::Cover];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Default => "Default",
            Self::Custom => "Custom",
            Self::Cover => "Book Cover",
        }
    }

    pub const fn index(self) -> usize {
        match self {
            Self::Default => 0,
            Self::Custom => 1,
            Self::Cover => 2,
        }
    }

    pub const fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::Default),
            1 => Some(Self::Custom),
            2 => Some(Self::Cover),
            _ => None,
        }
    }

    pub const fn next_wrapped(self) -> Self {
        match self {
            Self::Default => Self::Custom,
            Self::Custom => Self::Cover,
            Self::Cover => Self::Default,
        }
    }

    pub const fn prev_wrapped(self) -> Self {
        match self {
            Self::Default => Self::Cover,
            Self::Custom => Self::Default,
            Self::Cover => Self::Custom,
        }
    }
}

impl AutoSleepDuration {
    /// All auto-sleep duration variants
    pub const ALL: [Self; 7] = [
        Self::OneMinute,
        Self::ThreeMinutes,
        Self::FiveMinutes,
        Self::TenMinutes,
        Self::FifteenMinutes,
        Self::ThirtyMinutes,
        Self::Never,
    ];

    /// Get display label
    pub const fn label(self) -> &'static str {
        match self {
            Self::OneMinute => "1 minute",
            Self::ThreeMinutes => "3 minutes",
            Self::FiveMinutes => "5 minutes",
            Self::TenMinutes => "10 minutes",
            Self::FifteenMinutes => "15 minutes",
            Self::ThirtyMinutes => "30 minutes",
            Self::Never => "Never",
        }
    }

    /// Get duration in milliseconds (0 for Never)
    pub const fn milliseconds(self) -> u32 {
        match self {
            Self::OneMinute => 60_000,
            Self::ThreeMinutes => 180_000,
            Self::FiveMinutes => 300_000,
            Self::TenMinutes => 600_000,
            Self::FifteenMinutes => 900_000,
            Self::ThirtyMinutes => 1_800_000,
            Self::Never => 0,
        }
    }

    /// Get index in ALL array
    pub const fn index(self) -> usize {
        match self {
            Self::OneMinute => 0,
            Self::ThreeMinutes => 1,
            Self::FiveMinutes => 2,
            Self::TenMinutes => 3,
            Self::FifteenMinutes => 4,
            Self::ThirtyMinutes => 5,
            Self::Never => 6,
        }
    }

    /// Create from index
    pub const fn from_index(index: usize) -> Option<Self> {
        enum_from_index(&Self::ALL, index)
    }

    pub const fn next_wrapped(self) -> Self {
        match self {
            Self::OneMinute => Self::ThreeMinutes,
            Self::ThreeMinutes => Self::FiveMinutes,
            Self::FiveMinutes => Self::TenMinutes,
            Self::TenMinutes => Self::FifteenMinutes,
            Self::FifteenMinutes => Self::ThirtyMinutes,
            Self::ThirtyMinutes => Self::Never,
            Self::Never => Self::OneMinute,
        }
    }

    pub const fn prev_wrapped(self) -> Self {
        match self {
            Self::OneMinute => Self::Never,
            Self::ThreeMinutes => Self::OneMinute,
            Self::FiveMinutes => Self::ThreeMinutes,
            Self::TenMinutes => Self::FiveMinutes,
            Self::FifteenMinutes => Self::TenMinutes,
            Self::ThirtyMinutes => Self::FifteenMinutes,
            Self::Never => Self::ThirtyMinutes,
        }
    }
}

impl FontFamily {
    /// All font family variants
    pub const ALL: [Self; 3] = [Self::Monospace, Self::Serif, Self::SansSerif];

    /// Get display label for the font family
    pub const fn label(self) -> &'static str {
        match self {
            Self::Monospace => "Monospace",
            Self::Serif => "Serif",
            Self::SansSerif => "Sans-serif",
        }
    }

    /// Get index in ALL array
    pub const fn index(self) -> usize {
        match self {
            Self::Monospace => 0,
            Self::Serif => 1,
            Self::SansSerif => 2,
        }
    }

    /// Create from index
    pub const fn from_index(index: usize) -> Option<Self> {
        enum_from_index(&Self::ALL, index)
    }

    pub const fn next_wrapped(self) -> Self {
        match self {
            Self::Monospace => Self::Serif,
            Self::Serif => Self::SansSerif,
            Self::SansSerif => Self::Monospace,
        }
    }

    pub const fn prev_wrapped(self) -> Self {
        match self {
            Self::Monospace => Self::SansSerif,
            Self::Serif => Self::Monospace,
            Self::SansSerif => Self::Serif,
        }
    }
}

/// Settings data container (in-memory storage)
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Settings {
    pub font_size: FontSize,
    pub font_family: FontFamily,
    pub auto_sleep_duration: AutoSleepDuration,
}

impl Settings {
    /// Reset to factory defaults
    pub fn reset_to_defaults(&mut self) {
        *self = Self::default();
    }
}

/// Focusable setting rows in the settings screen
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingRow {
    FontSize,
    FontFamily,
    AutoSleep,
    ResetButton,
    SaveButton,
}

impl SettingRow {
    /// All setting rows in display order
    pub const ALL: [Self; 5] = [
        Self::FontSize,
        Self::FontFamily,
        Self::AutoSleep,
        Self::ResetButton,
        Self::SaveButton,
    ];

    /// Get the label for this setting row
    pub const fn label(self) -> &'static str {
        match self {
            Self::FontSize => "Font Size",
            Self::FontFamily => "Font Family",
            Self::AutoSleep => "Auto-Sleep",
            Self::ResetButton => "Reset to Defaults",
            Self::SaveButton => "Save Changes",
        }
    }

    /// Check if this is an action button (not a value row)
    pub const fn is_button(self) -> bool {
        matches!(self, Self::ResetButton | Self::SaveButton)
    }
}

/// Settings Activity implementing the Activity trait
#[derive(Debug, Clone)]
pub struct SettingsActivity {
    settings: Settings,
    original_settings: Settings,
    selected_index: usize,
    show_toast: bool,
    toast_message: String,
    toast_frames_remaining: u32,
    show_reset_modal: bool,
    apply_requested: bool,
    /// Tracks which button is selected in the reset modal (0=Cancel, 1=Reset)
    modal_button: usize,
    theme: Theme,
}

impl SettingsActivity {
    /// Toast display duration in frames
    const TOAST_DURATION: u32 = 120; // ~2 seconds at 60fps

    /// Create a new settings activity
    pub fn new() -> Self {
        let settings = Settings::default();
        Self {
            settings,
            original_settings: settings,
            selected_index: 0,
            show_toast: false,
            toast_message: String::new(),
            toast_frames_remaining: 0,
            show_reset_modal: false,
            apply_requested: false,
            modal_button: 0,
            theme: Theme::default(),
        }
    }

    /// Create with specific initial settings
    pub fn with_settings(settings: Settings) -> Self {
        Self {
            settings,
            original_settings: settings,
            selected_index: 0,
            show_toast: false,
            toast_message: String::new(),
            toast_frames_remaining: 0,
            show_reset_modal: false,
            apply_requested: false,
            modal_button: 0,
            theme: Theme::default(),
        }
    }

    /// Get current settings
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Get currently applied (saved) settings.
    pub fn applied_settings(&self) -> &Settings {
        &self.original_settings
    }

    /// Check if settings were modified
    pub fn is_modified(&self) -> bool {
        self.settings != self.original_settings
    }

    /// Consume pending apply request.
    pub fn take_apply_request(&mut self) -> bool {
        let requested = self.apply_requested;
        self.apply_requested = false;
        requested
    }

    /// Save draft settings and request apply.
    fn save_settings(&mut self) {
        self.original_settings = self.settings;
        self.apply_requested = true;
        self.show_toast("Settings saved");
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

    /// Get currently selected row
    fn current_row(&self) -> SettingRow {
        SettingRow::ALL[self.selected_index]
    }

    /// Move selection to next row (wraps)
    fn select_next(&mut self) {
        self.selected_index = (self.selected_index + 1) % SettingRow::ALL.len();
    }

    /// Move selection to previous row (wraps)
    fn select_prev(&mut self) {
        if self.selected_index == 0 {
            self.selected_index = SettingRow::ALL.len() - 1;
        } else {
            self.selected_index -= 1;
        }
    }

    /// Confirm reset to defaults
    fn confirm_reset(&mut self) {
        self.settings.reset_to_defaults();
        self.show_toast("Settings reset to defaults");
        self.show_reset_modal = false;
        self.modal_button = 0;
    }

    /// Cancel reset
    fn cancel_reset(&mut self) {
        self.show_reset_modal = false;
        self.modal_button = 0;
    }

    /// Get the current value label for a setting row
    fn get_value_label(&self, row: SettingRow) -> &'static str {
        match row {
            SettingRow::FontSize => self.settings.font_size.label(),
            SettingRow::FontFamily => self.settings.font_family.label(),
            SettingRow::AutoSleep => self.settings.auto_sleep_duration.label(),
            SettingRow::ResetButton => "",
            SettingRow::SaveButton => "",
        }
    }

    /// Render header bar
    fn render_header<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
        theme: &Theme,
    ) -> Result<(), D::Error> {
        use crate::ui::Header;

        let header = Header::new("Device Settings");
        header.render(display, theme)
    }

    /// Render main content area
    fn render_content<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
        theme: &Theme,
    ) -> Result<(), D::Error> {
        let display_width = display.bounding_box().size.width;
        let content_width = theme.metrics.content_width(display_width);
        let x = theme.metrics.side_padding as i32;
        let mut y = theme.metrics.header_height as i32 + theme.metrics.spacing_double() as i32;

        for (i, row) in SettingRow::ALL.iter().enumerate() {
            let row = *row;
            let is_selected = i == self.selected_index;

            if row.is_button() {
                // Extra spacing above the reset button
                y += theme.metrics.spacing_double() as i32;

                let height = theme.metrics.button_height;

                // Background
                let bg_color = if is_selected {
                    BinaryColor::On
                } else {
                    BinaryColor::Off
                };
                Rectangle::new(Point::new(x, y), Size::new(content_width, height))
                    .into_styled(PrimitiveStyle::with_fill(bg_color))
                    .draw(display)?;

                // Border
                Rectangle::new(Point::new(x, y), Size::new(content_width, height))
                    .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
                    .draw(display)?;

                // Centered bold text
                let text_color = if is_selected {
                    BinaryColor::Off
                } else {
                    BinaryColor::On
                };
                let label = row.label();
                let label_width = ui_text::width(label, Some(ui_text::DEFAULT_SIZE)) as i32;
                let label_x = x + (content_width as i32 - label_width) / 2;
                ui_text::draw_colored(
                    display,
                    label,
                    label_x,
                    y + ui_text::center_y(height, Some(ui_text::DEFAULT_SIZE)),
                    Some(ui_text::DEFAULT_SIZE),
                    text_color,
                )?;

                y += height as i32;
            } else {
                // Value row: label on left, < Value > on right
                let height = theme.metrics.list_item_height;
                // Background
                let bg_color = if is_selected {
                    BinaryColor::On
                } else {
                    BinaryColor::Off
                };
                Rectangle::new(Point::new(x, y), Size::new(content_width, height))
                    .into_styled(PrimitiveStyle::with_fill(bg_color))
                    .draw(display)?;

                let text_color = if is_selected {
                    BinaryColor::Off
                } else {
                    BinaryColor::On
                };

                // Label on left
                ui_text::draw_colored(
                    display,
                    row.label(),
                    x + theme.metrics.side_padding as i32,
                    y + ui_text::center_y(height, Some(ui_text::DEFAULT_SIZE)),
                    Some(ui_text::DEFAULT_SIZE),
                    text_color,
                )?;

                // < Value > on right
                let value = self.get_value_label(row);
                let value_text = format!("< {} >", value);
                let value_width = ui_text::width(&value_text, Some(ui_text::DEFAULT_SIZE)) as i32;
                let value_x =
                    x + content_width as i32 - value_width - theme.metrics.side_padding as i32;
                ui_text::draw_colored(
                    display,
                    &value_text,
                    value_x,
                    y + ui_text::center_y(height, Some(ui_text::DEFAULT_SIZE)),
                    Some(ui_text::DEFAULT_SIZE),
                    text_color,
                )?;

                y += height as i32;

                // Separator line between value rows (not after the last value row before button)
                if i < SettingRow::ALL.len() - 1 && !SettingRow::ALL[i + 1].is_button() {
                    Rectangle::new(Point::new(x + 10, y - 1), Size::new(content_width - 20, 1))
                        .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
                        .draw(display)?;
                }
            }
        }

        Ok(())
    }
}

impl Activity for SettingsActivity {
    fn on_enter(&mut self) {
        self.settings = self.original_settings;
        self.selected_index = 0;
        self.show_toast = false;
        self.show_reset_modal = false;
        self.apply_requested = false;
        self.modal_button = 0;
    }

    fn on_exit(&mut self) {
        // Keep only applied values when leaving without save.
        self.settings = self.original_settings;
        self.show_reset_modal = false;
        self.modal_button = 0;
    }

    fn handle_input(&mut self, event: InputEvent) -> ActivityResult {
        if self.show_reset_modal {
            return self.handle_modal_input(event);
        }

        match event {
            InputEvent::Press(Button::Back) => {
                self.settings = self.original_settings;
                ActivityResult::NavigateBack
            }
            InputEvent::Press(Button::Aux1) | InputEvent::Press(Button::Up) => {
                self.select_prev();
                ActivityResult::Consumed
            }
            InputEvent::Press(Button::Aux2) | InputEvent::Press(Button::Down) => {
                self.select_next();
                ActivityResult::Consumed
            }
            InputEvent::Press(Button::Right) | InputEvent::Press(Button::Confirm) => {
                self.handle_right_or_confirm()
            }
            InputEvent::Press(Button::Left) => self.handle_left(),
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

        // Content
        self.render_content(display, &self.theme)?;

        // Toast notification
        if self.show_toast {
            let display_width = display.bounding_box().size.width;
            let display_height = display.bounding_box().size.height;
            let toast = Toast::bottom_center(&self.toast_message, display_width, display_height);
            toast.render(display)?;
        }

        // Modal dialog — use tracked modal_button for selection
        if self.show_reset_modal {
            let mut modal = Modal::new("Reset Settings", "Restore all settings to defaults?")
                .with_button("Cancel")
                .with_button("Reset");
            modal.selected_button = self.modal_button;
            modal.render(display, &self.theme)?;
        }

        Ok(())
    }

    fn refresh_mode(&self) -> crate::ui::ActivityRefreshMode {
        crate::ui::ActivityRefreshMode::Fast
    }
}

impl SettingsActivity {
    /// Handle input when modal is shown.
    /// Left/Right cycle buttons, Confirm executes selected, Back cancels.
    fn handle_modal_input(&mut self, event: InputEvent) -> ActivityResult {
        match handle_two_button_modal_input(event, &mut self.modal_button) {
            TwoButtonModalInputResult::Consumed => ActivityResult::Consumed,
            TwoButtonModalInputResult::Confirmed => {
                self.confirm_reset();
                ActivityResult::Consumed
            }
            TwoButtonModalInputResult::Cancelled => {
                self.cancel_reset();
                ActivityResult::Consumed
            }
            TwoButtonModalInputResult::Ignored => ActivityResult::Ignored,
        }
    }

    /// Handle Right or Confirm press on the current row
    fn handle_right_or_confirm(&mut self) -> ActivityResult {
        match self.current_row() {
            SettingRow::FontSize => {
                if let Some(next) = self.settings.font_size.next() {
                    self.settings.font_size = next;
                    self.show_toast(format!("Font size: {}", next.label()));
                }
                ActivityResult::Consumed
            }
            SettingRow::FontFamily => {
                self.settings.font_family = self.settings.font_family.next_wrapped();
                self.show_toast(format!("Font: {}", self.settings.font_family.label()));
                ActivityResult::Consumed
            }
            SettingRow::AutoSleep => {
                self.settings.auto_sleep_duration =
                    self.settings.auto_sleep_duration.next_wrapped();
                self.show_toast(format!(
                    "Auto-sleep: {}",
                    self.settings.auto_sleep_duration.label()
                ));
                ActivityResult::Consumed
            }
            SettingRow::ResetButton => {
                self.show_reset_modal = true;
                self.modal_button = 0; // Start on Cancel (safe default)
                ActivityResult::Consumed
            }
            SettingRow::SaveButton => {
                self.save_settings();
                ActivityResult::NavigateBack
            }
        }
    }

    /// Handle Left press on the current row
    fn handle_left(&mut self) -> ActivityResult {
        match self.current_row() {
            SettingRow::FontSize => {
                if let Some(prev) = self.settings.font_size.prev() {
                    self.settings.font_size = prev;
                    self.show_toast(format!("Font size: {}", prev.label()));
                }
                ActivityResult::Consumed
            }
            SettingRow::FontFamily => {
                self.settings.font_family = self.settings.font_family.prev_wrapped();
                self.show_toast(format!("Font: {}", self.settings.font_family.label()));
                ActivityResult::Consumed
            }
            SettingRow::AutoSleep => {
                self.settings.auto_sleep_duration =
                    self.settings.auto_sleep_duration.prev_wrapped();
                self.show_toast(format!(
                    "Auto-sleep: {}",
                    self.settings.auto_sleep_duration.label()
                ));
                ActivityResult::Consumed
            }
            SettingRow::ResetButton => ActivityResult::Ignored,
            SettingRow::SaveButton => ActivityResult::Ignored,
        }
    }
}

impl Default for SettingsActivity {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_size_cycling() {
        let mut size = FontSize::Small;

        size = size.next().unwrap();
        assert_eq!(size, FontSize::Medium);

        size = size.next().unwrap();
        assert_eq!(size, FontSize::Large);

        size = size.next().unwrap();
        assert_eq!(size, FontSize::ExtraLarge);

        size = size.next().unwrap();
        assert_eq!(size, FontSize::Huge);

        size = size.next().unwrap();
        assert_eq!(size, FontSize::Max);

        assert!(size.next().is_none());

        size = size.prev().unwrap();
        assert_eq!(size, FontSize::Huge);

        size = size.prev().unwrap();
        assert_eq!(size, FontSize::ExtraLarge);

        size = size.prev().unwrap();
        assert_eq!(size, FontSize::Large);

        size = size.prev().unwrap();
        assert_eq!(size, FontSize::Medium);

        size = size.prev().unwrap();
        assert_eq!(size, FontSize::Small);

        assert!(size.prev().is_none());
    }

    #[test]
    fn font_size_labels() {
        assert_eq!(FontSize::Small.label(), "Small");
        assert_eq!(FontSize::Medium.label(), "Medium");
        assert_eq!(FontSize::Large.label(), "Large");
        assert_eq!(FontSize::ExtraLarge.label(), "Extra Large");
        assert_eq!(FontSize::Huge.label(), "Huge");
        assert_eq!(FontSize::Max.label(), "Max");
    }

    #[test]
    fn font_size_points() {
        assert_eq!(FontSize::Small.points(), 12);
        assert_eq!(FontSize::Medium.points(), 16);
        assert_eq!(FontSize::Large.points(), 20);
        assert_eq!(FontSize::ExtraLarge.points(), 24);
        assert_eq!(FontSize::Huge.points(), 28);
        assert_eq!(FontSize::Max.points(), 32);
    }

    #[test]
    fn font_size_index_roundtrip() {
        for i in 0..6 {
            let size = FontSize::from_index(i).unwrap();
            assert_eq!(size.index(), i);
        }
        assert!(FontSize::from_index(6).is_none());
    }

    #[test]
    fn font_family_labels() {
        assert_eq!(FontFamily::Monospace.label(), "Monospace");
        assert_eq!(FontFamily::Serif.label(), "Serif");
        assert_eq!(FontFamily::SansSerif.label(), "Sans-serif");
    }

    #[test]
    fn font_family_index_roundtrip() {
        for i in 0..3 {
            let family = FontFamily::from_index(i).unwrap();
            assert_eq!(family.index(), i);
        }
        assert!(FontFamily::from_index(3).is_none());
    }

    #[test]
    fn settings_defaults() {
        let settings = Settings::default();
        assert_eq!(settings.font_size, FontSize::Medium);
        assert_eq!(settings.font_family, FontFamily::Serif);
        assert_eq!(settings.auto_sleep_duration, AutoSleepDuration::TenMinutes);
    }

    #[test]
    fn settings_reset() {
        let mut settings = Settings {
            font_size: FontSize::ExtraLarge,
            font_family: FontFamily::SansSerif,
            auto_sleep_duration: AutoSleepDuration::Never,
        };

        settings.reset_to_defaults();

        assert_eq!(settings.font_size, FontSize::Medium);
        assert_eq!(settings.font_family, FontFamily::Serif);
        assert_eq!(settings.auto_sleep_duration, AutoSleepDuration::TenMinutes);
    }

    #[test]
    fn setting_row_labels() {
        assert_eq!(SettingRow::FontSize.label(), "Font Size");
        assert_eq!(SettingRow::FontFamily.label(), "Font Family");
        assert_eq!(SettingRow::ResetButton.label(), "Reset to Defaults");
        assert_eq!(SettingRow::SaveButton.label(), "Save Changes");

        assert!(!SettingRow::FontSize.is_button());
        assert!(!SettingRow::FontFamily.is_button());
        assert!(SettingRow::ResetButton.is_button());
        assert!(SettingRow::SaveButton.is_button());
    }

    #[test]
    fn settings_activity_lifecycle() {
        let mut activity = SettingsActivity::new();

        activity.on_enter();
        assert!(!activity.show_reset_modal);
        assert!(!activity.show_toast);
        assert_eq!(activity.selected_index, 0);

        activity.on_exit();
        // Settings should still be accessible
        assert_eq!(activity.settings().font_size, FontSize::Medium);
    }

    #[test]
    fn settings_activity_row_navigation() {
        let mut activity = SettingsActivity::new();
        activity.on_enter();

        // Initial selection
        assert_eq!(activity.selected_index, 0);
        assert_eq!(activity.current_row(), SettingRow::FontSize);

        // Navigate down
        let result = activity.handle_input(InputEvent::Press(Button::Down));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(activity.selected_index, 1);
        assert_eq!(activity.current_row(), SettingRow::FontFamily);

        // Navigate down again
        let result = activity.handle_input(InputEvent::Press(Button::Down));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(activity.selected_index, 2);
        assert_eq!(activity.current_row(), SettingRow::ResetButton);

        // Navigate down to save button
        let result = activity.handle_input(InputEvent::Press(Button::Down));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(activity.selected_index, 3);
        assert_eq!(activity.current_row(), SettingRow::SaveButton);

        // Wrap forward
        let result = activity.handle_input(InputEvent::Press(Button::Down));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(activity.selected_index, 0);

        // Navigate up (wraps backward)
        let result = activity.handle_input(InputEvent::Press(Button::Up));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(activity.selected_index, 3);

        // Navigate up
        let result = activity.handle_input(InputEvent::Press(Button::Up));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(activity.selected_index, 2);

        // Back navigates back
        let result = activity.handle_input(InputEvent::Press(Button::Back));
        assert_eq!(result, ActivityResult::NavigateBack);
    }

    #[test]
    fn settings_activity_font_size_adjust() {
        let mut activity = SettingsActivity::new();
        activity.on_enter();

        // Start on FontSize row, default is Medium
        assert_eq!(activity.current_row(), SettingRow::FontSize);
        assert_eq!(activity.settings().font_size, FontSize::Medium);

        // Right increases font size
        let result = activity.handle_input(InputEvent::Press(Button::Right));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(activity.settings().font_size, FontSize::Large);
        assert!(activity.show_toast);
        assert_eq!(activity.toast_message, "Font size: Large");

        // Left decreases font size
        let result = activity.handle_input(InputEvent::Press(Button::Left));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(activity.settings().font_size, FontSize::Medium);
        assert_eq!(activity.toast_message, "Font size: Medium");

        // Confirm also increases (same as Right)
        let result = activity.handle_input(InputEvent::Press(Button::Confirm));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(activity.settings().font_size, FontSize::Large);
    }

    #[test]
    fn settings_activity_font_family_adjust() {
        let mut activity = SettingsActivity::new();
        activity.on_enter();

        // Navigate to FontFamily row
        activity.handle_input(InputEvent::Press(Button::Down));
        assert_eq!(activity.current_row(), SettingRow::FontFamily);
        assert_eq!(activity.settings().font_family, FontFamily::Serif);

        // Right cycles forward
        let result = activity.handle_input(InputEvent::Press(Button::Right));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(activity.settings().font_family, FontFamily::SansSerif);

        // Right again
        activity.handle_input(InputEvent::Press(Button::Right));
        assert_eq!(activity.settings().font_family, FontFamily::Monospace);

        // Right wraps to beginning
        activity.handle_input(InputEvent::Press(Button::Right));
        assert_eq!(activity.settings().font_family, FontFamily::Serif);

        // Left cycles backward (wraps)
        let result = activity.handle_input(InputEvent::Press(Button::Left));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(activity.settings().font_family, FontFamily::Monospace);

        // Left again
        activity.handle_input(InputEvent::Press(Button::Left));
        assert_eq!(activity.settings().font_family, FontFamily::SansSerif);
    }

    #[test]
    fn settings_activity_font_size_at_bounds() {
        // Right at Max doesn't change
        let mut activity = SettingsActivity::with_settings(Settings {
            font_size: FontSize::Max,
            font_family: FontFamily::Monospace,
            auto_sleep_duration: AutoSleepDuration::FiveMinutes,
        });
        activity.on_enter();

        let result = activity.handle_input(InputEvent::Press(Button::Right));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(activity.settings().font_size, FontSize::Max);

        // Left at Small doesn't change
        let mut activity = SettingsActivity::with_settings(Settings {
            font_size: FontSize::Small,
            font_family: FontFamily::Monospace,
            auto_sleep_duration: AutoSleepDuration::FiveMinutes,
        });
        activity.on_enter();

        let result = activity.handle_input(InputEvent::Press(Button::Left));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(activity.settings().font_size, FontSize::Small);
    }

    #[test]
    fn settings_activity_left_on_reset_ignored() {
        let mut activity = SettingsActivity::new();
        activity.on_enter();

        // Navigate to ResetButton
        activity.handle_input(InputEvent::Press(Button::Down));
        activity.handle_input(InputEvent::Press(Button::Down));
        assert_eq!(activity.current_row(), SettingRow::ResetButton);

        // Left on ResetButton returns Ignored
        let result = activity.handle_input(InputEvent::Press(Button::Left));
        assert_eq!(result, ActivityResult::Ignored);
    }

    #[test]
    fn settings_activity_confirm_on_reset_opens_modal() {
        let mut activity = SettingsActivity::new();
        activity.on_enter();

        // Navigate to ResetButton
        activity.handle_input(InputEvent::Press(Button::Down));
        activity.handle_input(InputEvent::Press(Button::Down));
        assert_eq!(activity.current_row(), SettingRow::ResetButton);

        // Confirm opens modal
        let result = activity.handle_input(InputEvent::Press(Button::Confirm));
        assert_eq!(result, ActivityResult::Consumed);
        assert!(activity.show_reset_modal);
        assert_eq!(activity.modal_button, 0); // Starts on Cancel
    }

    #[test]
    fn settings_activity_reset_modal() {
        let mut activity = SettingsActivity::new();
        activity.on_enter();

        // Navigate to ResetButton and open modal
        activity.handle_input(InputEvent::Press(Button::Down));
        activity.handle_input(InputEvent::Press(Button::Down));
        activity.handle_input(InputEvent::Press(Button::Confirm));
        assert!(activity.show_reset_modal);
        assert_eq!(activity.modal_button, 0); // Starts on Cancel

        // Cancel modal with Back
        activity.handle_input(InputEvent::Press(Button::Back));
        assert!(!activity.show_reset_modal);

        // Reopen modal
        activity.handle_input(InputEvent::Press(Button::Confirm));
        assert!(activity.show_reset_modal);

        // Navigate to Reset button and confirm
        activity.handle_input(InputEvent::Press(Button::Right)); // Navigate to Reset
        assert_eq!(activity.modal_button, 1);
        activity.handle_input(InputEvent::Press(Button::Confirm));
        assert!(!activity.show_reset_modal);
        assert!(activity.show_toast);
    }

    #[test]
    fn settings_activity_modal_button_navigation() {
        let mut activity = SettingsActivity::new();
        activity.on_enter();

        // Navigate to reset button and open modal
        activity.handle_input(InputEvent::Press(Button::Down));
        activity.handle_input(InputEvent::Press(Button::Down));
        activity.handle_input(InputEvent::Press(Button::Confirm));
        assert!(activity.show_reset_modal);

        // Test button navigation
        assert_eq!(activity.modal_button, 0);
        activity.handle_input(InputEvent::Press(Button::Right));
        assert_eq!(activity.modal_button, 1);
        activity.handle_input(InputEvent::Press(Button::Left));
        assert_eq!(activity.modal_button, 0);

        // Wrapping left from 0 → 1
        activity.handle_input(InputEvent::Press(Button::Left));
        assert_eq!(activity.modal_button, 1);

        // Wrapping right from 1 → 0
        activity.handle_input(InputEvent::Press(Button::Right));
        assert_eq!(activity.modal_button, 0);

        // VolumeUp
        activity.handle_input(InputEvent::Press(Button::Aux1));
        assert_eq!(activity.modal_button, 1);

        // VolumeDown
        activity.handle_input(InputEvent::Press(Button::Aux2));
        assert_eq!(activity.modal_button, 0);
    }

    #[test]
    fn settings_activity_modified_check() {
        let mut activity = SettingsActivity::new();
        activity.on_enter();

        assert!(!activity.is_modified());

        // Change font size via Right
        activity.handle_input(InputEvent::Press(Button::Right));
        assert!(activity.is_modified());
    }

    #[test]
    fn settings_activity_save_button_applies_changes() {
        let mut activity = SettingsActivity::new();
        activity.on_enter();
        activity.handle_input(InputEvent::Press(Button::Right)); // Font size -> Large
        assert!(activity.is_modified());

        // Move to Save button and confirm
        activity.handle_input(InputEvent::Press(Button::Down));
        activity.handle_input(InputEvent::Press(Button::Down));
        activity.handle_input(InputEvent::Press(Button::Down));
        assert_eq!(activity.current_row(), SettingRow::SaveButton);
        let result = activity.handle_input(InputEvent::Press(Button::Confirm));
        assert_eq!(result, ActivityResult::NavigateBack);
        assert_eq!(activity.applied_settings().font_size, FontSize::Large);
        assert!(activity.take_apply_request());
        assert!(!activity.take_apply_request());
    }

    #[test]
    fn settings_activity_back_discards_unsaved_changes() {
        let mut activity = SettingsActivity::new();
        activity.on_enter();
        activity.handle_input(InputEvent::Press(Button::Right)); // Font size -> Large
        assert_eq!(activity.settings().font_size, FontSize::Large);

        let result = activity.handle_input(InputEvent::Press(Button::Back));
        assert_eq!(result, ActivityResult::NavigateBack);
        assert_eq!(activity.settings().font_size, FontSize::Medium);
        assert_eq!(activity.applied_settings().font_size, FontSize::Medium);
    }

    #[test]
    fn settings_activity_with_custom_settings() {
        let custom = Settings {
            font_size: FontSize::Large,
            font_family: FontFamily::Serif,
            auto_sleep_duration: AutoSleepDuration::TenMinutes,
        };

        let activity = SettingsActivity::with_settings(custom);

        assert_eq!(activity.settings().font_size, FontSize::Large);
        assert_eq!(activity.settings().font_family, FontFamily::Serif);
        assert_eq!(
            activity.settings().auto_sleep_duration,
            AutoSleepDuration::TenMinutes
        );
    }

    #[test]
    fn settings_activity_render() {
        let mut activity = SettingsActivity::new();
        activity.on_enter();

        let mut display = crate::test_display::TestDisplay::default_size();
        let result = activity.render(&mut display);
        assert!(result.is_ok());
    }

    #[test]
    fn settings_activity_volume_buttons_navigation() {
        let mut activity = SettingsActivity::new();
        activity.on_enter();

        // VolumeDown navigates next
        let result = activity.handle_input(InputEvent::Press(Button::Aux2));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(activity.selected_index, 1);

        // VolumeUp navigates previous
        let result = activity.handle_input(InputEvent::Press(Button::Aux1));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(activity.selected_index, 0);
    }

    #[test]
    fn toast_timing() {
        let mut activity = SettingsActivity::new();

        activity.show_toast("Test message");
        assert!(activity.show_toast);
        assert_eq!(
            activity.toast_frames_remaining,
            SettingsActivity::TOAST_DURATION
        );

        // Simulate frame updates
        for _ in 0..SettingsActivity::TOAST_DURATION {
            activity.update();
        }

        assert!(!activity.show_toast);
    }

    #[test]
    fn auto_sleep_duration_labels() {
        assert_eq!(AutoSleepDuration::OneMinute.label(), "1 minute");
        assert_eq!(AutoSleepDuration::ThreeMinutes.label(), "3 minutes");
        assert_eq!(AutoSleepDuration::FiveMinutes.label(), "5 minutes");
        assert_eq!(AutoSleepDuration::TenMinutes.label(), "10 minutes");
        assert_eq!(AutoSleepDuration::FifteenMinutes.label(), "15 minutes");
        assert_eq!(AutoSleepDuration::ThirtyMinutes.label(), "30 minutes");
        assert_eq!(AutoSleepDuration::Never.label(), "Never");
    }

    #[test]
    fn auto_sleep_duration_milliseconds() {
        assert_eq!(AutoSleepDuration::OneMinute.milliseconds(), 60_000);
        assert_eq!(AutoSleepDuration::ThreeMinutes.milliseconds(), 180_000);
        assert_eq!(AutoSleepDuration::FiveMinutes.milliseconds(), 300_000);
        assert_eq!(AutoSleepDuration::TenMinutes.milliseconds(), 600_000);
        assert_eq!(AutoSleepDuration::FifteenMinutes.milliseconds(), 900_000);
        assert_eq!(AutoSleepDuration::ThirtyMinutes.milliseconds(), 1_800_000);
        assert_eq!(AutoSleepDuration::Never.milliseconds(), 0);
    }

    #[test]
    fn auto_sleep_duration_index_roundtrip() {
        for i in 0..7 {
            let duration = AutoSleepDuration::from_index(i).unwrap();
            assert_eq!(duration.index(), i);
        }
        assert!(AutoSleepDuration::from_index(7).is_none());
    }

    #[test]
    fn auto_sleep_duration_cycling() {
        let mut duration = AutoSleepDuration::OneMinute;

        duration = duration.next_wrapped();
        assert_eq!(duration, AutoSleepDuration::ThreeMinutes);

        duration = duration.next_wrapped();
        assert_eq!(duration, AutoSleepDuration::FiveMinutes);

        duration = duration.next_wrapped();
        assert_eq!(duration, AutoSleepDuration::TenMinutes);

        duration = duration.next_wrapped();
        assert_eq!(duration, AutoSleepDuration::FifteenMinutes);

        duration = duration.next_wrapped();
        assert_eq!(duration, AutoSleepDuration::ThirtyMinutes);

        duration = duration.next_wrapped();
        assert_eq!(duration, AutoSleepDuration::Never);

        // Wraps back to start
        duration = duration.next_wrapped();
        assert_eq!(duration, AutoSleepDuration::OneMinute);

        // Test backwards
        duration = duration.prev_wrapped();
        assert_eq!(duration, AutoSleepDuration::Never);

        duration = duration.prev_wrapped();
        assert_eq!(duration, AutoSleepDuration::ThirtyMinutes);
    }

    #[test]
    fn settings_activity_auto_sleep_adjust() {
        let mut activity = SettingsActivity::new();
        activity.on_enter();

        // Navigate to AutoSleep row
        activity.handle_input(InputEvent::Press(Button::Aux2));
        activity.handle_input(InputEvent::Press(Button::Aux2));
        assert_eq!(activity.current_row(), SettingRow::AutoSleep);
        assert_eq!(
            activity.settings().auto_sleep_duration,
            AutoSleepDuration::TenMinutes
        );

        // Right cycles forward
        let result = activity.handle_input(InputEvent::Press(Button::Right));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(
            activity.settings().auto_sleep_duration,
            AutoSleepDuration::TenMinutes
        );

        // Left cycles backward
        let result = activity.handle_input(InputEvent::Press(Button::Left));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(
            activity.settings().auto_sleep_duration,
            AutoSleepDuration::TenMinutes
        );
    }
}
