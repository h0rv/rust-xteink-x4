//! Settings Activity for Xteink X4 e-reader.
//!
//! Provides font size and font family configuration with
//! modern, minimal design optimized for e-ink displays.

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
use crate::ui::{Activity, ActivityResult, Modal, Theme, Toast};

/// Font size options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FontSize {
    #[default]
    Small,
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
}

/// Settings data container (in-memory storage)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    pub font_size: FontSize,
    pub font_family: FontFamily,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            font_size: FontSize::default(),
            font_family: FontFamily::default(),
        }
    }
}

impl Settings {
    /// Reset to factory defaults
    pub fn reset_to_defaults(&mut self) {
        *self = Self::default();
    }
}

/// Font size selector component with +/- buttons
#[derive(Debug, Clone)]
pub struct FontSizeSelector {
    pub current_size: FontSize,
    pub focused: bool,
}

impl FontSizeSelector {
    /// Create a new font size selector
    pub fn new(initial_size: FontSize) -> Self {
        Self {
            current_size: initial_size,
            focused: false,
        }
    }

    /// Increase font size
    pub fn increase(&mut self) -> Option<FontSize> {
        self.current_size.next().map(|size| {
            self.current_size = size;
            size
        })
    }

    /// Decrease font size
    pub fn decrease(&mut self) -> Option<FontSize> {
        self.current_size.prev().map(|size| {
            self.current_size = size;
            size
        })
    }

    /// Check if can increase
    pub fn can_increase(&self) -> bool {
        self.current_size.next().is_some()
    }

    /// Check if can decrease
    pub fn can_decrease(&self) -> bool {
        self.current_size.prev().is_some()
    }

    /// Get height
    pub const fn height(theme: &Theme) -> u32 {
        theme.metrics.list_item_height
    }

    /// Render the font size selector
    pub fn render<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
        theme: &Theme,
        x: i32,
        y: i32,
        width: u32,
    ) -> Result<(), D::Error> {
        let height = Self::height(theme);
        let button_width = 40u32;
        let label_width = width.saturating_sub(button_width * 2 + theme.metrics.spacing * 2);

        // Background
        let bg_color = if self.focused {
            BinaryColor::On
        } else {
            BinaryColor::Off
        };
        Rectangle::new(Point::new(x, y), Size::new(width, height))
            .into_styled(PrimitiveStyle::with_fill(bg_color))
            .draw(display)?;

        // Border
        Rectangle::new(Point::new(x, y), Size::new(width, height))
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
            .draw(display)?;

        let text_color = if self.focused {
            BinaryColor::Off
        } else {
            BinaryColor::On
        };

        // Minus button
        let minus_style = if self.can_decrease() {
            text_color
        } else {
            BinaryColor::Off // Disabled look
        };
        let minus_text = Text::new(
            "-",
            Point::new(
                x + (button_width as i32) / 2 - 3,
                y + (height as i32) / 2 + 5,
            ),
            MonoTextStyle::new(&ascii::FONT_7X13_BOLD, minus_style),
        );
        minus_text.draw(display)?;

        // Current size label (centered)
        let label_x = x + button_width as i32 + theme.metrics.spacing as i32;
        let label_style = MonoTextStyle::new(&ascii::FONT_7X13, text_color);
        Text::new(
            self.current_size.label(),
            Point::new(
                label_x + (label_width as i32) / 2
                    - ((self.current_size.label().len() * 7) as i32) / 2,
                y + (height as i32) / 2 + 5,
            ),
            label_style,
        )
        .draw(display)?;

        // Plus button
        let plus_x = x + width as i32 - button_width as i32;
        let plus_style = if self.can_increase() {
            text_color
        } else {
            BinaryColor::Off // Disabled look
        };
        let plus_text = Text::new(
            "+",
            Point::new(
                plus_x + (button_width as i32) / 2 - 3,
                y + (height as i32) / 2 + 5,
            ),
            MonoTextStyle::new(&ascii::FONT_7X13_BOLD, plus_style),
        );
        plus_text.draw(display)?;

        // Separator lines for buttons
        Rectangle::new(Point::new(x + button_width as i32, y), Size::new(1, height))
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(display)?;
        Rectangle::new(Point::new(plus_x, y), Size::new(1, height))
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(display)?;

        Ok(())
    }
}

