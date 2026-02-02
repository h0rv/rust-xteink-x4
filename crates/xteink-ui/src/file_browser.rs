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
    primitives::Rectangle,
    text::Text,
};

use embedded_text::{alignment::HorizontalAlignment, style::TextBoxStyleBuilder, TextBox};

use crate::filesystem::{filter_by_extension, FileInfo, FileSystem};
use crate::input::{Button, InputEvent};
use crate::portrait_dimensions;

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
    const ITEMS_PER_PAGE: usize = 14;
    /// Line height in pixels
    const LINE_HEIGHT: i32 = 24;
    /// Top margin
    const TOP_MARGIN: i32 = 48;

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

    /// Set current path (used for startup defaults)
    pub fn set_path(&mut self, path: &str) {
        self.current_path = path.to_string();
        self.selected_index = 0;
        self.scroll_offset = 0;
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

        // Log loaded directory contents
        log::info!(
            "LOAD: {} files in '{}'",
            self.files.len(),
            self.current_path
        );
        for (i, file) in self.files.iter().take(5).enumerate() {
            let file_type = if file.is_directory { "DIR" } else { "FILE" };
            log::info!("  [{}] {} ({})", i, file.name, file_type);
        }
        if self.files.len() > 5 {
            log::info!("  ... and {} more files", self.files.len() - 5);
        }

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
                    if let Some(file) = self.files.get(self.selected_index) {
                        log::info!(
                            "NAV: UP -> selected [{}] {}",
                            self.selected_index,
                            file.name
                        );
                    }
                    return (true, None);
                }
            }
            InputEvent::Press(Button::VolumeDown) => {
                if self.selected_index + 1 < self.files.len() {
                    self.selected_index += 1;
                    self.adjust_scroll();
                    if let Some(file) = self.files.get(self.selected_index) {
                        log::info!(
                            "NAV: DOWN -> selected [{}] {}",
                            self.selected_index,
                            file.name
                        );
                    }
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
                    log::info!(
                        "CONFIRM pressed - selected: {} (is_dir: {})",
                        file.name,
                        file.is_directory
                    );
                    if file.is_directory {
                        // Navigate into directory
                        if file.name == ".." {
                            // Go up
                            let old_path = self.current_path.clone();
                            self.current_path =
                                crate::filesystem::dirname(&self.current_path).to_string();
                            log::info!("Navigating UP: {} -> {}", old_path, self.current_path);
                        } else {
                            // Go down
                            let old_path = self.current_path.clone();
                            self.current_path =
                                crate::filesystem::join_path(&self.current_path, &file.name);
                            log::info!("Navigating DOWN: {} -> {}", old_path, self.current_path);
                        }
                        return (true, Some(String::new())); // Signal to reload
                    } else {
                        // Selected a file - return its path
                        let full_path =
                            crate::filesystem::join_path(&self.current_path, &file.name);
                        log::info!("Opening file: {}", full_path);
                        return (true, Some(full_path));
                    }
                }
            }
            InputEvent::Press(Button::Back) => {
                if self.current_path != "/" {
                    let old_path = self.current_path.clone();
                    self.current_path = crate::filesystem::dirname(&self.current_path).to_string();
                    log::info!("BACK pressed: {} -> {}", old_path, self.current_path);
                    return (true, Some(String::new())); // Signal to reload
                } else {
                    log::info!("BACK pressed at root - no action");
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
    pub fn render<D: DrawTarget<Color = BinaryColor> + OriginDimensions>(
        &self,
        display: &mut D,
    ) -> Result<(), D::Error> {
        let (_width, _height) = portrait_dimensions(display);

        // Clear screen
        display.clear(BinaryColor::Off)?;

        // Header (minimal)
        let header_style = MonoTextStyle::new(&FONT_10X20, BinaryColor::On);
        let header_text = if self.current_path == "/" {
            "Library".to_string()
        } else {
            crate::filesystem::basename(&self.current_path).to_string()
        };
        Text::new(&header_text, Point::new(6, 18), header_style).draw(display)?;

        // File list (no icons, minimal chrome)
        let normal_style = MonoTextStyle::new(&FONT_10X20, BinaryColor::On);
        let selected_style = MonoTextStyle::new(&FONT_10X20, BinaryColor::On);

        let end_index = (self.scroll_offset + self.visible_items).min(self.files.len());

        for (i, file) in self.files[self.scroll_offset..end_index].iter().enumerate() {
            let actual_index = self.scroll_offset + i;
            let y = Self::TOP_MARGIN + (i as i32 * Self::LINE_HEIGHT);

            // File name (truncated if too long)
            let mut name = if file.name.len() > 38 {
                format!("{}...", &file.name[..35])
            } else {
                file.name.clone()
            };

            if file.is_directory && file.name != ".." {
                name.push('/');
            }

            let display_text = if actual_index == self.selected_index {
                format!("> {}", name)
            } else {
                format!("  {}", name)
            };
            let style = if actual_index == self.selected_index {
                selected_style
            } else {
                normal_style
            };

            Text::new(&display_text, Point::new(6, y), style).draw(display)?;
        }

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
    const CONTENT_TOP_MARGIN: i32 = 50;
    const CONTENT_BOTTOM_MARGIN: i32 = 40;

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
    pub fn render<D: DrawTarget<Color = BinaryColor> + OriginDimensions>(
        &self,
        display: &mut D,
        title: &str,
    ) -> Result<(), D::Error> {
        let (width, height) = portrait_dimensions(display);
        let content_height = height as i32 - Self::CONTENT_TOP_MARGIN - Self::CONTENT_BOTTOM_MARGIN;

        // Clear screen
        display.clear(BinaryColor::Off)?;

        // Minimal header
        let header_style = MonoTextStyle::new(&FONT_10X20, BinaryColor::On);
        let truncated_title = if title.len() > 40 {
            format!("{}...", &title[..37])
        } else {
            title.to_string()
        };
        Text::new(&truncated_title, Point::new(6, 18), header_style).draw(display)?;

        // Use embedded-text TextBox for proper word wrapping
        let character_style = MonoTextStyle::new(&FONT_10X20, BinaryColor::On);
        let textbox_style = TextBoxStyleBuilder::new()
            .alignment(HorizontalAlignment::Left)
            .paragraph_spacing(6)
            .build();

        // Define content area (480x710)
        let bounds = Rectangle::new(
            Point::new(10, Self::TOP_MARGIN),
            Size::new(width.saturating_sub(20), content_height as u32),
        );

        // Get current page content
        let page_content = &self.pages[self.current_page];

        // Create and draw text box
        let text_box =
            TextBox::with_textbox_style(page_content, bounds, character_style, textbox_style);
        text_box.draw(display)?;

        // Footer (page number only)
        let footer_style = MonoTextStyle::new(&FONT_10X20, BinaryColor::On);
        let footer_text = format!("{}/{}", self.current_page + 1, self.total_pages());
        Text::new(
            &footer_text,
            Point::new(width as i32 - 60, height as i32 - 10),
            footer_style,
        )
        .draw(display)?;

        Ok(())
    }
}
