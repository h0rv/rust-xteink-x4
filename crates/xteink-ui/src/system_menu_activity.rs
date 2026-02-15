//! System Menu Activity for Xteink X4 e-reader.
//!
//! Main system menu with device-level options including library access,
//! settings, device information, and power controls.
//! Features large touch-friendly buttons optimized for e-ink displays.

extern crate alloc;

use alloc::format;

use embedded_graphics::{
    mono_font::{MonoTextStyle, MonoTextStyleBuilder},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    text::Text,
};

use crate::input::{Button, InputEvent};
use crate::ui::theme::{ui_font, ui_font_bold};
use crate::ui::{Activity, ActivityResult, Modal, Theme, ThemeMetrics};

/// Menu item types for the system menu
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuItem {
    /// Navigate to library/book browser
    Library,
    /// Navigate to raw filesystem browser
    Files,
    /// Reader settings (font, layout, etc.)
    ReaderSettings,
    /// Device settings (WiFi, storage, etc.)
    DeviceSettings,
    /// Device information (version, battery, etc.)
    Information,
    /// Put device to sleep
    Sleep,
    /// Power off the device
    PowerOff,
}

impl MenuItem {
    /// All menu items in order
    pub const ALL: [Self; 7] = [
        Self::Library,
        Self::Files,
        Self::ReaderSettings,
        Self::DeviceSettings,
        Self::Information,
        Self::Sleep,
        Self::PowerOff,
    ];

    /// Get display label for the menu item
    pub const fn label(self) -> &'static str {
        match self {
            Self::Library => "Library",
            Self::Files => "Files",
            Self::ReaderSettings => "Reader Settings",
            Self::DeviceSettings => "Device Settings",
            Self::Information => "Information",
            Self::Sleep => "Sleep",
            Self::PowerOff => "Power Off",
        }
    }

    /// Get icon character/ASCII art for the menu item
    /// Returns empty string for clean, modern text-only menu
    pub const fn icon(self) -> &'static str {
        ""
    }

    /// Get index in ALL array
    pub const fn index(self) -> usize {
        match self {
            Self::Library => 0,
            Self::Files => 1,
            Self::ReaderSettings => 2,
            Self::DeviceSettings => 3,
            Self::Information => 4,
            Self::Sleep => 5,
            Self::PowerOff => 6,
        }
    }

    /// Create from index
    pub const fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::Library),
            1 => Some(Self::Files),
            2 => Some(Self::ReaderSettings),
            3 => Some(Self::DeviceSettings),
            4 => Some(Self::Information),
            5 => Some(Self::Sleep),
            6 => Some(Self::PowerOff),
            _ => None,
        }
    }

    /// Check if this item requires confirmation
    pub const fn requires_confirmation(self) -> bool {
        matches!(self, Self::Sleep | Self::PowerOff)
    }

    /// Get confirmation message for this item
    pub const fn confirmation_message(self) -> &'static str {
        match self {
            Self::Sleep => "Put device to sleep?",
            Self::PowerOff => "Shut down device?",
            _ => "",
        }
    }
}

/// Device status information
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceStatus {
    /// Battery percentage (0-100)
    pub battery_percent: u8,
    /// Whether battery is charging
    pub is_charging: bool,
    /// Firmware version string
    pub firmware_version: &'static str,
    /// Storage used percentage (0-100)
    pub storage_used_percent: u8,
}

impl Default for DeviceStatus {
    fn default() -> Self {
        Self {
            battery_percent: 85,
            is_charging: false,
            firmware_version: "1.0.0",
            storage_used_percent: 42,
        }
    }
}

/// Navigation callbacks for activity results
#[derive(Debug, Clone, Copy)]
pub struct NavigationCallbacks {
    /// Called when user selects Library
    pub on_library: fn(),
    /// Called when user selects Reader Settings
    pub on_reader_settings: fn(),
    /// Called when user selects Files
    pub on_files: fn(),
    /// Called when user selects Device Settings
    pub on_device_settings: fn(),
    /// Called when user selects Information
    pub on_information: fn(),
    /// Called when user confirms Sleep
    pub on_sleep: fn(),
    /// Called when user confirms Power Off
    pub on_power_off: fn(),
}