/// Font family selector component
#[derive(Debug, Clone)]
pub struct FontFamilySelector {
    pub current_family: FontFamily,
    pub focused: bool,
}

impl FontFamilySelector {
    /// Create a new font family selector
    pub fn new(initial_family: FontFamily) -> Self {
        Self {
            current_family: initial_family,
            focused: false,
        }
    }

    /// Select next font family
    pub fn next(&mut self) -> FontFamily {
        let next_index = (self.current_family.index() + 1) % FontFamily::ALL.len();
        self.current_family = FontFamily::from_index(next_index).unwrap_or(FontFamily::default());
        self.current_family
    }

    /// Select previous font family
    pub fn prev(&mut self) -> FontFamily {
        let prev_index = if self.current_family.index() == 0 {
            FontFamily::ALL.len() - 1
        } else {
            self.current_family.index() - 1
        };
        self.current_family = FontFamily::from_index(prev_index).unwrap_or(FontFamily::default());
        self.current_family
    }

    /// Get total height for all options
    pub fn height(theme: &Theme) -> u32 {
        FontFamily::ALL.len() as u32 * theme.metrics.list_item_height
    }

    /// Render the font family selector
    pub fn render<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
        theme: &Theme,
        x: i32,
        y: i32,
        width: u32,
    ) -> Result<(), D::Error> {
        let item_height = theme.metrics.list_item_height as i32;

        for (i, family) in FontFamily::ALL.iter().enumerate() {
            let item_y = y + (i as i32) * item_height;
            let is_selected = *family == self.current_family;

            // Background
            let bg_color = if is_selected {
                BinaryColor::On
            } else {
                BinaryColor::Off
            };
            Rectangle::new(Point::new(x, item_y), Size::new(width, item_height as u32))
                .into_styled(PrimitiveStyle::with_fill(bg_color))
                .draw(display)?;

            // Selection indicator
            if is_selected {
                Text::new(
                    "> ",
                    Point::new(x + theme.metrics.side_padding as i32, item_y + 28),
                    MonoTextStyle::new(&ascii::FONT_7X13, BinaryColor::Off),
                )
                .draw(display)?;
            }

            // Text
            let text_color = if is_selected {
                BinaryColor::Off
            } else {
                BinaryColor::On
            };
            let text_x = x + theme.metrics.side_padding as i32 + if is_selected { 14 } else { 0 };
            Text::new(
                family.label(),
                Point::new(text_x, item_y + 28),
                MonoTextStyle::new(&ascii::FONT_7X13, text_color),
            )
            .draw(display)?;

            // Separator line (except for last item)
            if i < FontFamily::ALL.len() - 1 {
                Rectangle::new(
                    Point::new(x + 10, item_y + item_height - 1),
                    Size::new(width - 20, 1),
                )
                .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
                .draw(display)?;
            }
        }

        Ok(())
    }
}

/// Settings group with header
#[derive(Debug, Clone)]
pub struct SettingsGroup {
    pub title: String,
}

impl SettingsGroup {
    /// Create a new settings group
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
        }
    }

    /// Render the group header
    pub fn render_header<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
        _theme: &Theme,
        x: i32,
        y: i32,
        _width: u32,
    ) -> Result<(), D::Error> {
        let title_style = MonoTextStyleBuilder::new()
            .font(&ascii::FONT_7X13_BOLD)
            .text_color(BinaryColor::On)
            .build();

        Text::new(&self.title, Point::new(x, y + 15), title_style).draw(display)?;

        Ok(())
    }

    /// Height of the header
    pub const fn header_height() -> u32 {
        25
    }
}

/// Focusable elements in the settings screen
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusElement {
    FontSizeMinus,
    FontSizePlus,
    FontFamilyList(usize),
    ResetButton,
}

