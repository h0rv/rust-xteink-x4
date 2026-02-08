//! Host-side scenario test harness for scripted UI flows.

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
}
