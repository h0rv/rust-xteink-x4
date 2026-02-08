//! Information Activity for Xteink X4 e-reader.
//!
//! Displays device information including firmware version, battery status,
//! storage usage, and display specifications.
//! Read-only screen with scrollable info list optimized for e-ink displays.

extern crate alloc;

use alloc::format;

use embedded_graphics::{
    mono_font::{ascii, MonoTextStyle, MonoTextStyleBuilder},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    text::Text,
};

use crate::input::{Button, InputEvent};
use crate::system_menu_activity::DeviceStatus;
use crate::ui::{Activity, ActivityResult, Theme, ThemeMetrics, FONT_CHAR_WIDTH};

/// Information row label/value pairs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InfoField {
    /// Device model name
    DeviceName,
    /// Firmware version
    FirmwareVersion,
    /// Battery level and charging status
    Battery,
    /// Storage usage percentage
    Storage,
    /// Display resolution and PPI
    Display,
    /// About / legal
    About,
}

impl InfoField {
    /// All info fields in display order
    pub const ALL: [Self; 6] = [
        Self::DeviceName,
        Self::FirmwareVersion,
        Self::Battery,
        Self::Storage,
        Self::Display,
        Self::About,
    ];

    /// Get display label for the info field
    pub const fn label(self) -> &'static str {
        match self {
            Self::DeviceName => "Device",
            Self::FirmwareVersion => "Firmware",
            Self::Battery => "Battery",
            Self::Storage => "Storage",
            Self::Display => "Display",
            Self::About => "About",
        }
    }

    /// Get index in ALL array
    pub const fn index(self) -> usize {
        match self {
            Self::DeviceName => 0,
            Self::FirmwareVersion => 1,
            Self::Battery => 2,
            Self::Storage => 3,
            Self::Display => 4,
            Self::About => 5,
        }
    }

    /// Create from index
    pub const fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::DeviceName),
            1 => Some(Self::FirmwareVersion),
            2 => Some(Self::Battery),
            3 => Some(Self::Storage),
            4 => Some(Self::Display),
            5 => Some(Self::About),
            _ => None,
        }
    }
}

/// Information Activity implementing the Activity trait
#[derive(Debug, Clone)]
pub struct InformationActivity {
    /// Device status information
    device_status: DeviceStatus,
    /// Currently highlighted row index
    selected_index: usize,
    /// Scroll offset for long lists
    scroll_offset: usize,
    /// Theme for styling
    theme: Theme,
}

impl InformationActivity {
    /// Create a new information activity
    pub fn new() -> Self {
        Self {
            device_status: DeviceStatus::default(),
            selected_index: 0,
            scroll_offset: 0,
            theme: Theme::default(),
        }
    }

    /// Create with custom device status
    pub fn with_device_status(mut self, status: DeviceStatus) -> Self {
        self.device_status = status;
        self
    }

    /// Get current device status
    pub fn device_status(&self) -> &DeviceStatus {
        &self.device_status
    }

    /// Update device status
    pub fn set_device_status(&mut self, status: DeviceStatus) {
        self.device_status = status;
    }

    /// Get the selected field
    pub fn selected_field(&self) -> Option<InfoField> {
        InfoField::from_index(self.selected_index)
    }

    /// Get the value string for a given info field
    fn field_value(&self, field: InfoField) -> alloc::string::String {
        match field {
            InfoField::DeviceName => alloc::string::String::from("Xteink X4"),
            InfoField::FirmwareVersion => {
                format!("v{}", self.device_status.firmware_version)
            }
            InfoField::Battery => {
                if self.device_status.is_charging {
                    format!("{}% (Charging)", self.device_status.battery_percent)
                } else {
                    format!("{}%", self.device_status.battery_percent)
                }
            }
            InfoField::Storage => {
                format!("{}% used", self.device_status.storage_used_percent)
            }
            InfoField::Display => alloc::string::String::from("480x800 @ 220 PPI"),
            InfoField::About => alloc::string::String::from("Open Source E-Reader"),
        }
    }

    /// Move selection down (wraps around)
    fn select_next(&mut self) {
        self.selected_index = (self.selected_index + 1) % InfoField::ALL.len();
        self.ensure_visible();
    }

    /// Move selection up (wraps around)
    fn select_prev(&mut self) {
        self.selected_index = if self.selected_index == 0 {
            InfoField::ALL.len() - 1
        } else {
            self.selected_index - 1
        };
        self.ensure_visible();
    }

