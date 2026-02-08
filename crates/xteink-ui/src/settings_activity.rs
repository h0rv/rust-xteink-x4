//! Settings Activity for Xteink X4 e-reader.
//!
//! Provides font size and font family configuration with
//! modern, minimal design optimized for e-ink displays.

extern crate alloc;

use alloc::format;
use alloc::string::String;

use embedded_graphics::{
    mono_font::{MonoTextStyle, MonoTextStyleBuilder},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    text::Text,
};

use crate::input::{Button, InputEvent};
use crate::ui::theme::{ui_font, ui_font_bold};
use crate::ui::{Activity, ActivityResult, Modal, Theme, ThemeMetrics, Toast};

/// Font size options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FontSize {
    Small,
    #[default]
    Medium,
    Large,
    ExtraLarge,
}

impl FontSize {
    /// All font size variants in order
    pub const ALL: [Self; 4] = [Self::Small, Self::Medium, Self::Large, Self::ExtraLarge];

    /// Get display label for the font size
    pub const fn label(self) -> &'static str {
        match self {
            Self::Small => "Small",
            Self::Medium => "Medium",
            Self::Large => "Large",
            Self::ExtraLarge => "Extra Large",
        }
    }

    /// Get the font size in points
    pub const fn points(self) -> u8 {
        match self {
            Self::Small => 12,
            Self::Medium => 14,
            Self::Large => 18,
            Self::ExtraLarge => 24,
        }
    }

    /// Get the next larger font size
    pub const fn next(self) -> Option<Self> {
        match self {
            Self::Small => Some(Self::Medium),
            Self::Medium => Some(Self::Large),
            Self::Large => Some(Self::ExtraLarge),
            Self::ExtraLarge => None,
        }
    }

    /// Get the previous smaller font size
    pub const fn prev(self) -> Option<Self> {
        match self {
            Self::Small => None,
            Self::Medium => Some(Self::Small),
            Self::Large => Some(Self::Medium),
            Self::ExtraLarge => Some(Self::Large),
        }
    }

    /// Get index in ALL array
    pub const fn index(self) -> usize {
        match self {
            Self::Small => 0,
            Self::Medium => 1,
            Self::Large => 2,
            Self::ExtraLarge => 3,
        }
    }

    /// Create from index
    pub const fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::Small),
            1 => Some(Self::Medium),
            2 => Some(Self::Large),
            3 => Some(Self::ExtraLarge),
            _ => None,
        }
    }
}

/// Font family options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FontFamily {
    #[default]
    Monospace,
    Serif,
    SansSerif,
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
        match index {
            0 => Some(Self::Monospace),
            1 => Some(Self::Serif),
            2 => Some(Self::SansSerif),
            _ => None,
        }
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
    ResetButton,
    SaveButton,
}

impl SettingRow {
    /// All setting rows in display order
    pub const ALL: [Self; 4] = [
        Self::FontSize,
        Self::FontFamily,
        Self::ResetButton,
        Self::SaveButton,
    ];

