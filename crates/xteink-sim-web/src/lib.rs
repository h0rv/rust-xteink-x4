//! WASM browser simulator for Xteink X4.
//!
//! Uses the activity-based App for full UI navigation.

use embedded_graphics_web_simulator::{
    display::WebSimulatorDisplay, output_settings::OutputSettingsBuilder,
};
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use xteink_ui::{App, Button, InputEvent, MockFileSystem, DISPLAY_HEIGHT, DISPLAY_WIDTH};

use embedded_graphics::pixelcolor::BinaryColor;

struct State {
    app: App,
    fs: MockFileSystem,
    display: WebSimulatorDisplay<BinaryColor>,
}

impl State {
    fn new(display: WebSimulatorDisplay<BinaryColor>) -> Self {
        let app = App::new();
        let fs = MockFileSystem::new();

        let mut state = Self { app, fs, display };

        state.render();
        state
    }

    fn render(&mut self) {
        self.app.render(&mut self.display).unwrap();
        self.display.flush().unwrap();
    }

    fn on_key(&mut self, btn: Button) {
        let input = InputEvent::Press(btn);

        if self.app.handle_input(input) {
            self.render();

            if self.app.process_deferred_tasks(&mut self.fs) {
                self.render();
            }
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

    let state = Rc::new(RefCell::new(State::new(display)));

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

    // Log instructions to console
    web_sys::console::log_1(&"Xteink X4 Web Simulator".into());
    web_sys::console::log_1(&"Controls:".into());
    web_sys::console::log_1(&"  Arrow Keys / WASD - Navigate".into());
    web_sys::console::log_1(&"  Enter / Space     - Confirm / Select".into());
    web_sys::console::log_1(&"  Backspace         - Back".into());

    Ok(())
}

fn key_to_button(key: &str) -> Option<Button> {
    match key {
        "ArrowLeft" | "a" => Some(Button::Left),
        "ArrowRight" | "d" => Some(Button::Right),
        "ArrowUp" | "w" => Some(Button::Aux1),
        "ArrowDown" | "s" => Some(Button::Aux2),
        "Enter" | " " => Some(Button::Confirm),
        "Backspace" => Some(Button::Back),
        _ => None,
    }
}