impl Default for NavigationCallbacks {
    fn default() -> Self {
        Self {
            on_library: || {},
            on_files: || {},
            on_reader_settings: || {},
            on_device_settings: || {},
            on_information: || {},
            on_sleep: || {},
            on_power_off: || {},
        }
    }
}

/// Modal dialog types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModalType {
    None,
    ConfirmSleep,
    ConfirmPowerOff,
}

/// System Menu Activity implementing the Activity trait
#[derive(Debug, Clone)]
pub struct SystemMenuActivity {
    /// Currently selected menu index
    selected_index: usize,
    /// Device status information
    device_status: DeviceStatus,
    /// Navigation callbacks
    callbacks: NavigationCallbacks,
    /// Current modal dialog state
    modal_type: ModalType,
    /// Selected button within the modal (0 = Cancel, 1 = Confirm)
    modal_button: usize,
    /// Theme for styling
    theme: Theme,
}

impl SystemMenuActivity {
    /// Create a new system menu activity
    pub fn new() -> Self {
        Self {
            selected_index: 0,
            device_status: DeviceStatus::default(),
            callbacks: NavigationCallbacks::default(),
            modal_type: ModalType::None,
            modal_button: 0,
            theme: Theme::default(),
        }
    }

    /// Create with custom device status
    pub fn with_device_status(mut self, status: DeviceStatus) -> Self {
        self.device_status = status;
        self
    }

    /// Create with navigation callbacks
    pub fn with_callbacks(mut self, callbacks: NavigationCallbacks) -> Self {
        self.callbacks = callbacks;
        self
    }

    /// Get current selected menu item
    pub fn selected_item(&self) -> Option<MenuItem> {
        MenuItem::from_index(self.selected_index)
    }

    /// Get current device status
    pub fn device_status(&self) -> &DeviceStatus {
        &self.device_status
    }

    /// Update device status
    pub fn set_device_status(&mut self, status: DeviceStatus) {
        self.device_status = status;
    }

    /// Move selection down (wraps around)
    fn select_next(&mut self) {
        self.selected_index = (self.selected_index + 1) % MenuItem::ALL.len();
    }

    /// Move selection up (wraps around)
    fn select_prev(&mut self) {
        self.selected_index = if self.selected_index == 0 {
            MenuItem::ALL.len() - 1
        } else {
            self.selected_index - 1
        };
    }

    /// Handle selection of current menu item
    fn handle_select(&mut self) -> ActivityResult {
        if let Some(item) = self.selected_item() {
            if item.requires_confirmation() {
                self.show_confirmation_modal(item);
                ActivityResult::Consumed
            } else {
                self.execute_callback(item)
            }
        } else {
            ActivityResult::Ignored
        }
    }

    /// Show confirmation modal for destructive actions
    fn show_confirmation_modal(&mut self, item: MenuItem) {
        self.modal_type = match item {
            MenuItem::Sleep => ModalType::ConfirmSleep,
            MenuItem::PowerOff => ModalType::ConfirmPowerOff,
            _ => ModalType::None,
        };
        self.modal_button = 0; // Start on Cancel (safe default)
    }

    /// Close modal without action
    fn close_modal(&mut self) {
        self.modal_type = ModalType::None;
        self.modal_button = 0;
    }

    /// Execute the callback for a menu item
    fn execute_callback(&self, item: MenuItem) -> ActivityResult {
        match item {
            MenuItem::Library => {
                (self.callbacks.on_library)();
                ActivityResult::NavigateTo("library")
            }
            MenuItem::Files => {
                (self.callbacks.on_files)();
                ActivityResult::NavigateTo("files")
            }
            MenuItem::ReaderSettings => {
                (self.callbacks.on_reader_settings)();
                ActivityResult::NavigateTo("reader_settings")
            }
            MenuItem::DeviceSettings => {
                (self.callbacks.on_device_settings)();
                ActivityResult::NavigateTo("device_settings")
            }
            MenuItem::Information => {
                (self.callbacks.on_information)();
                ActivityResult::NavigateTo("information")
            }
            MenuItem::Sleep => {
                (self.callbacks.on_sleep)();
                ActivityResult::Consumed
            }
            MenuItem::PowerOff => {
                (self.callbacks.on_power_off)();
                ActivityResult::Consumed
            }
        }
    }

