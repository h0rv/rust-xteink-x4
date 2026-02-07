//! Button input abstraction.

/// Physical device buttons (directly maps to hardware)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    // GPIO1 ADC (resistor ladder)
    Left,
    Right,
    Up,
    Down,
    Confirm,
    Back,
    // GPIO2 ADC (resistor ladder)
    VolumeUp,
    VolumeDown,
    // GPIO3 (digital, active LOW)
    Power,
}

/// Input events
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputEvent {
    Press(Button),
}
