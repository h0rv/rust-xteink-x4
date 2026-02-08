//! Main application state with activity-based navigation.
//!
//! Uses an enum-based activity dispatch with a navigation stack,
//! routing input events and rendering to the currently active activity.

extern crate alloc;

use alloc::vec::Vec;

use embedded_graphics::{pixelcolor::BinaryColor, prelude::*};

use crate::information_activity::InformationActivity;
use crate::input::InputEvent;
use crate::library_activity::LibraryActivity;
use crate::reader_settings_activity::ReaderSettingsActivity;
use crate::settings_activity::SettingsActivity;
use crate::system_menu_activity::{DeviceStatus, SystemMenuActivity};
use crate::ui::{Activity, ActivityResult};

/// Identifies which screen is currently active
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppScreen {
    /// Main system menu (root activity)
    SystemMenu,
    /// Library / book browser
    Library,
    /// Device settings (font size, font family)
    Settings,
    /// Reader-specific settings (margins, layout, etc.)
    ReaderSettings,
    /// Device information (firmware, battery, storage)
    Information,
}

/// Application state managing activity-based navigation.
///
/// Holds all activity instances and a navigation stack. The top of
/// the stack is the currently visible screen. Pressing Back pops
/// the stack, and NavigateTo pushes a new screen.
pub struct App {
    /// Navigation stack (bottom is always SystemMenu)
    nav_stack: Vec<AppScreen>,

    // Activity instances (owned, always alive)
    system_menu: SystemMenuActivity,
    library: LibraryActivity,
    settings: SettingsActivity,
    reader_settings: ReaderSettingsActivity,
    information: InformationActivity,
}

impl App {
    /// Create a new App with SystemMenu as the root screen.
    pub fn new() -> Self {
        Self {
            nav_stack: alloc::vec![AppScreen::SystemMenu],
            system_menu: SystemMenuActivity::new(),
            library: LibraryActivity::new(),
            settings: SettingsActivity::new(),
            reader_settings: ReaderSettingsActivity::new(),
            information: InformationActivity::new(),
        }
    }

    /// Get the currently active screen.
    pub fn current_screen(&self) -> AppScreen {
        self.nav_stack
            .last()
            .copied()
            .unwrap_or(AppScreen::SystemMenu)
    }

    /// Get the navigation stack depth.
    pub fn nav_depth(&self) -> usize {
        self.nav_stack.len()
    }

    /// Update device status on activities that display it.
    pub fn set_device_status(&mut self, status: DeviceStatus) {
        self.system_menu.set_device_status(status);
        self.information.set_device_status(status);
    }

    /// Handle input event. Returns true if a redraw is needed.
    pub fn handle_input(&mut self, event: InputEvent) -> bool {
        let result = match self.current_screen() {
            AppScreen::SystemMenu => self.system_menu.handle_input(event),
            AppScreen::Library => self.library.handle_input(event),
            AppScreen::Settings => self.settings.handle_input(event),
            AppScreen::ReaderSettings => self.reader_settings.handle_input(event),
            AppScreen::Information => self.information.handle_input(event),
        };

        self.process_result(result)
    }