    /// Confirm the current modal action
    fn confirm_modal(&mut self) -> ActivityResult {
        let result = match self.modal_type {
            ModalType::ConfirmSleep => self.execute_callback(MenuItem::Sleep),
            ModalType::ConfirmPowerOff => self.execute_callback(MenuItem::PowerOff),
            ModalType::None => ActivityResult::Consumed,
        };
        self.close_modal();
        result
    }

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
                // Button 0 = Cancel, Button 1 = Confirm
                if self.modal_button == 1 {
                    self.confirm_modal()
                } else {
                    self.close_modal();
                    ActivityResult::Consumed
                }
            }
            InputEvent::Press(Button::Back) => {
                self.close_modal();
                ActivityResult::Consumed
            }
            _ => ActivityResult::Ignored,
        }
    }

    /// Render header bar with title and battery indicator
    fn render_header<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
        theme: &Theme,
    ) -> Result<(), D::Error> {
        use crate::ui::Header;

        let battery_text = if self.device_status.is_charging {
            format!("{}%+", self.device_status.battery_percent)
        } else {
            format!("{}%", self.device_status.battery_percent)
        };

        let header = Header::new("System Menu").with_right_text(battery_text);
        header.render(display, theme)
    }

    /// Render the menu list with large touch-friendly items
    fn render_menu_list<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
        theme: &Theme,
    ) -> Result<(), D::Error> {
        let display_width = display.bounding_box().size.width;
        let content_width = theme.metrics.content_width(display_width);
        let x = theme.metrics.side_padding as i32;
        let item_height = theme.metrics.list_item_height;
        let mut y = theme.metrics.header_height as i32 + theme.metrics.spacing as i32;

        for (i, item) in MenuItem::ALL.iter().enumerate() {
            let is_selected = i == self.selected_index;

            self.render_menu_item(
                display,
                theme,
                x,
                y,
                content_width,
                item_height,
                *item,
                is_selected,
            )?;

            y += item_height as i32;
        }

        Ok(())
    }

    /// Render a single menu item
    #[allow(clippy::too_many_arguments)]
    fn render_menu_item<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
        theme: &Theme,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        item: MenuItem,
        is_selected: bool,
    ) -> Result<(), D::Error> {
        let text_y = y + theme.metrics.item_text_y();

        // Clean background - only fill if selected
        if is_selected {
            Rectangle::new(Point::new(x, y), Size::new(width, height))
                .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
                .draw(display)?;
        }

        let text_color = if is_selected {
            BinaryColor::Off
        } else {
            BinaryColor::On
        };

        // Label (clean, text-only)
        let label_style = MonoTextStyle::new(ui_font(), text_color);
        Text::new(
            item.label(),
            Point::new(x + theme.metrics.side_padding as i32, text_y),
            label_style,
        )
        .draw(display)?;

        // Subtle bottom divider (except for selected)
        if !is_selected {
            Rectangle::new(Point::new(x, y + height as i32 - 1), Size::new(width, 1))
                .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
                .draw(display)?;
        }

        Ok(())
    }

    /// Render modal dialog if active
    fn render_modal<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
        theme: &Theme,
    ) -> Result<(), D::Error> {
        let (title, message) = match self.modal_type {
            ModalType::ConfirmSleep => ("Confirm Sleep", MenuItem::Sleep.confirmation_message()),
            ModalType::ConfirmPowerOff => (
                "Confirm Power Off",
                MenuItem::PowerOff.confirmation_message(),
            ),
            ModalType::None => return Ok(()),
        };

        let mut modal = Modal::new(title, message)
            .with_button("Cancel")
            .with_button("Confirm");
        modal.selected_button = self.modal_button;

        modal.render(display, theme)
    }
}

impl Activity for SystemMenuActivity {
    fn on_enter(&mut self) {
        self.selected_index = 0;
        self.modal_type = ModalType::None;
        self.modal_button = 0;
    }