/// Settings Activity implementing the Activity trait
#[derive(Debug, Clone)]
pub struct SettingsActivity {
    settings: Settings,
    original_settings: Settings,
    focus_index: usize,
    show_toast: bool,
    toast_message: String,
    toast_frames_remaining: u32,
    show_reset_modal: bool,
    font_size_selector: FontSizeSelector,
    font_family_selector: FontFamilySelector,
    theme: Theme,
}

impl SettingsActivity {
    /// Number of focusable elements
    const FOCUS_COUNT: usize = 5; // minus, plus, font families (3)

    /// Toast display duration in frames
    const TOAST_DURATION: u32 = 120; // ~2 seconds at 60fps

    /// Create a new settings activity
    pub fn new() -> Self {
        let settings = Settings::default();
        Self {
            settings,
            original_settings: settings,
            focus_index: 0,
            show_toast: false,
            toast_message: String::new(),
            toast_frames_remaining: 0,
            show_reset_modal: false,
            font_size_selector: FontSizeSelector::new(settings.font_size),
            font_family_selector: FontFamilySelector::new(settings.font_family),
            theme: Theme::default(),
        }
    }

    /// Create with specific initial settings
    pub fn with_settings(settings: Settings) -> Self {
        Self {
            settings,
            original_settings: settings,
            focus_index: 0,
            show_toast: false,
            toast_message: String::new(),
            toast_frames_remaining: 0,
            show_reset_modal: false,
            font_size_selector: FontSizeSelector::new(settings.font_size),
            font_family_selector: FontFamilySelector::new(settings.font_family),
            theme: Theme::default(),
        }
    }

