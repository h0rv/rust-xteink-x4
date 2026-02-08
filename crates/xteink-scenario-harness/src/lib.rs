//! Host-side scenario test harness for scripted UI flows.

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use std::time::{Duration, Instant};

use embedded_graphics::pixelcolor::BinaryColor;
use png::{BitDepth, ColorType, Encoder};
use xteink_ui::test_display::TestDisplay;
use xteink_ui::{App, Button, InputEvent, MockFileSystem};

/// Small helper that couples app, mock filesystem, and display for scenario tests.
pub struct ScenarioHarness {
    app: App,
    fs: MockFileSystem,
    display: TestDisplay,
}

impl ScenarioHarness {
    /// Construct a harness with caller-provided app and mock filesystem state.
    pub fn new(app: App, fs: MockFileSystem) -> Self {
        Self {
            app,
            fs,
            display: TestDisplay::default_size(),
        }
    }

    /// Simulate a button press through the app input pipeline.
    pub fn press(&mut self, button: Button) -> bool {
        self.app.handle_input(InputEvent::Press(button))
    }

    /// Pump deferred tasks until idle or a safety cap is reached.
    pub fn pump_deferred_until_idle(&mut self) -> usize {
        const MAX_PUMPS: usize = 32;
        let mut updates = 0;

        for _ in 0..MAX_PUMPS {
            if !self.app.process_deferred_tasks(&mut self.fs) {
                break;
            }
            updates += 1;
        }

        updates
    }

    /// Render the current UI screen.
    pub fn render(&mut self) {
        self.app
            .render(&mut self.display)
            .expect("scenario render should succeed");
    }

    /// Render and return elapsed wall time.
    pub fn render_timed(&mut self) -> Duration {
        let start = Instant::now();
        self.render();
        start.elapsed()
    }

    /// Render and assert wall-time budget in milliseconds.
    pub fn assert_render_budget_ms(&mut self, max_ms: u128, label: &str) {
        let elapsed = self.render_timed();
        assert!(
            elapsed.as_millis() <= max_ms,
            "{} render exceeded budget: {}ms > {}ms",
            label,
            elapsed.as_millis(),
            max_ms
        );
    }

    /// Access the app for assertions.
    pub fn app(&self) -> &App {
        &self.app
    }

    /// Access the display for render assertions.
    pub fn display(&self) -> &TestDisplay {
        &self.display
    }

    /// Access mock filesystem for scenario setup.
    pub fn fs_mut(&mut self) -> &mut MockFileSystem {
        &mut self.fs
    }

    /// Save the current framebuffer to a PNG (white = Off, black = On).
    pub fn save_screenshot_png(&self, path: impl AsRef<Path>) -> Result<(), String> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        let (width, height) = self.display.dimensions();
        let mut data = Vec::with_capacity((width * height) as usize);
        for pixel in self.display.pixels() {
            let value = match pixel {
                BinaryColor::On => 0u8,
                BinaryColor::Off => 255u8,
            };
            data.push(value);
        }

        let file = File::create(path).map_err(|e| e.to_string())?;
        let writer = BufWriter::new(file);
        let mut encoder = Encoder::new(writer, width, height);
        encoder.set_color(ColorType::Grayscale);
        encoder.set_depth(BitDepth::Eight);
        let mut png_writer = encoder.write_header().map_err(|e| e.to_string())?;
        png_writer
            .write_image_data(&data)
            .map_err(|e| e.to_string())
    }
}
