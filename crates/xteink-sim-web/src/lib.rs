//! WASM browser simulator for Xteink X4.

use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics_web_simulator::{
    display::WebSimulatorDisplay, output_settings::OutputSettingsBuilder,
};
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use xteink_ui::{App, Button, InputEvent, DISPLAY_HEIGHT, DISPLAY_WIDTH};

struct State {
    app: App,
    display: WebSimulatorDisplay<BinaryColor>,
}

impl State {
    fn render(&mut self) {
        self.app.render(&mut self.display).unwrap();
        self.display.flush().unwrap();
    }

    fn on_key(&mut self, btn: Button) {
        if self.app.handle_input(InputEvent::Press(btn)) {
            self.render();
        }
    }
}

#[wasm_bindgen(start)]
pub fn main() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();

    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();
    let container = document.get_element_by_id("display-container").unwrap();

    let output_settings = OutputSettingsBuilder::new().scale(1).build();
    let display = WebSimulatorDisplay::new(
        (DISPLAY_WIDTH, DISPLAY_HEIGHT),
        &output_settings,
        Some(&container),
    );

    let state = Rc::new(RefCell::new(State {
        app: App::new(),
        display,
    }));
    state.borrow_mut().render();

    // Keyboard handler
    let state_clone = state.clone();
    let closure = Closure::wrap(Box::new(move |e: web_sys::KeyboardEvent| {
        if let Some(btn) = key_to_button(&e.key()) {
            e.prevent_default();
            state_clone.borrow_mut().on_key(btn);
        }
    }) as Box<dyn FnMut(_)>);

    window.add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref())?;
    closure.forget();

    Ok(())
}

fn key_to_button(key: &str) -> Option<Button> {
    match key {
        "ArrowLeft" | "a" => Some(Button::Left),
        "ArrowRight" | "d" => Some(Button::Right),
        "ArrowUp" | "w" => Some(Button::VolumeUp),
        "ArrowDown" | "s" => Some(Button::VolumeDown),
        "Enter" | " " => Some(Button::Confirm),
        "Escape" => Some(Button::Back),
        "p" => Some(Button::Power),
        _ => None,
    }
}