    /// Ensure selected item is visible within scroll window
    fn ensure_visible(&mut self) {
        let visible_count = self.visible_count();
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        } else if self.selected_index >= self.scroll_offset + visible_count {
            self.scroll_offset = self.selected_index.saturating_sub(visible_count - 1);
        }
    }

    /// Calculate how many rows are visible given default display height
    fn visible_count(&self) -> usize {
        self.theme.metrics.visible_items(crate::DISPLAY_HEIGHT)
    }

    /// Render header bar with title
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
            .font(&ascii::FONT_7X13_BOLD)
            .text_color(BinaryColor::Off)
            .build();
        Text::new(
            "Information",
            Point::new(theme.metrics.side_padding as i32, header_y),
            title_style,
        )
        .draw(display)?;

        // Back indicator
        let back_style = MonoTextStyle::new(&ascii::FONT_7X13, BinaryColor::Off);
        let back_text = "[Back]";
        let text_width = ThemeMetrics::text_width(back_text.len());
        Text::new(
            back_text,
            Point::new(
                display_width as i32 - text_width - theme.metrics.side_padding as i32,
                header_y,
            ),
            back_style,
        )
        .draw(display)?;

        Ok(())
    }

    /// Render the info list
    fn render_info_list<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
        theme: &Theme,
    ) -> Result<(), D::Error> {
        let display_width = display.bounding_box().size.width;
        let content_width = theme.metrics.content_width(display_width);
        let x = theme.metrics.side_padding as i32;
        let item_height = theme.metrics.list_item_height;
        let mut y = theme.metrics.header_height as i32 + theme.metrics.spacing_double() as i32;

        let visible_count = self.visible_count();

        for (i, field) in InfoField::ALL
            .iter()
            .skip(self.scroll_offset)
            .take(visible_count)
            .enumerate()
        {
            let actual_index = self.scroll_offset + i;
            let is_selected = actual_index == self.selected_index;

            self.render_info_row(
                display,
                theme,
                x,
                y,
                content_width,
                item_height,
                *field,
                is_selected,
            )?;

            y += item_height as i32 + theme.metrics.spacing as i32;
        }

        Ok(())
    }

    /// Render a single info row with label and value
    #[allow(clippy::too_many_arguments)]
    fn render_info_row<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
        theme: &Theme,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        field: InfoField,
        is_selected: bool,
    ) -> Result<(), D::Error> {
        let text_y = y + theme.metrics.item_text_y();

        // Background
        let bg_color = if is_selected {
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

        let text_color = if is_selected {
            BinaryColor::Off
        } else {
            BinaryColor::On
        };

        // Label (left side, bold)
        let label_style = MonoTextStyleBuilder::new()
            .font(&ascii::FONT_7X13_BOLD)
            .text_color(text_color)
            .build();
        Text::new(
            field.label(),
            Point::new(x + theme.metrics.side_padding as i32, text_y),
            label_style,
        )
        .draw(display)?;

        // Value (right side)
        let value_style = MonoTextStyle::new(&ascii::FONT_7X13, text_color);
        let value = self.field_value(field);
        let value_width = value.len() as i32 * FONT_CHAR_WIDTH;
        Text::new(
            &value,
            Point::new(
                x + width as i32 - value_width - theme.metrics.side_padding as i32,
                text_y,
            ),
            value_style,
        )
        .draw(display)?;

        Ok(())
    }
}

impl Activity for InformationActivity {
    fn on_enter(&mut self) {
        self.selected_index = 0;
        self.scroll_offset = 0;
    }

    fn on_exit(&mut self) {
        // No cleanup needed for read-only screen
    }