    /// Get current settings
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Check if settings were modified
    pub fn is_modified(&self) -> bool {
        self.settings != self.original_settings
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

    /// Get current focus element
    fn current_focus(&self) -> FocusElement {
        match self.focus_index {
            0 => FocusElement::FontSizeMinus,
            1 => FocusElement::FontSizePlus,
            2 => FocusElement::FontFamilyList(0),
            3 => FocusElement::FontFamilyList(1),
            4 => FocusElement::FontFamilyList(2),
            _ => FocusElement::ResetButton,
        }
    }

    /// Move focus to next element
    fn focus_next(&mut self) {
        self.focus_index = (self.focus_index + 1) % Self::FOCUS_COUNT;
        self.update_focus_states();
    }

    /// Move focus to previous element
    fn focus_prev(&mut self) {
        self.focus_index = if self.focus_index == 0 {
            Self::FOCUS_COUNT - 1
        } else {
            self.focus_index - 1
        };
        self.update_focus_states();
    }

    /// Update component focus states based on current focus
    fn update_focus_states(&mut self) {
        // Font size selector focus is handled separately in input handling
        // Font family selector focus is based on focus_index
        let family_index = self.focus_index.saturating_sub(2);
        self.font_family_selector.focused = family_index < FontFamily::ALL.len();
    }

    /// Handle font size decrease
    fn handle_font_size_minus(&mut self) -> ActivityResult {
        if let Some(new_size) = self.font_size_selector.decrease() {
            self.settings.font_size = new_size;
            self.show_toast(format!("Font size: {}", new_size.label()));
        }
        ActivityResult::Consumed
    }

    /// Handle font size increase
    fn handle_font_size_plus(&mut self) -> ActivityResult {
        if let Some(new_size) = self.font_size_selector.increase() {
            self.settings.font_size = new_size;
            self.show_toast(format!("Font size: {}", new_size.label()));
        }
        ActivityResult::Consumed
    }

    /// Handle font family selection
    fn handle_font_family_select(&mut self, index: usize) {
        if let Some(family) = FontFamily::from_index(index) {
            self.font_family_selector.current_family = family;
            self.settings.font_family = family;
            self.show_toast(format!("Font: {}", family.label()));
        }
    }

    /// Handle reset to defaults
    fn handle_reset(&mut self) {
        self.show_reset_modal = true;
    }

    /// Confirm reset to defaults
    fn confirm_reset(&mut self) {
        self.settings.reset_to_defaults();
        self.font_size_selector.current_size = self.settings.font_size;
        self.font_family_selector.current_family = self.settings.font_family;
        self.show_toast("Settings reset to defaults");
        self.show_reset_modal = false;
    }

    /// Cancel reset
    fn cancel_reset(&mut self) {
        self.show_reset_modal = false;
    }

    /// Render header bar
    fn render_header<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
        theme: &Theme,
    ) -> Result<(), D::Error> {
        let display_width = display.bounding_box().size.width;
        let header_height = theme.metrics.header_height;

        // Header background
        Rectangle::new(Point::new(0, 0), Size::new(display_width, header_height))
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(display)?;

        // Title
        let title_style = MonoTextStyleBuilder::new()
            .font(&ascii::FONT_7X13_BOLD)
            .text_color(BinaryColor::Off)
            .build();
        Text::new(
            "Settings",
            Point::new(theme.metrics.side_padding as i32, 32),
            title_style,
        )
        .draw(display)?;

        // Back button label
        let back_style = MonoTextStyle::new(&ascii::FONT_7X13, BinaryColor::Off);
        let back_text = "[Back]";
        let back_width = back_text.len() as i32 * 7;
        Text::new(
            back_text,
            Point::new(
                display_width as i32 - back_width - theme.metrics.side_padding as i32,
                32,
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
        let mut y = theme.metrics.header_height as i32 + theme.metrics.spacing as i32;

        // Font Size Group
        let font_size_group = SettingsGroup::new("Font Size");
        font_size_group.render_header(display, theme, x, y, content_width)?;
        y += SettingsGroup::header_height() as i32;

        // Font size selector
        self.font_size_selector
            .render(display, theme, x, y, content_width)?;
        y += FontSizeSelector::height(theme) as i32 + theme.metrics.spacing_double() as i32;

        // Font Family Group
        let font_family_group = SettingsGroup::new("Font Family");
        font_family_group.render_header(display, theme, x, y, content_width)?;
        y += SettingsGroup::header_height() as i32;

        // Font family selector
        self.font_family_selector
            .render(display, theme, x, y, content_width)?;
        y += FontFamilySelector::height(theme) as i32 + theme.metrics.spacing_double() as i32;

        // Reset button
        self.render_reset_button(display, theme, x, y, content_width)?;

        Ok(())
    }

    /// Render reset button
    fn render_reset_button<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
        theme: &Theme,
        x: i32,
        y: i32,
        width: u32,
    ) -> Result<(), D::Error> {
        let height = theme.metrics.button_height;
        let is_focused = self.focus_index == 5;

        // Background
        let bg_color = if is_focused {
            BinaryColor::On
        } else {
            BinaryColor::Off
        };
        Rectangle::new(Point::new(x, y), Size::new(width, height))
            .into_styled(PrimitiveStyle::with_fill(bg_color))
            .draw(display)?;

        // Border
        Rectangle::new(Point::new(x, y), Size::new(width, height))
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
            .draw(display)?;

        // Text
        let text_color = if is_focused {
            BinaryColor::Off
        } else {
            BinaryColor::On
        };
        let label = "Reset to Defaults";
        let text_width = label.len() as i32 * 7;
        let text_x = x + (width as i32 - text_width) / 2;
        Text::new(
            label,
            Point::new(text_x, y + (height as i32) / 2 + 5),
            MonoTextStyle::new(&ascii::FONT_7X13, text_color),
        )
        .draw(display)?;

        Ok(())
    }
}

impl Activity for SettingsActivity {
    fn on_enter(&mut self) {
        self.original_settings = self.settings;
        self.focus_index = 0;
        self.show_toast = false;
        self.show_reset_modal = false;
        self.update_focus_states();
    }

    fn on_exit(&mut self) {
        // Settings are persisted in memory (self.settings)
        // Could add persistence logic here in the future
    }