    /// Get the label for this setting row
    pub const fn label(self) -> &'static str {
        match self {
            Self::FontSize => "Font Size",
            Self::FontFamily => "Font Family",
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
        let display_width = display.bounding_box().size.width;
        let header_height = theme.metrics.header_height;
        let header_y = theme.metrics.header_text_y();

        // Header background
        Rectangle::new(Point::new(0, 0), Size::new(display_width, header_height))
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(display)?;

        // Title
        let title_style = MonoTextStyleBuilder::new()
            .font(ui_font_bold())
            .text_color(BinaryColor::Off)
            .build();
        Text::new(
            "Settings",
            Point::new(theme.metrics.side_padding as i32, header_y),
            title_style,
        )
        .draw(display)?;

        // Save hint
        let back_style = MonoTextStyle::new(ui_font(), BinaryColor::Off);
        let back_text = "[Save]";
        let back_width = ThemeMetrics::text_width(back_text.len());
        Text::new(
            back_text,
            Point::new(
                display_width as i32 - back_width - theme.metrics.side_padding as i32,
                header_y,
            ),
            back_style,
        )
        .draw(display)?;

        Ok(())
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
                let text_y = theme.metrics.button_text_y();

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
                let label_width = ThemeMetrics::text_width(label.len());
                let label_x = x + (content_width as i32 - label_width) / 2;
                Text::new(
                    label,
                    Point::new(label_x, y + text_y),
                    MonoTextStyle::new(ui_font_bold(), text_color),
                )
                .draw(display)?;

                y += height as i32;
            } else {
                // Value row: label on left, < Value > on right
                let height = theme.metrics.list_item_height;
                let text_y = theme.metrics.item_text_y();

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
                Text::new(
                    row.label(),
                    Point::new(x + theme.metrics.side_padding as i32, y + text_y),
                    MonoTextStyle::new(ui_font(), text_color),
                )
                .draw(display)?;

                // < Value > on right
                let value = self.get_value_label(row);
                let value_text = format!("< {} >", value);
                let value_width = ThemeMetrics::text_width(value_text.len());
                let value_x =
                    x + content_width as i32 - value_width - theme.metrics.side_padding as i32;
                Text::new(
                    &value_text,
                    Point::new(value_x, y + text_y),
                    MonoTextStyle::new(ui_font(), text_color),
                )
                .draw(display)?;

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
            InputEvent::Press(Button::VolumeUp) | InputEvent::Press(Button::Up) => {
                self.select_prev();
                ActivityResult::Consumed
            }
            InputEvent::Press(Button::VolumeDown) | InputEvent::Press(Button::Down) => {
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
        // Settings redraws can touch large areas; avoid diff buffer allocations
        // on constrained firmware builds.
        crate::ui::ActivityRefreshMode::Partial
    }
}

impl SettingsActivity {
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
                    self.confirm_reset();
                } else {
                    self.cancel_reset();
                }
                ActivityResult::Consumed
            }
            InputEvent::Press(Button::Back) => {
                self.cancel_reset();
                ActivityResult::Consumed
            }
            _ => ActivityResult::Ignored,
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

        assert!(size.next().is_none());

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
    }

    #[test]
    fn font_size_points() {
        assert_eq!(FontSize::Small.points(), 12);
        assert_eq!(FontSize::Medium.points(), 14);
        assert_eq!(FontSize::Large.points(), 18);
        assert_eq!(FontSize::ExtraLarge.points(), 24);
    }

    #[test]
    fn font_size_index_roundtrip() {
        for i in 0..4 {
            let size = FontSize::from_index(i).unwrap();
            assert_eq!(size.index(), i);
        }
        assert!(FontSize::from_index(4).is_none());
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
        assert_eq!(settings.font_family, FontFamily::Monospace);
    }

    #[test]
    fn settings_reset() {
        let mut settings = Settings {
            font_size: FontSize::ExtraLarge,
            font_family: FontFamily::SansSerif,
        };

        settings.reset_to_defaults();

        assert_eq!(settings.font_size, FontSize::Medium);
        assert_eq!(settings.font_family, FontFamily::Monospace);
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
        assert_eq!(activity.settings().font_family, FontFamily::Monospace);

        // Right cycles forward
        let result = activity.handle_input(InputEvent::Press(Button::Right));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(activity.settings().font_family, FontFamily::Serif);

        // Right again
        activity.handle_input(InputEvent::Press(Button::Right));
        assert_eq!(activity.settings().font_family, FontFamily::SansSerif);

        // Right wraps to beginning
        activity.handle_input(InputEvent::Press(Button::Right));
        assert_eq!(activity.settings().font_family, FontFamily::Monospace);

        // Left cycles backward (wraps)
        let result = activity.handle_input(InputEvent::Press(Button::Left));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(activity.settings().font_family, FontFamily::SansSerif);

        // Left again
        activity.handle_input(InputEvent::Press(Button::Left));
        assert_eq!(activity.settings().font_family, FontFamily::Serif);
    }

    #[test]
    fn settings_activity_font_size_at_bounds() {
        // Right at ExtraLarge doesn't change
        let mut activity = SettingsActivity::with_settings(Settings {
            font_size: FontSize::ExtraLarge,
            font_family: FontFamily::Monospace,
        });
        activity.on_enter();

        let result = activity.handle_input(InputEvent::Press(Button::Right));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(activity.settings().font_size, FontSize::ExtraLarge);

        // Left at Small doesn't change
        let mut activity = SettingsActivity::with_settings(Settings {
            font_size: FontSize::Small,
            font_family: FontFamily::Monospace,
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
        activity.handle_input(InputEvent::Press(Button::VolumeUp));
        assert_eq!(activity.modal_button, 1);

        // VolumeDown
        activity.handle_input(InputEvent::Press(Button::VolumeDown));
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
        };

        let activity = SettingsActivity::with_settings(custom);

        assert_eq!(activity.settings().font_size, FontSize::Large);
        assert_eq!(activity.settings().font_family, FontFamily::Serif);
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
        let result = activity.handle_input(InputEvent::Press(Button::VolumeDown));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(activity.selected_index, 1);

        // VolumeUp navigates previous
        let result = activity.handle_input(InputEvent::Press(Button::VolumeUp));
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
}
