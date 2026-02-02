//! WASM browser simulator for Xteink X4.
//!
//! Demonstrates file browser and text viewer with mock filesystem.

use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics_web_simulator::{
    display::WebSimulatorDisplay, output_settings::OutputSettingsBuilder,
};
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use xteink_ui::filesystem::FileSystem;
use xteink_ui::{
    Button, EpubRenderer, FileBrowser, InputEvent, MockFileSystem, TextViewer, DISPLAY_HEIGHT,
    DISPLAY_WIDTH,
};

#[derive(Debug, Clone, Copy, PartialEq)]
enum AppMode {
    Library, // File browser
    Reader,  // Text viewer
    Epub,
}

struct State {
    mode: AppMode,
    browser: FileBrowser,
    viewer: Option<TextViewer>,
    epub_renderer: Option<EpubRenderer>,
    current_file: String,
    fs: MockFileSystem,
    display: WebSimulatorDisplay<BinaryColor>,
}

impl State {
    fn new(display: WebSimulatorDisplay<BinaryColor>) -> Self {
        let mut fs = MockFileSystem::new();
        let mut browser = FileBrowser::new("/books");

        // Load initial directory
        if let Err(e) = browser.load(&mut fs) {
            web_sys::console::log_1(&format!("Error loading filesystem: {:?}", e).into());
        }

        let mut state = Self {
            mode: AppMode::Library,
            browser,
            viewer: None,
            epub_renderer: None,
            current_file: String::new(),
            fs,
            display,
        };

        state.render();
        state
    }

    fn render(&mut self) {
        match self.mode {
            AppMode::Library => {
                self.browser.render(&mut self.display).unwrap();
            }
            AppMode::Reader => {
                if let Some(ref viewer) = self.viewer {
                    viewer
                        .render(&mut self.display, &self.current_file)
                        .unwrap();
                }
            }
            AppMode::Epub => {
                if let Some(ref mut renderer) = self.epub_renderer {
                    renderer.render(&mut self.display).unwrap();
                }
            }
        }
        self.display.flush().unwrap();
    }

    fn on_key(&mut self, btn: Button) {
        if btn == Button::Power {
            // Toggle between library and reader modes
            self.mode = match self.mode {
                AppMode::Library => {
                    if self.viewer.is_some() {
                        AppMode::Reader
                    } else {
                        AppMode::Library
                    }
                }
                AppMode::Reader => AppMode::Library,
                AppMode::Epub => AppMode::Library,
            };
            self.render();
            return;
        }

        match self.mode {
            AppMode::Library => {
                let input = InputEvent::Press(btn);
                let (needs_redraw, selected) = self.browser.handle_input(input);

                if let Some(path) = selected {
                    if path.is_empty() {
                        // Reload (navigated to different directory)
                        if let Err(e) = self.browser.load(&mut self.fs) {
                            web_sys::console::log_1(&format!("Error reloading: {:?}", e).into());
                        }
                        self.render();
                    } else {
                        // Open file
                        web_sys::console::log_1(&format!("Opening: {}", path).into());
                        if path.to_lowercase().ends_with(".epub") {
                            let mut renderer = EpubRenderer::new();
                            if renderer.load(&path).is_ok() {
                                self.epub_renderer = Some(renderer);
                                self.mode = AppMode::Epub;
                                self.render();
                                return;
                            }
                        }
                        match self.fs.read_file(&path) {
                            Ok(content) => {
                                self.current_file = path.clone();
                                self.viewer = Some(TextViewer::new(content));
                                self.mode = AppMode::Reader;
                                self.render();
                            }
                            Err(e) => {
                                web_sys::console::log_1(
                                    &format!("Error reading file: {:?}", e).into(),
                                );
                            }
                        }
                    }
                } else if needs_redraw {
                    self.render();
                }
            }
            AppMode::Reader => {
                if let Some(ref mut viewer) = self.viewer {
                    let input = InputEvent::Press(btn);
                    if viewer.handle_input(input) {
                        self.render();
                    } else if btn == Button::Back {
                        // Go back to library
                        self.mode = AppMode::Library;
                        self.render();
                    }
                }
            }
            AppMode::Epub => {
                if let Some(ref mut renderer) = self.epub_renderer {
                    let mut changed = false;
                    match btn {
                        Button::Left | Button::VolumeUp => {
                            changed = renderer.prev_page();
                        }
                        Button::Right | Button::VolumeDown => {
                            changed = renderer.next_page();
                        }
                        Button::Back => {
                            self.mode = AppMode::Library;
                            self.render();
                            return;
                        }
                        _ => {}
                    }
                    if changed {
                        self.render();
                    }
                }
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
    web_sys::console::log_1(&"Xteink X4 Web Simulator - File Browser".into());
    web_sys::console::log_1(&"Controls:".into());
    web_sys::console::log_1(&"  Arrow Keys / WASD - Navigate".into());
    web_sys::console::log_1(&"  Enter / Space - Open file".into());
    web_sys::console::log_1(&"  Escape - Back".into());
    web_sys::console::log_1(&"  P - Toggle Library/Reader mode".into());

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