    fn handle_input(&mut self, event: InputEvent) -> ActivityResult {
        if self.show_reset_modal {
            return self.handle_modal_input(event);
        }

        match event {
            InputEvent::Press(Button::Back) => ActivityResult::NavigateBack,
            InputEvent::Press(Button::Left) => {
                self.focus_prev();
                ActivityResult::Consumed
            }
            InputEvent::Press(Button::Right) => {
                self.focus_next();
                ActivityResult::Consumed
            }
            InputEvent::Press(Button::Confirm) => {
                self.handle_confirm_press();
                ActivityResult::Consumed
            }
            InputEvent::Press(Button::VolumeUp) => {
                self.focus_prev();
                ActivityResult::Consumed
            }
            InputEvent::Press(Button::VolumeDown) => {
                self.focus_next();
                ActivityResult::Consumed
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

        // Content
        self.render_content(display, &self.theme)?;

        // Toast notification
        if self.show_toast {
            let display_width = display.bounding_box().size.width;
            let display_height = display.bounding_box().size.height;
            let toast = Toast::bottom_center(&self.toast_message, display_width, display_height);
            toast.render(display)?;
        }

        // Modal dialog
        if self.show_reset_modal {
            let modal = Modal::new("Reset Settings", "Restore all settings to defaults?")
                .with_button("Cancel")
                .with_button("Reset");
            modal.render(display, &self.theme)?;
        }

        Ok(())
    }
}

impl SettingsActivity {
    /// Handle input when modal is shown
    fn handle_modal_input(&mut self, event: InputEvent) -> ActivityResult {
        // For simplicity, Confirm resets, Back/Left cancels
        match event {
            InputEvent::Press(Button::Confirm) => {
                self.confirm_reset();
                ActivityResult::Consumed
            }
            InputEvent::Press(Button::Back) | InputEvent::Press(Button::Left) => {
                self.cancel_reset();
                ActivityResult::Consumed
            }
            _ => ActivityResult::Ignored,
        }
    }

    /// Handle confirm button press based on current focus
    fn handle_confirm_press(&mut self) {
        match self.current_focus() {
            FocusElement::FontSizeMinus => {
                self.handle_font_size_minus();
            }
            FocusElement::FontSizePlus => {
                self.handle_font_size_plus();
            }
            FocusElement::FontFamilyList(index) => {
                self.handle_font_family_select(index);
            }
            FocusElement::ResetButton => {
                self.handle_reset();
            }
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
    use embedded_graphics::mock_display::MockDisplay;

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
    fn font_family_selector_navigation() {
        let mut selector = FontFamilySelector::new(FontFamily::Monospace);
        assert_eq!(selector.current_family, FontFamily::Monospace);

        selector.next();
        assert_eq!(selector.current_family, FontFamily::Serif);

        selector.next();
        assert_eq!(selector.current_family, FontFamily::SansSerif);

        selector.next();
        assert_eq!(selector.current_family, FontFamily::Monospace);

        selector.prev();
        assert_eq!(selector.current_family, FontFamily::SansSerif);

        selector.prev();
        assert_eq!(selector.current_family, FontFamily::Serif);
    }

    #[test]
    fn font_size_selector_increase_decrease() {
        let mut selector = FontSizeSelector::new(FontSize::Medium);
        assert_eq!(selector.current_size, FontSize::Medium);

        selector.increase();
        assert_eq!(selector.current_size, FontSize::Large);

        selector.decrease();
        assert_eq!(selector.current_size, FontSize::Medium);

        // Test boundaries
        let mut small = FontSizeSelector::new(FontSize::Small);
        assert!(small.decrease().is_none());
        assert_eq!(small.current_size, FontSize::Small);

        let mut xl = FontSizeSelector::new(FontSize::ExtraLarge);
        assert!(xl.increase().is_none());
        assert_eq!(xl.current_size, FontSize::ExtraLarge);
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
    fn settings_activity_lifecycle() {
        let mut activity = SettingsActivity::new();

        activity.on_enter();
        assert!(!activity.show_reset_modal);
        assert!(!activity.show_toast);

        activity.on_exit();
        // Settings should still be accessible
        assert_eq!(activity.settings().font_size, FontSize::Medium);
    }

    #[test]
    fn settings_activity_input_navigation() {
        let mut activity = SettingsActivity::new();
        activity.on_enter();

        // Initial focus
        assert_eq!(activity.focus_index, 0);

        // Navigate next
        let result = activity.handle_input(InputEvent::Press(Button::Right));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(activity.focus_index, 1);

        // Navigate prev
        let result = activity.handle_input(InputEvent::Press(Button::Left));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(activity.focus_index, 0);

        // Navigate back
        let result = activity.handle_input(InputEvent::Press(Button::Back));
        assert_eq!(result, ActivityResult::NavigateBack);
    }

    #[test]
    fn settings_activity_font_size_change() {
        let mut activity = SettingsActivity::new();
        activity.on_enter();

        // Focus should be on font size minus initially
        // Navigate to plus button
        activity.handle_input(InputEvent::Press(Button::Right));
        assert_eq!(activity.focus_index, 1);

        // Press confirm to increase
        activity.handle_input(InputEvent::Press(Button::Confirm));

        assert_eq!(activity.settings().font_size, FontSize::Large);
        assert!(activity.show_toast);
        assert_eq!(activity.toast_message, "Font size: Large");
    }

    #[test]
    fn settings_activity_font_family_change() {
        let mut activity = SettingsActivity::new();
        activity.on_enter();

        // Navigate to font family list (index 2 = first font family item)
        activity.handle_input(InputEvent::Press(Button::Right));
        activity.handle_input(InputEvent::Press(Button::Right));
        assert_eq!(activity.focus_index, 2);

        // Confirm to select current font family
        activity.handle_input(InputEvent::Press(Button::Confirm));

        assert_eq!(activity.settings().font_family, FontFamily::Monospace);
    }

    #[test]
    fn settings_activity_modified_check() {
        let mut activity = SettingsActivity::new();
        activity.on_enter();

        assert!(!activity.is_modified());

        // Change font size
        activity.font_size_selector.increase();
        activity.settings.font_size = activity.font_size_selector.current_size;

        assert!(activity.is_modified());
    }

    #[test]
    fn settings_activity_reset_modal() {
        let mut activity = SettingsActivity::new();
        activity.on_enter();

        // Navigate to reset button and confirm
        for _ in 0..5 {
            activity.handle_input(InputEvent::Press(Button::Right));
        }
        activity.handle_input(InputEvent::Press(Button::Confirm));

        assert!(activity.show_reset_modal);

        // Cancel modal
        activity.handle_input(InputEvent::Press(Button::Back));
        assert!(!activity.show_reset_modal);

        // Reopen and confirm reset
        activity.handle_input(InputEvent::Press(Button::Confirm));
        assert!(activity.show_reset_modal);

        activity.handle_input(InputEvent::Press(Button::Confirm));
        assert!(!activity.show_reset_modal);
        assert!(activity.show_toast);
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
    fn settings_group_creation() {
        let group = SettingsGroup::new("Test Group");
        assert_eq!(group.title, "Test Group");
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
    fn settings_activity_render() {
        let mut activity = SettingsActivity::new();
        activity.on_enter();

        let mut display = MockDisplay::new();
        let result = activity.render(&mut display);
        assert!(result.is_ok());
    }

    #[test]
    fn focus_element_enum() {
        let elements = [
            FocusElement::FontSizeMinus,
            FocusElement::FontSizePlus,
            FocusElement::FontFamilyList(0),
            FocusElement::FontFamilyList(1),
            FocusElement::FontFamilyList(2),
            FocusElement::ResetButton,
        ];

        // Just verify the enum variants exist and can be compared
        assert_ne!(elements[0], elements[1]);
        assert_eq!(elements[2], FocusElement::FontFamilyList(0));
    }

    #[test]
    fn volume_buttons_navigation() {
        let mut activity = SettingsActivity::new();
        activity.on_enter();

        let result = activity.handle_input(InputEvent::Press(Button::VolumeDown));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(activity.focus_index, 1);

        let result = activity.handle_input(InputEvent::Press(Button::VolumeUp));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(activity.focus_index, 0);
    }
}
