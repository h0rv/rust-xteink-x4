//! Desktop SDL simulator for Xteink X4.
//!
//! Demonstrates file browser and text viewer with mock filesystem.

use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics_simulator::{
    sdl2::Keycode, OutputSettingsBuilder, SimulatorDisplay, SimulatorEvent, Window,
};
use xteink_ui::filesystem::FileSystem;
use xteink_ui::{
    Button, FileBrowser, InputEvent, MockFileSystem, TextViewer, DISPLAY_HEIGHT, DISPLAY_WIDTH,
};

#[derive(Debug, Clone, Copy, PartialEq)]
enum AppMode {
    Library, // File browser
    Reader,  // Text viewer
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_settings = OutputSettingsBuilder::new().scale(1).build();
    let mut display: SimulatorDisplay<BinaryColor> =
        SimulatorDisplay::new(Size::new(DISPLAY_WIDTH, DISPLAY_HEIGHT));
    let mut window = Window::new("Xteink X4 - File Browser Demo", &output_settings);

    // Initialize mock filesystem with sample files
    let mut fs = MockFileSystem::new();

    // File browser
    let mut browser = FileBrowser::new("/books");
    browser.load(&mut fs)?;

    // Text viewer (starts empty)
    let mut viewer: Option<TextViewer> = None;
    let mut current_file: String = String::new();

    let mut mode = AppMode::Library;

    // Initial render
    browser.render(&mut display)?;
    window.update(&display);

    println!("Xteink X4 Simulator - File Browser");
    println!("Controls:");
    println!("  Arrow Keys / WASD - Navigate");
    println!("  Enter / Space - Open file");
    println!("  Escape - Back");
    println!("  P - Power (toggle mode)");

    loop {
        for event in window.events().collect::<Vec<_>>() {
            match event {
                SimulatorEvent::Quit => return Ok(()),
                SimulatorEvent::KeyDown { keycode, .. } => {
                    if keycode == Keycode::P {
                        // Toggle between library and reader modes
                        mode = match mode {
                            AppMode::Library => {
                                if viewer.is_some() {
                                    AppMode::Reader
                                } else {
                                    AppMode::Library
                                }
                            }
                            AppMode::Reader => AppMode::Library,
                        };

                        // Redraw
                        match mode {
                            AppMode::Library => browser.render(&mut display)?,
                            AppMode::Reader => {
                                if let Some(ref v) = viewer {
                                    v.render(&mut display, &current_file)?;
                                }
                            }
                        }
                        window.update(&display);
                        continue;
                    }

                    if let Some(btn) = keycode_to_button(keycode) {
                        let input = InputEvent::Press(btn);

                        match mode {
                            AppMode::Library => {
                                let (needs_redraw, selected) = browser.handle_input(input);

                                if selected.is_some() {
                                    let path = selected.unwrap();
                                    if path.is_empty() {
                                        // Reload (navigated to different directory)
                                        browser.load(&mut fs)?;
                                        browser.render(&mut display)?;
                                        window.update(&display);
                                    } else {
                                        // Open file
                                        println!("Opening: {}", path);
                                        match fs.read_file(&path) {
                                            Ok(content) => {
                                                current_file = path.clone();
                                                viewer = Some(TextViewer::new(content));
                                                mode = AppMode::Reader;
                                                if let Some(ref v) = viewer {
                                                    v.render(&mut display, &current_file)?;
                                                }
                                                window.update(&display);
                                            }
                                            Err(e) => {
                                                println!("Error reading file: {:?}", e);
                                            }
                                        }
                                    }
                                } else if needs_redraw {
                                    browser.render(&mut display)?;
                                    window.update(&display);
                                }
                            }
                            AppMode::Reader => {
                                if let Some(ref mut v) = viewer {
                                    if v.handle_input(input) {
                                        v.render(&mut display, &current_file)?;
                                        window.update(&display);
                                    } else if btn == Button::Back {
                                        // Go back to library
                                        mode = AppMode::Library;
                                        browser.render(&mut display)?;
                                        window.update(&display);
                                    }
                                }
                            }
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
