//! Main application state.

extern crate alloc;

use alloc::string::String;

use embedded_graphics::{pixelcolor::BinaryColor, prelude::*};

use crate::file_browser::{FileBrowser, TextViewer};
use crate::filesystem::{basename, FileSystem};
use crate::input::{Button, InputEvent};

/// Application screen state
pub enum Screen {
    FileBrowser,
    TextViewer { title: String },
}

/// Application state
pub struct App {
    screen: Screen,
    file_browser: FileBrowser,
    text_viewer: Option<TextViewer>,
}

impl App {
    pub fn new() -> Self {
        Self {
            screen: Screen::FileBrowser,
            file_browser: FileBrowser::new("/"),
            text_viewer: None,
        }
    }

    /// Initialize with filesystem - call after creation
    pub fn init<FS: FileSystem>(&mut self, fs: &mut FS) {
        let _ = self.file_browser.load(fs);
    }

    /// Handle input. Returns true if redraw needed.
    pub fn handle_input<FS: FileSystem>(&mut self, event: InputEvent, fs: &mut FS) -> bool {
        match &self.screen {
            Screen::FileBrowser => {
                let (redraw, selected) = self.file_browser.handle_input(event);

                if let Some(path) = selected {
                    if path.is_empty() {
                        let _ = self.file_browser.load(fs);
                        return true;
                    }

                    if let Ok(content) = fs.read_file(&path) {
                        let title = basename(&path).into();
                        self.text_viewer = Some(TextViewer::new(content));
                        self.screen = Screen::TextViewer { title };
                        return true;
                    }
                }

                redraw
            }
            Screen::TextViewer { .. } => {
                if let InputEvent::Press(Button::Back) = event {
                    self.screen = Screen::FileBrowser;
                    self.text_viewer = None;
                    return true;
                }

                if let Some(viewer) = &mut self.text_viewer {
                    viewer.handle_input(event)
                } else {
                    false
                }
            }
        }
    }

    /// Render to any display.
    pub fn render<D: DrawTarget<Color = BinaryColor> + OriginDimensions>(
        &self,
        display: &mut D,
    ) -> Result<(), D::Error> {
        match &self.screen {
            Screen::FileBrowser => self.file_browser.render(display),
            Screen::TextViewer { title } => {
                if let Some(viewer) = &self.text_viewer {
                    viewer.render(display, title)
                } else {
                    Ok(())
                }
            }
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
