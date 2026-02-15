//! Activity pattern for screen-based navigation.
//!
//! The Activity pattern provides a lifecycle for UI screens,
//! similar to Android Activities but simplified for e-ink devices.

use embedded_graphics::{pixelcolor::BinaryColor, prelude::*};

use crate::app::AppScreen;
use crate::input::InputEvent;

/// Result of handling an input event
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityResult {
    /// Event consumed, stay on current activity
    Consumed,
    /// Event consumed, request navigation back
    NavigateBack,
    /// Event consumed, request navigation to new activity
    NavigateTo(AppScreen),
    /// Event not handled, propagate to parent
    Ignored,
}

/// Activity trait for screen-based UI architecture.
///
/// Each screen in the application implements this trait to handle
/// its own lifecycle, input processing, and rendering.
///
/// # Example
/// ```
/// use xteink_ui::ui::{Activity, ActivityResult};
/// use xteink_ui::input::InputEvent;
/// use embedded_graphics::prelude::*;
/// use embedded_graphics::pixelcolor::BinaryColor;
///
/// struct MainMenu {
///     selected_index: usize,
/// }
///
/// impl Activity for MainMenu {
///     fn on_enter(&mut self) {
///         // Initialize state when entering screen
///     }
///     
///     fn on_exit(&mut self) {
///         // Cleanup when leaving screen
///     }
///     
///     fn handle_input(&mut self, event: InputEvent) -> ActivityResult {
///         match event {
///             InputEvent::Press(xteink_ui::input::Button::Back) => ActivityResult::NavigateBack,
///             _ => ActivityResult::Consumed,
///         }
///     }
///     
///     fn render<D: DrawTarget<Color = BinaryColor>>(
///         &self,
///         display: &mut D,
///     ) -> Result<(), D::Error> {
///         // Render UI to display
///         Ok(())
///     }
/// }
/// ```
/// Display refresh mode preference for an activity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActivityRefreshMode {
    /// Use fast refresh mode (default for most interactions)
    #[default]
    Fast,
    /// Use partial refresh mode (for periodic ghost cleanup)
    Partial,
    /// Use full refresh mode (on activity enter or manual trigger)
    Full,
}

pub trait Activity {
    /// Called when the activity becomes visible
    fn on_enter(&mut self);

    /// Called when the activity is being replaced
    fn on_exit(&mut self);

    /// Handle input event.
    ///
    /// Returns an ActivityResult indicating how the event was handled
    /// and what navigation action (if any) should occur.
    fn handle_input(&mut self, event: InputEvent) -> ActivityResult;

    /// Render the activity to the display.
    ///
    /// The display uses BinaryColor (black/white) for e-ink optimization.
    fn render<D: DrawTarget<Color = BinaryColor>>(&self, display: &mut D) -> Result<(), D::Error>;

    /// Get the preferred refresh mode for this activity.
    ///
    /// Called by the firmware after rendering to determine which
    /// e-ink refresh mode to use. Activities can override this
    /// to request full or partial refreshes when needed.
    fn refresh_mode(&self) -> ActivityRefreshMode {
        ActivityRefreshMode::default()
    }
}