    fn on_exit(&mut self) {
        self.modal_type = ModalType::None;
        self.modal_button = 0;
    }

    fn handle_input(&mut self, event: InputEvent) -> ActivityResult {
        // Handle modal input first if modal is shown
        if self.modal_type != ModalType::None {
            return self.handle_modal_input(event);
        }

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
            InputEvent::Press(Button::Confirm) => self.handle_select(),
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

        // Menu list
        self.render_menu_list(display, &self.theme)?;

        // Modal dialog if active
        self.render_modal(display, &self.theme)?;

        Ok(())
    }
}

impl Default for SystemMenuActivity {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_item_labels() {
        assert_eq!(MenuItem::Library.label(), "Library");
        assert_eq!(MenuItem::Files.label(), "Files");
        assert_eq!(MenuItem::ReaderSettings.label(), "Reader Settings");
        assert_eq!(MenuItem::DeviceSettings.label(), "Device Settings");
        assert_eq!(MenuItem::Information.label(), "Information");
        assert_eq!(MenuItem::Sleep.label(), "Sleep");
        assert_eq!(MenuItem::PowerOff.label(), "Power Off");
    }

    #[test]
    fn menu_item_icons() {
        assert_eq!(MenuItem::Library.icon(), "[===]");
        assert_eq!(MenuItem::Files.icon(), "[DIR]");
        assert_eq!(MenuItem::ReaderSettings.icon(), "{Aa}");
        assert_eq!(MenuItem::DeviceSettings.icon(), "[*]");
        assert_eq!(MenuItem::Information.icon(), "(i)");
        assert_eq!(MenuItem::Sleep.icon(), "[Zzz]");
        assert_eq!(MenuItem::PowerOff.icon(), "[O]");
    }

    #[test]
    fn menu_item_index_roundtrip() {
        for i in 0..7 {
            let item = MenuItem::from_index(i).unwrap();
            assert_eq!(item.index(), i);
        }
        assert!(MenuItem::from_index(7).is_none());
    }

    #[test]
    fn menu_item_all_count() {
        assert_eq!(MenuItem::ALL.len(), 7);
    }

    #[test]
    fn menu_item_requires_confirmation() {
        assert!(!MenuItem::Library.requires_confirmation());
        assert!(!MenuItem::Files.requires_confirmation());
        assert!(!MenuItem::ReaderSettings.requires_confirmation());
        assert!(!MenuItem::DeviceSettings.requires_confirmation());
        assert!(!MenuItem::Information.requires_confirmation());
        assert!(MenuItem::Sleep.requires_confirmation());
        assert!(MenuItem::PowerOff.requires_confirmation());
    }

    #[test]
    fn menu_item_confirmation_messages() {
        assert_eq!(
            MenuItem::Sleep.confirmation_message(),
            "Put device to sleep?"
        );
        assert_eq!(
            MenuItem::PowerOff.confirmation_message(),
            "Shut down device?"
        );
        assert_eq!(MenuItem::Library.confirmation_message(), "");
        assert_eq!(MenuItem::Files.confirmation_message(), "");
    }

    #[test]
    fn device_status_defaults() {
        let status = DeviceStatus::default();
        assert_eq!(status.battery_percent, 85);
        assert!(!status.is_charging);
        assert_eq!(status.firmware_version, "1.0.0");
        assert_eq!(status.storage_used_percent, 42);
    }

    #[test]
    fn navigation_callbacks_default() {
        let callbacks = NavigationCallbacks::default();
        // Should not panic when called
        (callbacks.on_library)();
        (callbacks.on_files)();
        (callbacks.on_reader_settings)();
        (callbacks.on_device_settings)();
        (callbacks.on_information)();
        (callbacks.on_sleep)();
        (callbacks.on_power_off)();
    }

    #[test]
    fn system_menu_activity_new() {
        let activity = SystemMenuActivity::new();
        assert_eq!(activity.selected_index, 0);
        assert_eq!(activity.modal_type, ModalType::None);
    }

