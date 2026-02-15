//! Internal helpers shared across UI activities.

use crate::input::{Button, InputEvent};

/// Outcome of handling a two-button modal input event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TwoButtonModalInputResult {
    /// Selection moved between buttons.
    Consumed,
    /// Confirm/primary action selected.
    Confirmed,
    /// Cancel/back action selected.
    Cancelled,
    /// Event not handled by modal navigation.
    Ignored,
}

/// Convert an index into a value from a static enum variant table.
pub(crate) const fn enum_from_index<T: Copy, const N: usize>(
    all: &[T; N],
    index: usize,
) -> Option<T> {
    if index < N {
        Some(all[index])
    } else {
        None
    }
}

/// Handle standard two-button modal navigation and actions.
///
/// Button mapping:
/// - Left / VolumeUp: select previous button (wrap)
/// - Right / VolumeDown: select next button (wrap)
/// - Confirm: returns `Confirmed` when index is 1, otherwise `Cancelled`
/// - Back: returns `Cancelled`
pub(crate) fn handle_two_button_modal_input(
    event: InputEvent,
    selected_button: &mut usize,
) -> TwoButtonModalInputResult {
    match event {
        InputEvent::Press(Button::Left) | InputEvent::Press(Button::VolumeUp) => {
            if *selected_button > 0 {
                *selected_button -= 1;
            } else {
                *selected_button = 1;
            }
            TwoButtonModalInputResult::Consumed
        }
        InputEvent::Press(Button::Right) | InputEvent::Press(Button::VolumeDown) => {
            *selected_button = (*selected_button + 1) % 2;
            TwoButtonModalInputResult::Consumed
        }
        InputEvent::Press(Button::Confirm) => {
            if *selected_button == 1 {
                TwoButtonModalInputResult::Confirmed
            } else {
                TwoButtonModalInputResult::Cancelled
            }
        }
        InputEvent::Press(Button::Back) => TwoButtonModalInputResult::Cancelled,
        _ => TwoButtonModalInputResult::Ignored,
    }
}