    /// Render the currently active screen to the display.
    pub fn render<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
    ) -> Result<(), D::Error> {
        match self.current_screen() {
            AppScreen::SystemMenu => self.system_menu.render(display),
            AppScreen::Library => self.library.render(display),
            AppScreen::Settings => self.settings.render(display),
            AppScreen::ReaderSettings => self.reader_settings.render(display),
            AppScreen::Information => self.information.render(display),
        }
    }

    /// Process an ActivityResult, handling navigation.
    /// Returns true if the screen changed (redraw needed).
    fn process_result(&mut self, result: ActivityResult) -> bool {
        match result {
            ActivityResult::Consumed => true,
            ActivityResult::NavigateBack => self.navigate_back(),
            ActivityResult::NavigateTo(target) => self.navigate_to(target),
            ActivityResult::Ignored => false,
        }
    }

    /// Push a new screen onto the navigation stack.
    fn navigate_to(&mut self, target: &str) -> bool {
        let screen = match target {
            "library" => AppScreen::Library,
            "device_settings" => AppScreen::Settings,
            "reader_settings" => AppScreen::ReaderSettings,
            "information" => AppScreen::Information,
            _ => return false, // Unknown target
        };

        // Call on_exit for current activity
        self.call_on_exit(self.current_screen());

        self.nav_stack.push(screen);

        // Call on_enter for new activity
        self.call_on_enter(screen);

        true
    }

    /// Pop the current screen and return to the previous one.
    fn navigate_back(&mut self) -> bool {
        // Don't pop the root screen
        if self.nav_stack.len() <= 1 {
            return false;
        }

        let leaving = self.nav_stack.pop().unwrap();
        self.call_on_exit(leaving);

        // Re-enter the now-current screen
        self.call_on_enter(self.current_screen());

        true
    }

    /// Call on_enter on the activity for the given screen.
    fn call_on_enter(&mut self, screen: AppScreen) {
        match screen {
            AppScreen::SystemMenu => self.system_menu.on_enter(),
            AppScreen::Library => self.library.on_enter(),
            AppScreen::Settings => self.settings.on_enter(),
            AppScreen::ReaderSettings => self.reader_settings.on_enter(),
            AppScreen::Information => self.information.on_enter(),
        }
    }

    /// Call on_exit on the activity for the given screen.
    fn call_on_exit(&mut self, screen: AppScreen) {
        match screen {
            AppScreen::SystemMenu => self.system_menu.on_exit(),
            AppScreen::Library => self.library.on_exit(),
            AppScreen::Settings => self.settings.on_exit(),
            AppScreen::ReaderSettings => self.reader_settings.on_exit(),
            AppScreen::Information => self.information.on_exit(),
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::Button;

    #[test]
    fn app_starts_on_system_menu() {
        let app = App::new();
        assert_eq!(app.current_screen(), AppScreen::SystemMenu);
        assert_eq!(app.nav_depth(), 1);
    }

    #[test]
    fn app_navigate_to_library() {
        let mut app = App::new();

        // Confirm on first item (Library) should navigate
        let redraw = app.handle_input(InputEvent::Press(Button::Confirm));
        assert!(redraw);
        assert_eq!(app.current_screen(), AppScreen::Library);
        assert_eq!(app.nav_depth(), 2);
    }

    #[test]
    fn app_navigate_to_reader_settings() {
        let mut app = App::new();

        // Move down to Reader Settings (index 1)
        app.handle_input(InputEvent::Press(Button::VolumeDown));
        let redraw = app.handle_input(InputEvent::Press(Button::Confirm));
        assert!(redraw);
        assert_eq!(app.current_screen(), AppScreen::ReaderSettings);
    }

    #[test]
    fn app_navigate_to_device_settings() {
        let mut app = App::new();

        // Move down to Device Settings (index 2)
        app.handle_input(InputEvent::Press(Button::VolumeDown));
        app.handle_input(InputEvent::Press(Button::VolumeDown));
        let redraw = app.handle_input(InputEvent::Press(Button::Confirm));
        assert!(redraw);
        assert_eq!(app.current_screen(), AppScreen::Settings);
    }

    #[test]
    fn app_navigate_to_information() {
        let mut app = App::new();

        // Move down to Information (index 3)
        for _ in 0..3 {
            app.handle_input(InputEvent::Press(Button::VolumeDown));
        }
        let redraw = app.handle_input(InputEvent::Press(Button::Confirm));
        assert!(redraw);
        assert_eq!(app.current_screen(), AppScreen::Information);
    }

    #[test]
    fn app_navigate_back_to_system_menu() {
        let mut app = App::new();

        // Navigate to Library
        app.handle_input(InputEvent::Press(Button::Confirm));
        assert_eq!(app.current_screen(), AppScreen::Library);

        // Navigate back
        let redraw = app.handle_input(InputEvent::Press(Button::Back));
        assert!(redraw);
        assert_eq!(app.current_screen(), AppScreen::SystemMenu);
        assert_eq!(app.nav_depth(), 1);
    }

    #[test]
    fn app_cannot_pop_root() {
        let mut app = App::new();

        // Back on root should navigate back (handled by SystemMenuActivity)
        // SystemMenuActivity returns NavigateBack on Back button
        let redraw = app.handle_input(InputEvent::Press(Button::Back));
        // Can't pop root, so no redraw
        assert!(!redraw);
        assert_eq!(app.current_screen(), AppScreen::SystemMenu);
        assert_eq!(app.nav_depth(), 1);
    }

    #[test]
    fn app_render_does_not_panic() {
        let app = App::new();
        let mut display = crate::test_display::TestDisplay::default_size();
        let result = app.render(&mut display);
        assert!(result.is_ok());
    }

    #[test]
    fn app_set_device_status() {
        let mut app = App::new();
        let status = DeviceStatus {
            battery_percent: 42,
            is_charging: true,
            firmware_version: "2.0.0",
            storage_used_percent: 75,
        };
        app.set_device_status(status);

        // Verify status propagated
        assert_eq!(app.system_menu.device_status().battery_percent, 42);
        assert_eq!(app.information.device_status().battery_percent, 42);
    }

    #[test]
    fn app_deep_navigation_and_back() {
        let mut app = App::new();

        // SystemMenu -> Library
        app.handle_input(InputEvent::Press(Button::Confirm));
        assert_eq!(app.current_screen(), AppScreen::Library);
        assert_eq!(app.nav_depth(), 2);

        // Library -> Back -> SystemMenu
        app.handle_input(InputEvent::Press(Button::Back));
        assert_eq!(app.current_screen(), AppScreen::SystemMenu);
        assert_eq!(app.nav_depth(), 1);
    }

    #[test]
    fn app_default_trait() {
        let app: App = Default::default();
        assert_eq!(app.current_screen(), AppScreen::SystemMenu);
    }
}
