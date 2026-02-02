//! Simple file browser UI component
//!
//! Displays a list of files and allows navigation/selection.
//! Uses embedded-text for text rendering.

extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use embedded_graphics::{
    mono_font::{ascii::FONT_10X20, MonoTextStyle},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    text::Text,
};

use embedded_text::{alignment::HorizontalAlignment, style::TextBoxStyleBuilder, TextBox};

use crate::filesystem::{filter_by_extension, FileInfo, FileSystem};
use crate::input::{Button, InputEvent};
use crate::{DISPLAY_HEIGHT, DISPLAY_WIDTH};

/// File browser state
pub struct FileBrowser {
    current_path: String,
    files: Vec<FileInfo>,
    selected_index: usize,
    scroll_offset: usize,
    visible_items: usize,
}

impl FileBrowser {
    /// Number of items visible on screen
    const ITEMS_PER_PAGE: usize = 12;
    /// Line height in pixels
    const LINE_HEIGHT: i32 = 30;
    /// Top margin
    const TOP_MARGIN: i32 = 40;

    /// Create new file browser starting at given path
    pub fn new(start_path: &str) -> Self {
        Self {
            current_path: start_path.to_string(),
            files: Vec::new(),
            selected_index: 0,
            scroll_offset: 0,
            visible_items: Self::ITEMS_PER_PAGE,
        }
    }

    /// Load files from filesystem
    pub fn load<FS: FileSystem>(
        &mut self,
        fs: &mut FS,
    ) -> Result<(), crate::filesystem::FileSystemError> {
        let mut files = fs.list_files(&self.current_path)?;

        // Filter to show only .txt and .epub files, plus directories
        files = filter_by_extension(&files, &[".txt", ".epub"]);

        // Sort: directories first, then alphabetically
        files.sort_by(|a, b| match (a.is_directory, b.is_directory) {
            (true, false) => core::cmp::Ordering::Less,
            (false, true) => core::cmp::Ordering::Greater,
            _ => a.name.cmp(&b.name),
        });

        // Add ".." entry if not at root
        if self.current_path != "/" {
            files.insert(
                0,
                FileInfo {
                    name: String::from(".."),
                    size: 0,
                    is_directory: true,
                },
            );
        }

        self.files = files;
        self.selected_index = 0;
        self.scroll_offset = 0;

        Ok(())
    }

    /// Handle input event
    /// Returns true if screen needs redraw
    /// Returns Some(path) if a file was selected
    pub fn handle_input(&mut self, event: InputEvent) -> (bool, Option<String>) {
        match event {
            InputEvent::Press(Button::VolumeUp) => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                    self.adjust_scroll();
                    return (true, None);
                }
            }
            InputEvent::Press(Button::VolumeDown) => {
                if self.selected_index + 1 < self.files.len() {
                    self.selected_index += 1;
                    self.adjust_scroll();
                    return (true, None);
                }
            }
            InputEvent::Press(Button::Left) => {
                // Page up
                if self.selected_index >= self.visible_items {
                    self.selected_index -= self.visible_items;
                    self.adjust_scroll();
                    return (true, None);
                }
            }
            InputEvent::Press(Button::Right) => {
                // Page down
                let new_index = self.selected_index + self.visible_items;
                if new_index < self.files.len() {
                    self.selected_index = new_index;
                    self.adjust_scroll();
                    return (true, None);
                }
            }
            InputEvent::Press(Button::Confirm) => {
                if let Some(file) = self.files.get(self.selected_index) {
                    if file.is_directory {
                        // Navigate into directory
                        if file.name == ".." {
                            // Go up
                            self.current_path =
                                crate::filesystem::dirname(&self.current_path).to_string();
                        } else {
                            // Go down
                            self.current_path =
                                crate::filesystem::join_path(&self.current_path, &file.name);
                        }
                        return (true, Some(String::new())); // Signal to reload
                    } else {
                        // Selected a file - return its path
                        let full_path =
                            crate::filesystem::join_path(&self.current_path, &file.name);
                        return (true, Some(full_path));
                    }
                }
            }
            InputEvent::Press(Button::Back) => {
                if self.current_path != "/" {
                    self.current_path = crate::filesystem::dirname(&self.current_path).to_string();
                    return (true, Some(String::new())); // Signal to reload
                }
            }
            _ => {}
        }

        (false, None)
    }

    /// Adjust scroll offset to keep selected item visible
    fn adjust_scroll(&mut self) {
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        } else if self.selected_index >= self.scroll_offset + self.visible_items {
            self.scroll_offset = self.selected_index.saturating_sub(self.visible_items - 1);
        }
    }

    /// Get current path
    pub fn current_path(&self) -> &str {
        &self.current_path
    }

    /// Get selected file info
    pub fn selected_file(&self) -> Option<&FileInfo> {
        self.files.get(self.selected_index)
    }

    /// Render file browser
    pub fn render<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
    ) -> Result<(), D::Error> {
        // Clear screen
        display.clear(BinaryColor::Off)?;

        // Header
        let header_style = MonoTextStyle::new(&FONT_10X20, BinaryColor::On);
        let header_text = if self.current_path == "/" {
            String::from("Library")
        } else {
            crate::filesystem::basename(&self.current_path).to_string()
        };
        Text::new(&header_text, Point::new(10, 25), header_style).draw(display)?;

        // Draw header line
        Rectangle::new(Point::new(0, 32), Size::new(DISPLAY_WIDTH, 2))
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(display)?;

        // File list
        let normal_style = MonoTextStyle::new(&FONT_10X20, BinaryColor::On);
        let selected_style = MonoTextStyle::new(&FONT_10X20, BinaryColor::Off);
        // Use ASCII icons that render properly
        let file_icon = "[-] ";
        let folder_icon = "[+] ";
        let up_icon = "[^] ";

        let end_index = (self.scroll_offset + self.visible_items).min(self.files.len());

        for (i, file) in self.files[self.scroll_offset..end_index].iter().enumerate() {
            let actual_index = self.scroll_offset + i;
            let y = Self::TOP_MARGIN + (i as i32 * Self::LINE_HEIGHT);

            // Highlight selected item
            if actual_index == self.selected_index {
                Rectangle::new(
                    Point::new(0, y - 22),
                    Size::new(DISPLAY_WIDTH, Self::LINE_HEIGHT as u32),
                )
                .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
                .draw(display)?;
            }

            // Icon
            let icon = if file.name == ".." {
                up_icon
            } else if file.is_directory {
                folder_icon
            } else {
                file_icon
            };

            // File name (truncated if too long)
            let name = if file.name.len() > 35 {
                format!("{}...", &file.name[..32])
            } else {
                file.name.clone()
            };

            let display_text = format!("{}{}", icon, name);
            let style = if actual_index == self.selected_index {
                selected_style
            } else {
                normal_style
            };

            Text::new(&display_text, Point::new(10, y), style).draw(display)?;
        }

        // Footer with help
        let help_style = MonoTextStyle::new(&FONT_10X20, BinaryColor::On);
        let help_text = format!("{} files | VOL=Navigate | ENT=Open", self.files.len());
        Text::new(
            &help_text,
            Point::new(10, DISPLAY_HEIGHT as i32 - 10),
            help_style,
        )
        .draw(display)?;

        Ok(())
    }
}

