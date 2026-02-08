//! Desktop SDL simulator for Xteink X4.
//!
//! Uses the activity-based App for full UI navigation.

use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics_simulator::{
    sdl2::Keycode, OutputSettingsBuilder, SimulatorDisplay, SimulatorEvent, Window,
};
use std::path::{Path, PathBuf};
use xteink_ui::{App, Button, InputEvent, MockFileSystem, DISPLAY_HEIGHT, DISPLAY_WIDTH};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_settings = OutputSettingsBuilder::new().scale(1).build();
    let mut display: SimulatorDisplay<BinaryColor> =
        SimulatorDisplay::new(Size::new(DISPLAY_WIDTH, DISPLAY_HEIGHT));
    let mut window = Window::new("Xteink X4", &output_settings);

    // Create activity-based app
    let mut app = App::new();
    let mut fs = MockFileSystem::new();
    let synced = sync_sample_books_from_workspace(&mut fs).unwrap_or(0);
    if synced > 0 {
        println!(
            "Loaded {} file(s) from sample_books/ into simulator FS",
            synced
        );
    }

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

                        let needs_redraw = app.handle_input(input);
                        let deferred_updated = app.process_deferred_tasks(&mut fs);

                        if needs_redraw || deferred_updated {
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

fn sync_sample_books_from_workspace(fs: &mut MockFileSystem) -> Result<usize, std::io::Error> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let sample_root = manifest_dir.join("../../sample_books");
    if !sample_root.exists() {
        return Ok(0);
    }

    fs.add_directory("/books");
    let mut added = 0usize;
    sync_directory(fs, &sample_root, &sample_root, &mut added)?;
    Ok(added)
}

fn sync_directory(
    fs: &mut MockFileSystem,
    root: &Path,
    dir: &Path,
    added: &mut usize,
) -> Result<(), std::io::Error> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let rel = path.strip_prefix(root).expect("sample root prefix");
            let sim_path = rel_to_sim_path(rel);
            fs.add_directory(&sim_path);
            sync_directory(fs, root, &path, added)?;
            continue;
        }

        if !path.is_file() {
            continue;
        }

        let rel = path.strip_prefix(root).expect("sample root prefix");
        let sim_path = rel_to_sim_path(rel);
        let bytes = std::fs::read(&path)?;
        fs.add_file(&sim_path, &bytes);
        *added += 1;
    }
    Ok(())
}

fn rel_to_sim_path(rel: &Path) -> String {
    let mut out = String::from("/books");
    for component in rel.components() {
        out.push('/');
        out.push_str(component.as_os_str().to_str().unwrap_or_default());
    }
    out
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