    #[test]
    fn system_menu_activity_with_device_status() {
        let custom_status = DeviceStatus {
            battery_percent: 42,
            is_charging: true,
            firmware_version: "2.0.0",
            storage_used_percent: 75,
        };

        let activity = SystemMenuActivity::new().with_device_status(custom_status);
        assert_eq!(activity.device_status().battery_percent, 42);
        assert!(activity.device_status().is_charging);
        assert_eq!(activity.device_status().firmware_version, "2.0.0");
        assert_eq!(activity.device_status().storage_used_percent, 75);
    }

    #[test]
    fn system_menu_activity_with_callbacks() {
        let callbacks = NavigationCallbacks {
            on_library: || {},
            on_files: || {},
            on_reader_settings: || {},
            on_device_settings: || {},
            on_information: || {},
            on_sleep: || {},
            on_power_off: || {},
        };

        let activity = SystemMenuActivity::new().with_callbacks(callbacks);

        // Verify callbacks are stored and callable without panic
        (activity.callbacks.on_library)();
        (activity.callbacks.on_reader_settings)();
    }

    #[test]
    fn system_menu_activity_selection() {
        let mut activity = SystemMenuActivity::new();

        // Initial selection
        assert_eq!(activity.selected_item(), Some(MenuItem::Library));
        assert_eq!(activity.selected_index, 0);

        // Select next
        activity.select_next();
        assert_eq!(activity.selected_item(), Some(MenuItem::Files));
        assert_eq!(activity.selected_index, 1);

        // Select next again
        activity.select_next();
        assert_eq!(activity.selected_item(), Some(MenuItem::ReaderSettings));
        assert_eq!(activity.selected_index, 2);

        // Select previous
        activity.select_prev();
        assert_eq!(activity.selected_item(), Some(MenuItem::Files));
        assert_eq!(activity.selected_index, 1);

        // Wrap around backward
        activity.select_prev();
        activity.select_prev();
        assert_eq!(activity.selected_item(), Some(MenuItem::PowerOff));
        assert_eq!(activity.selected_index, 6);

        // Wrap around forward
        activity.select_next();
        assert_eq!(activity.selected_item(), Some(MenuItem::Library));
        assert_eq!(activity.selected_index, 0);
    }

    #[test]
    fn system_menu_activity_lifecycle() {
        let mut activity = SystemMenuActivity::new();

        // Set some state
        activity.selected_index = 3;
        activity.modal_type = ModalType::ConfirmSleep;

        // On enter should reset
        activity.on_enter();
        assert_eq!(activity.selected_index, 0);
        assert_eq!(activity.modal_type, ModalType::None);

        // Set modal and exit
        activity.modal_type = ModalType::ConfirmPowerOff;
        activity.on_exit();
        assert_eq!(activity.modal_type, ModalType::None);
    }