/// Text viewer using embedded-text for proper word wrapping
pub struct TextViewer {
    #[allow(dead_code)]
    content: String,
    current_page: usize,
    pages: Vec<String>,
}

impl TextViewer {
    const TOP_MARGIN: i32 = 50;
    #[allow(dead_code)]
    const BOTTOM_MARGIN: i32 = 40;
    const CONTENT_HEIGHT: i32 = DISPLAY_HEIGHT as i32 - 50 - 40; // 710px

    /// Create new text viewer with content
    /// Automatically paginates the content to fit the display
    pub fn new(content: String) -> Self {
        // Paginate content into screen-sized chunks
        let pages = Self::paginate_content(&content);

        Self {
            content,
            current_page: 0,
            pages,
        }
    }

    /// Paginate content by measuring with embedded-text
    fn paginate_content(content: &str) -> Vec<String> {
        // For now, simple pagination by line count
        // TODO: Use embedded-text to measure actual rendered height
        let lines: Vec<&str> = content.lines().collect();
        let lines_per_page = 25; // Approximate for 710px / 24px line height

        let mut pages = Vec::new();
        for chunk in lines.chunks(lines_per_page) {
            pages.push(chunk.join("\n"));
        }

        if pages.is_empty() {
            pages.push(String::new());
        }

        pages
    }

    /// Get total pages
    pub fn total_pages(&self) -> usize {
        self.pages.len()
    }

    /// Get current page content
    pub fn current_content(&self) -> &str {
        &self.pages[self.current_page]
    }

    /// Handle input
    /// Returns true if needs redraw
    pub fn handle_input(&mut self, event: InputEvent) -> bool {
        match event {
            InputEvent::Press(Button::Left) | InputEvent::Press(Button::VolumeUp) => {
                if self.current_page > 0 {
                    self.current_page -= 1;
                    return true;
                }
            }
            InputEvent::Press(Button::Right) | InputEvent::Press(Button::VolumeDown) => {
                if self.current_page + 1 < self.total_pages() {
                    self.current_page += 1;
                    return true;
                }
            }
            _ => {}
        }
        false
    }

    /// Render text viewer using embedded-text for proper word wrapping
    pub fn render<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
        title: &str,
    ) -> Result<(), D::Error> {
        // Clear screen
        display.clear(BinaryColor::Off)?;

        // Header with title
        let header_style = MonoTextStyle::new(&FONT_10X20, BinaryColor::On);
        let truncated_title = if title.len() > 40 {
            format!("{}...", &title[..37])
        } else {
            title.to_string()
        };
        Text::new(&truncated_title, Point::new(10, 25), header_style).draw(display)?;

        // Header line
        Rectangle::new(Point::new(0, 32), Size::new(DISPLAY_WIDTH, 2))
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(display)?;

        // Use embedded-text TextBox for proper word wrapping
        let character_style = MonoTextStyle::new(&FONT_10X20, BinaryColor::On);
        let textbox_style = TextBoxStyleBuilder::new()
            .alignment(HorizontalAlignment::Left)
            .paragraph_spacing(6)
            .build();

        // Define content area (480x710)
        let bounds = Rectangle::new(
            Point::new(10, Self::TOP_MARGIN),
            Size::new(DISPLAY_WIDTH - 20, Self::CONTENT_HEIGHT as u32),
        );

        // Get current page content
        let page_content = &self.pages[self.current_page];

        // Create and draw text box
        let text_box =
            TextBox::with_textbox_style(page_content, bounds, character_style, textbox_style);
        text_box.draw(display)?;

        // Footer with page number
        let footer_style = MonoTextStyle::new(&FONT_10X20, BinaryColor::On);
        let footer_text = format!(
            "Page {} of {} | <=Back | >=Next",
            self.current_page + 1,
            self.total_pages()
        );
        Text::new(
            &footer_text,
            Point::new(10, DISPLAY_HEIGHT as i32 - 10),
            footer_style,
        )
        .draw(display)?;

        Ok(())
    }
}
