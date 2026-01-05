//! Desktop SDL simulator for Xteink X4.

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
    let mut app = App::new();

    app.render(&mut display)?;
    window.update(&display);

    loop {
        for event in window.events().collect::<Vec<_>>() {
            match event {
                SimulatorEvent::Quit => return Ok(()),
                SimulatorEvent::KeyDown { keycode, .. } => {
                    if let Some(btn) = keycode_to_button(keycode) {
                        if app.handle_input(InputEvent::Press(btn)) {
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
        Keycode::Escape => Some(Button::Back),
        Keycode::P => Some(Button::Power),
        _ => None,
    }
}
