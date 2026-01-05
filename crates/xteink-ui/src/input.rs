//! Button input abstraction.

/// Device buttons
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Left,
    Right,
    Up,
    Down,
    Confirm,
    Back,
}

/// Input events
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputEvent {
    Press(Button),
}