    fn handle_input(&mut self, event: InputEvent) -> ActivityResult {
        match event {
            InputEvent::Press(Button::Back) => ActivityResult::NavigateBack,
            InputEvent::Press(Button::VolumeDown) | InputEvent::Press(Button::Right) => {
                self.select_next();
                ActivityResult::Consumed
            }
            InputEvent::Press(Button::VolumeUp) | InputEvent::Press(Button::Left) => {
                self.select_prev();
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

        // Info list
        self.render_info_list(display, &self.theme)?;

        Ok(())
    }
}

impl Default for InformationActivity {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_display::TestDisplay;

    #[test]
    fn info_field_labels() {
        assert_eq!(InfoField::DeviceName.label(), "Device");
        assert_eq!(InfoField::FirmwareVersion.label(), "Firmware");
        assert_eq!(InfoField::Battery.label(), "Battery");
        assert_eq!(InfoField::Storage.label(), "Storage");
        assert_eq!(InfoField::Display.label(), "Display");
        assert_eq!(InfoField::About.label(), "About");
    }

    #[test]
    fn info_field_index_roundtrip() {
        for i in 0..6 {
            let field = InfoField::from_index(i).unwrap();
            assert_eq!(field.index(), i);
        }
        assert!(InfoField::from_index(6).is_none());
    }

    #[test]
    fn info_field_all_count() {
        assert_eq!(InfoField::ALL.len(), 6);
    }

    #[test]
    fn information_activity_new() {
        let activity = InformationActivity::new();
        assert_eq!(activity.selected_index, 0);
        assert_eq!(activity.scroll_offset, 0);
    }

    #[test]
    fn information_activity_with_device_status() {
        let custom_status = DeviceStatus {
            battery_percent: 42,
            is_charging: true,
            firmware_version: "2.0.0",
            storage_used_percent: 75,
        };

        let activity = InformationActivity::new().with_device_status(custom_status);
        assert_eq!(activity.device_status().battery_percent, 42);
        assert!(activity.device_status().is_charging);
        assert_eq!(activity.device_status().firmware_version, "2.0.0");
        assert_eq!(activity.device_status().storage_used_percent, 75);
    }

    #[test]
    fn information_activity_set_device_status() {
        let mut activity = InformationActivity::new();

        let new_status = DeviceStatus {
            battery_percent: 15,
            is_charging: true,
            firmware_version: "1.5.0",
            storage_used_percent: 90,
        };

        activity.set_device_status(new_status);
        assert_eq!(activity.device_status().battery_percent, 15);
        assert!(activity.device_status().is_charging);
        assert_eq!(activity.device_status().firmware_version, "1.5.0");
        assert_eq!(activity.device_status().storage_used_percent, 90);
    }

    #[test]
    fn information_activity_field_values() {
        let activity = InformationActivity::new();

        assert_eq!(activity.field_value(InfoField::DeviceName), "Xteink X4");
        assert_eq!(activity.field_value(InfoField::FirmwareVersion), "v1.0.0");
        assert_eq!(activity.field_value(InfoField::Battery), "85%");
        assert_eq!(activity.field_value(InfoField::Storage), "42% used");
        assert_eq!(
            activity.field_value(InfoField::Display),
            "480x800 @ 220 PPI"
        );
        assert_eq!(
            activity.field_value(InfoField::About),
            "Open Source E-Reader"
        );
    }

    #[test]
    fn information_activity_field_values_charging() {
        let status = DeviceStatus {
            battery_percent: 50,
            is_charging: true,
            firmware_version: "1.0.0",
            storage_used_percent: 42,
        };
        let activity = InformationActivity::new().with_device_status(status);

        assert_eq!(activity.field_value(InfoField::Battery), "50% (Charging)");
    }

    #[test]
    fn information_activity_lifecycle() {
        let mut activity = InformationActivity::new();

        // Set some state
        activity.selected_index = 3;
        activity.scroll_offset = 1;

        // On enter should reset
        activity.on_enter();
        assert_eq!(activity.selected_index, 0);
        assert_eq!(activity.scroll_offset, 0);
    }

    #[test]
    fn information_activity_input_navigation() {
        let mut activity = InformationActivity::new();
        activity.on_enter();

        // Initial state
        assert_eq!(activity.selected_index, 0);

        // Volume down should move to next
        let result = activity.handle_input(InputEvent::Press(Button::VolumeDown));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(activity.selected_index, 1);

        // Volume up should move to previous
        let result = activity.handle_input(InputEvent::Press(Button::VolumeUp));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(activity.selected_index, 0);

        // Right button should move to next
        let result = activity.handle_input(InputEvent::Press(Button::Right));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(activity.selected_index, 1);

        // Left button should move to previous
        let result = activity.handle_input(InputEvent::Press(Button::Left));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(activity.selected_index, 0);

        // Back should navigate back
        let result = activity.handle_input(InputEvent::Press(Button::Back));
        assert_eq!(result, ActivityResult::NavigateBack);
    }

    #[test]
    fn information_activity_selection_wraps() {
        let mut activity = InformationActivity::new();
        activity.on_enter();

        // Wrap backward from 0 to last
        let result = activity.handle_input(InputEvent::Press(Button::VolumeUp));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(activity.selected_index, InfoField::ALL.len() - 1);

        // Wrap forward from last to 0
        let result = activity.handle_input(InputEvent::Press(Button::VolumeDown));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(activity.selected_index, 0);
    }

    #[test]
    fn information_activity_selected_field() {
        let mut activity = InformationActivity::new();
        activity.on_enter();

        assert_eq!(activity.selected_field(), Some(InfoField::DeviceName));

        activity.handle_input(InputEvent::Press(Button::VolumeDown));
        assert_eq!(activity.selected_field(), Some(InfoField::FirmwareVersion));

        activity.handle_input(InputEvent::Press(Button::VolumeDown));
        assert_eq!(activity.selected_field(), Some(InfoField::Battery));
    }

    #[test]
    fn information_activity_unhandled_input() {
        let mut activity = InformationActivity::new();
        activity.on_enter();

        // Confirm should be ignored (read-only screen)
        let result = activity.handle_input(InputEvent::Press(Button::Confirm));
        assert_eq!(result, ActivityResult::Ignored);
    }

    #[test]
    fn information_activity_render() {
        let activity = InformationActivity::new();
        let mut display = TestDisplay::default_size();

        let result = activity.render(&mut display);
        assert!(result.is_ok());
    }

    #[test]
    fn information_activity_default_trait() {
        let activity: InformationActivity = Default::default();
        assert_eq!(activity.selected_index, 0);
    }
}
