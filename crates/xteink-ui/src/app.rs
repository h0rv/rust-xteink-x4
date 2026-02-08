//! Main application state with activity-based navigation.
//!
//! Uses an enum-based activity dispatch with a navigation stack,
//! routing input events and rendering to the currently active activity.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use embedded_graphics::{pixelcolor::BinaryColor, prelude::*};

use crate::information_activity::InformationActivity;
use crate::input::InputEvent;
use crate::library_activity::LibraryActivity;
use crate::reader_settings_activity::ReaderSettingsActivity;
use crate::settings_activity::SettingsActivity;
use crate::system_menu_activity::{DeviceStatus, SystemMenuActivity};
use crate::ui::{Activity, ActivityRefreshMode, ActivityResult};
use crate::{BookInfo, FileSystem};

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

    /// Per-activity page counters since last non-fast cleanup refresh.
    refresh_counters: [u32; Self::SCREEN_COUNT],
    /// Full refresh frequency setting (pages between full refreshes)
    refresh_frequency_pages: u32,
    /// Force a full refresh on the next redraw (used on activity enter)
    needs_full_refresh_on_next_draw: bool,
    /// Cached filesystem-discovered books for the library.
    library_cache: Option<Vec<BookInfo>>,
    /// Root directory used for library discovery.
    library_root: String,
    /// Set when we should run a scan on the next task tick.
    library_scan_pending: bool,
    /// Set when cache should be ignored and rebuilt.
    library_cache_invalidated: bool,
}

impl App {
    const SCREEN_COUNT: usize = 5;
    /// Default number of pages between full refreshes
    const DEFAULT_REFRESH_FREQUENCY: u32 = 10;

    /// Create a new App with SystemMenu as the root screen.
    pub fn new() -> Self {
        Self {
            nav_stack: alloc::vec![AppScreen::SystemMenu],
            system_menu: SystemMenuActivity::new(),
            library: LibraryActivity::new(),
            settings: SettingsActivity::new(),
            reader_settings: ReaderSettingsActivity::new(),
            information: InformationActivity::new(),
            refresh_counters: [0; Self::SCREEN_COUNT],
            refresh_frequency_pages: Self::DEFAULT_REFRESH_FREQUENCY,
            needs_full_refresh_on_next_draw: true,
            library_cache: None,
            library_root: String::from(LibraryActivity::DEFAULT_BOOKS_ROOT),
            library_scan_pending: false,
            library_cache_invalidated: true,
        }
    }

