//! Desktop SDL simulator for Xteink X4.
//!
//! Uses the activity-based App for full UI navigation.

use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics_simulator::{
    sdl2::Keycode, OutputSettingsBuilder, SimulatorDisplay, SimulatorEvent, Window,
};
use xteink_ui::{App, Button, InputEvent, DISPLAY_HEIGHT, DISPLAY_WIDTH};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_settings = OutputSettingsBuilder::new().scale(1).build();
    let mut display: SimulatorDisplay<BinaryColor> =
        SimulatorDisplay::new(Size::new(DISPLAY_WIDTH, DISPLAY_HEIGHT));
    let mut window = Window::new("Xteink X4", &output_settings);

    // Create activity-based app
    let mut app = App::new();

    // Initial render
    app.render(&mut display)?;
    window.update(&display);

    println!("Xteink X4 Simulator");
    println!("Controls:");
    println!("  Arrow Keys / WASD - Navigate");
    println!("  Enter / Space     - Confirm / Select");
    println!("  Backspace         - Back");
    println!("  Escape            - Quit");

    loop {
        let events = window.events().collect::<Vec<_>>();

        for event in events {
            match event {
                SimulatorEvent::Quit => {
                    return Ok(());
                }
                SimulatorEvent::KeyDown { keycode, .. } => {
                    if let Some(btn) = keycode_to_button(keycode) {
                        let input = InputEvent::Press(btn);

                        if app.handle_input(input) {
                            app.render(&mut display)?;
                            window.update(&display);
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

fn keycode_to_button(keycode: Keycode) -> Option<Button> {
    match keycode {
        Keycode::Left | Keycode::A => Some(Button::Left),
        Keycode::Right | Keycode::D => Some(Button::Right),
        Keycode::Up | Keycode::W => Some(Button::VolumeUp),
        Keycode::Down | Keycode::S => Some(Button::VolumeDown),
        Keycode::Return | Keycode::Space => Some(Button::Confirm),
        Keycode::Backspace => Some(Button::Back),
        _ => None,
    }
}