    #[test]
    fn system_menu_activity_input_navigation() {
        let mut activity = SystemMenuActivity::new();
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
    fn system_menu_activity_input_select() {
        let mut activity = SystemMenuActivity::new();
        activity.on_enter();

        // Select library should navigate
        let result = activity.handle_input(InputEvent::Press(Button::Confirm));
        assert_eq!(result, ActivityResult::NavigateTo("library"));

        // Move to files
        activity.select_next();
        let result = activity.handle_input(InputEvent::Press(Button::Confirm));
        assert_eq!(result, ActivityResult::NavigateTo("files"));

        // Move to reader settings
        activity.select_next();
        let result = activity.handle_input(InputEvent::Press(Button::Confirm));
        assert_eq!(result, ActivityResult::NavigateTo("reader_settings"));
    }

    #[test]
    fn system_menu_activity_modal_sleep() {
        let mut activity = SystemMenuActivity::new();
        activity.on_enter();

        // Move to sleep option
        for _ in 0..5 {
            activity.select_next();
        }
        assert_eq!(activity.selected_item(), Some(MenuItem::Sleep));

        // Selecting sleep should show modal
        let result = activity.handle_input(InputEvent::Press(Button::Confirm));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(activity.modal_type, ModalType::ConfirmSleep);
        assert_eq!(activity.modal_button, 0); // Starts on Cancel

        // Cancel with back
        let result = activity.handle_input(InputEvent::Press(Button::Back));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(activity.modal_type, ModalType::None);

        // Reopen modal
        let result = activity.handle_input(InputEvent::Press(Button::Confirm));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(activity.modal_type, ModalType::ConfirmSleep);

        // Navigate to Confirm button with Right, then press Confirm
        activity.handle_input(InputEvent::Press(Button::Right));
        assert_eq!(activity.modal_button, 1);
        let result = activity.handle_input(InputEvent::Press(Button::Confirm));
        assert_eq!(result, ActivityResult::Consumed); // Sleep callback fires
        assert_eq!(activity.modal_type, ModalType::None);
    }

    #[test]
    fn system_menu_activity_modal_button_navigation() {
        let mut activity = SystemMenuActivity::new();
        activity.on_enter();

        // Open sleep modal
        for _ in 0..5 {
            activity.select_next();
        }
        activity.handle_input(InputEvent::Press(Button::Confirm));
        assert_eq!(activity.modal_type, ModalType::ConfirmSleep);
        assert_eq!(activity.modal_button, 0);

        // Right cycles forward
        activity.handle_input(InputEvent::Press(Button::Right));
        assert_eq!(activity.modal_button, 1);

        // Right wraps around
        activity.handle_input(InputEvent::Press(Button::Right));
        assert_eq!(activity.modal_button, 0);

        // Left wraps backward
        activity.handle_input(InputEvent::Press(Button::Left));
        assert_eq!(activity.modal_button, 1);

        // VolumeUp goes backward
        activity.handle_input(InputEvent::Press(Button::VolumeUp));
        assert_eq!(activity.modal_button, 0);

        // VolumeDown goes forward
        activity.handle_input(InputEvent::Press(Button::VolumeDown));
        assert_eq!(activity.modal_button, 1);

        // Confirm on Cancel button (0) dismisses
        activity.modal_button = 0;
        let result = activity.handle_input(InputEvent::Press(Button::Confirm));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(activity.modal_type, ModalType::None);
    }

    #[test]
    fn system_menu_activity_modal_power_off() {
        let mut activity = SystemMenuActivity::new();
        activity.on_enter();

        // Move to power off option
        for _ in 0..6 {
            activity.select_next();
        }
        assert_eq!(activity.selected_item(), Some(MenuItem::PowerOff));

        // Selecting power off should show modal
        let result = activity.handle_input(InputEvent::Press(Button::Confirm));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(activity.modal_type, ModalType::ConfirmPowerOff);

        // Navigate to Confirm button and confirm
        activity.handle_input(InputEvent::Press(Button::Right));
        let result = activity.handle_input(InputEvent::Press(Button::Confirm));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(activity.modal_type, ModalType::None);
    }

    #[test]
    fn system_menu_activity_render() {
        let activity = SystemMenuActivity::new();
        let mut display = crate::test_display::TestDisplay::default_size();

        let result = activity.render(&mut display);
        assert!(result.is_ok());
    }

    #[test]
    fn system_menu_activity_render_with_modal() {
        let mut activity = SystemMenuActivity::new();
        activity.modal_type = ModalType::ConfirmSleep;

        let mut display = crate::test_display::TestDisplay::default_size();
        let result = activity.render(&mut display);
        assert!(result.is_ok());
    }

    #[test]
    fn system_menu_activity_set_device_status() {
        let mut activity = SystemMenuActivity::new();

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
    fn system_menu_activity_default_trait() {
        let activity: SystemMenuActivity = Default::default();
        assert_eq!(activity.selected_index, 0);
    }

    #[test]
    fn modal_type_enum_variants() {
        let types = [
            ModalType::None,
            ModalType::ConfirmSleep,
            ModalType::ConfirmPowerOff,
        ];

        assert_ne!(types[0], types[1]);
        assert_ne!(types[1], types[2]);
        assert_ne!(types[0], types[2]);
    }

    #[test]
    fn system_menu_activity_unhandled_input() {
        let mut activity = SystemMenuActivity::new();

        let result = activity.handle_input(InputEvent::Press(Button::Confirm));
        // Should navigate for Library (first item)
        assert_ne!(result, ActivityResult::Ignored);
    }
}