    fn screen_index(screen: AppScreen) -> usize {
        match screen {
            AppScreen::SystemMenu => 0,
            AppScreen::Library => 1,
            AppScreen::Settings => 2,
            AppScreen::ReaderSettings => 3,
            AppScreen::Information => 4,
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

    /// Get the refresh mode for the current activity.
    ///
    /// Implements a counter-based strategy:
    /// - Returns Full refresh when entering a new activity or on manual trigger
    /// - Returns Partial refresh after N fast updates (for ghost cleanup)
    /// - Returns Fast refresh for most interactions
    pub fn get_refresh_mode(&mut self) -> ActivityRefreshMode {
        self.refresh_frequency_pages =
            self.reader_settings.settings().refresh_frequency.pages() as u32;
        let screen = self.current_screen();
        let screen_index = Self::screen_index(screen);

        if self.needs_full_refresh_on_next_draw {
            self.needs_full_refresh_on_next_draw = false;
            self.refresh_counters[screen_index] = 0;
            self.mark_refresh_complete(screen);
            return ActivityRefreshMode::Full;
        }

        // Check if activity explicitly requests a specific mode
        let activity_mode = match screen {
            AppScreen::SystemMenu => self.system_menu.refresh_mode(),
            AppScreen::Library => self.library.refresh_mode(),
            AppScreen::Settings => self.settings.refresh_mode(),
            AppScreen::ReaderSettings => self.reader_settings.refresh_mode(),
            AppScreen::Information => self.information.refresh_mode(),
        };

        // If activity requests Full, use Full and mark as consumed
        if activity_mode == ActivityRefreshMode::Full {
            self.refresh_counters[screen_index] = 0;
            self.mark_refresh_complete(screen);
            return ActivityRefreshMode::Full;
        }

        // Increment counter and check if we need a partial refresh
        self.refresh_counters[screen_index] += 1;

        if self.refresh_counters[screen_index] >= self.refresh_frequency_pages {
            // Time for a ghost-cleanup partial refresh
            self.refresh_counters[screen_index] = 0;
            ActivityRefreshMode::Partial
        } else {
            // Use the activity's preference (usually Fast)
            activity_mode
        }
    }

    fn mark_refresh_complete(&mut self, screen: AppScreen) {
        if screen == AppScreen::Library {
            self.library.mark_refresh_complete();
        }
    }

    /// Reset the refresh counter for an activity and force full refresh on next draw.
    pub fn reset_refresh_counter_for(&mut self, screen: AppScreen) {
        let index = Self::screen_index(screen);
        self.refresh_counters[index] = 0;
        self.needs_full_refresh_on_next_draw = true;
    }

    /// Get the current refresh frequency setting (pages between partial refreshes)
    pub fn refresh_frequency_pages(&self) -> u32 {
        self.refresh_frequency_pages
    }

    /// Set the refresh frequency (pages between partial refreshes)
    pub fn set_refresh_frequency_pages(&mut self, pages: u32) {
        // Clamp to reasonable values (1-50)
        self.refresh_frequency_pages = pages.clamp(1, 50);
    }

    /// Set the root directory used for library scanning.
    pub fn set_library_root(&mut self, root: impl Into<String>) {
        self.library_root = root.into();
        self.invalidate_library_cache();
    }

    /// Invalidate library cache and schedule a scan on next library entry.
    pub fn invalidate_library_cache(&mut self) {
        self.library_cache_invalidated = true;
        self.library_scan_pending = true;

        if self.current_screen() == AppScreen::Library {
            self.library.begin_loading_scan();
        }
    }

    /// Run deferred library scan work.
    ///
    /// Returns `true` when UI changed and should be redrawn.
    pub fn process_library_scan(&mut self, fs: &mut dyn FileSystem) -> bool {
        if self.current_screen() != AppScreen::Library || !self.library_scan_pending {
            return false;
        }

        let books = LibraryActivity::discover_books(fs, &self.library_root);
        self.library_cache = Some(books.clone());
        self.library_cache_invalidated = false;
        self.library_scan_pending = false;
        self.library.set_books(books);
        self.library.finish_loading_scan();
        true
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

        let redraw = self.process_result(result);
        self.capture_library_refresh_request();
        redraw
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

        // Reset refresh counter when entering a new activity
        self.reset_refresh_counter_for(screen);

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

        self.reset_refresh_counter_for(self.current_screen());
        // Re-enter the now-current screen
        self.call_on_enter(self.current_screen());

        true
    }

    /// Call on_enter on the activity for the given screen.
    fn call_on_enter(&mut self, screen: AppScreen) {
        match screen {
            AppScreen::SystemMenu => self.system_menu.on_enter(),
            AppScreen::Library => {
                self.library.on_enter();

                if self.library_cache_invalidated || self.library_cache.is_none() {
                    self.library.begin_loading_scan();
                    self.library_scan_pending = true;
                } else if let Some(cached_books) = self.library_cache.as_ref() {
                    self.library.set_books(cached_books.clone());
                    self.library.finish_loading_scan();
                    self.library_scan_pending = false;
                }
            }
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

    fn capture_library_refresh_request(&mut self) {
        if self.current_screen() == AppScreen::Library && self.library.take_refresh_request() {
            self.invalidate_library_cache();
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
    #[cfg(feature = "std")]
    use crate::MockFileSystem;

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

    #[test]
    fn app_forces_full_refresh_on_activity_enter() {
        let mut app = App::new();

        assert_eq!(app.get_refresh_mode(), ActivityRefreshMode::Full);
        assert_eq!(app.get_refresh_mode(), ActivityRefreshMode::Fast);

        app.handle_input(InputEvent::Press(Button::Confirm));
        assert_eq!(app.current_screen(), AppScreen::Library);
        assert_eq!(app.get_refresh_mode(), ActivityRefreshMode::Full);
    }

    #[test]
    fn app_uses_per_activity_refresh_counters() {
        let mut app = App::new();

        // Consume initial full refresh.
        assert_eq!(app.get_refresh_mode(), ActivityRefreshMode::Full);

        // Use a few fast updates on SystemMenu only.
        assert_eq!(app.get_refresh_mode(), ActivityRefreshMode::Fast);
        assert_eq!(app.get_refresh_mode(), ActivityRefreshMode::Fast);
        assert_eq!(app.get_refresh_mode(), ActivityRefreshMode::Fast);

        // Enter and leave Library: each enter should force full refresh.
        app.handle_input(InputEvent::Press(Button::Confirm));
        assert_eq!(app.current_screen(), AppScreen::Library);
        assert_eq!(app.get_refresh_mode(), ActivityRefreshMode::Full);
        app.handle_input(InputEvent::Press(Button::Back));
        assert_eq!(app.current_screen(), AppScreen::SystemMenu);
        assert_eq!(app.get_refresh_mode(), ActivityRefreshMode::Full);

        // SystemMenu counter restarts after re-enter and still reaches periodic partial.
        for _ in 0..9 {
            assert_eq!(app.get_refresh_mode(), ActivityRefreshMode::Fast);
        }
        assert_eq!(app.get_refresh_mode(), ActivityRefreshMode::Partial);
    }

    #[cfg(feature = "std")]
    #[test]
    fn library_scan_uses_cache_until_invalidated() {
        let mut app = App::new();
        let mut fs = MockFileSystem::new();

        app.handle_input(InputEvent::Press(Button::Confirm));
        assert!(app.process_library_scan(&mut fs));
        assert!(!app.process_library_scan(&mut fs));

        app.handle_input(InputEvent::Press(Button::Back));
        app.handle_input(InputEvent::Press(Button::Confirm));
        assert!(!app.process_library_scan(&mut fs));

        app.invalidate_library_cache();
        assert!(app.process_library_scan(&mut fs));
    }
}
