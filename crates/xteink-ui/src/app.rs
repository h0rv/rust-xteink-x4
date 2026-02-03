//! Main application state.

extern crate alloc;

use alloc::string::{String, ToString};

use embedded_graphics::{pixelcolor::BinaryColor, prelude::*};

#[cfg(feature = "std")]
use crate::epub_render::EpubRenderer;
use crate::file_browser::{FileBrowser, TextViewer};
use crate::filesystem::{basename, FileSystem};
use crate::input::{Button, InputEvent};

/// Application screen state
#[derive(Debug)]
pub enum Screen {
    FileBrowser,
    TextViewer {
        title: String,
    },
    #[cfg(feature = "std")]
    EpubViewer {
        title: String,
    },
}

/// Application state
pub struct App {
    screen: Screen,
    file_browser: FileBrowser,
    text_viewer: Option<TextViewer>,
    #[cfg(feature = "std")]
    epub_renderer: Option<EpubRenderer>,
}

impl App {
    pub fn new() -> Self {
        Self {
            screen: Screen::FileBrowser,
            file_browser: FileBrowser::new("/"),
            text_viewer: None,
            #[cfg(feature = "std")]
            epub_renderer: None,
        }
    }

    /// Initialize with filesystem - call after creation
    pub fn init<FS: FileSystem>(&mut self, fs: &mut FS) {
        // Prefer /books if it exists to avoid system folders
        if fs.exists("/books") {
            self.file_browser.set_path("/books");
        }
        let _ = self.file_browser.load(fs);
    }

    /// Handle input. Returns true if redraw needed.
    pub fn handle_input<FS: FileSystem>(&mut self, event: InputEvent, fs: &mut FS) -> bool {
        log::info!("APP: handle_input {:?} in {:?}", event, self.screen);

        match &self.screen {
            Screen::FileBrowser => {
                let (redraw, selected) = self.file_browser.handle_input(event);
                log::info!(
                    "APP: FileBrowser result - redraw: {}, selected: {:?}",
                    redraw,
                    selected
                );

                if let Some(path) = selected {
                    if path.is_empty() {
                        let _ = self.file_browser.load(fs);
                        return true;
                    }

                    if path.to_lowercase().ends_with(".epub") {
                        #[cfg(feature = "std")]
                        {
                            let mut renderer = EpubRenderer::new();
                            if renderer.load(&path).is_ok() {
                                let title = basename(&path).into();
                                self.epub_renderer = Some(renderer);
                                self.screen = Screen::EpubViewer { title };
                                return true;
                            }
                        }

                        #[cfg(not(feature = "std"))]
                        {
                            let title = basename(&path).into();
                            let message = "EPUB support is not available on firmware yet.\n\nUse the desktop or web simulator for EPUB rendering.";
                            self.text_viewer = Some(TextViewer::new(message.to_string()));
                            self.screen = Screen::TextViewer { title };
                            return true;
                        }
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
            #[cfg(feature = "std")]
            Screen::EpubViewer { .. } => {
                if let InputEvent::Press(Button::Back) = event {
                    self.screen = Screen::FileBrowser;
                    self.epub_renderer = None;
                    return true;
                }

                if let Some(renderer) = &mut self.epub_renderer {
                    match event {
                        InputEvent::Press(Button::Left) | InputEvent::Press(Button::VolumeUp) => {
                            renderer.prev_page()
                        }
                        InputEvent::Press(Button::Right)
                        | InputEvent::Press(Button::VolumeDown) => renderer.next_page(),
                        _ => false,
                    }
                } else {
                    false
                }
            }
        }
    }

    /// Render to any display.
    pub fn render<D: DrawTarget<Color = BinaryColor> + OriginDimensions>(
        &mut self,
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
            #[cfg(feature = "std")]
            Screen::EpubViewer { .. } => {
                if let Some(renderer) = &mut self.epub_renderer {
                    renderer.render(display)
                } else {
                    Ok(())
                }
            }
        }
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    use crate::input::Button;
    use crate::mock_filesystem::MockFileSystem;

    #[test]
    fn app_file_browser_navigation_changes_selection() {
        let mut fs = MockFileSystem::new();
        let mut app = App::new();
        app.init(&mut fs);

        // Navigate down in file browser
        let changed = app.handle_input(InputEvent::Press(Button::VolumeDown), &mut fs);
        assert!(changed, "Expected redraw after VolumeDown");
    }

    #[test]
    fn app_open_and_close_file() {
        let mut fs = MockFileSystem::new();
        let mut app = App::new();
        app.init(&mut fs);

        // Open first entry (if it is a directory or file, should trigger redraw)
        let changed = app.handle_input(InputEvent::Press(Button::Confirm), &mut fs);
        assert!(changed, "Expected redraw after Confirm");

        // Back should return to file browser
        let changed = app.handle_input(InputEvent::Press(Button::Back), &mut fs);
        assert!(changed, "Expected redraw after Back");
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
