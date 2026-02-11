//! Simple file browser UI component
//!
//! Displays a list of files and allows navigation/selection.
//! Uses embedded-text for text rendering.

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use embedded_graphics::{
    mono_font::{ascii::FONT_10X20, ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    text::Text,
};

use embedded_text::{alignment::HorizontalAlignment, style::TextBoxStyleBuilder, TextBox};

use crate::filesystem::{FileInfo, FileSystem};
use crate::input::{Button, InputEvent};

/// File browser state
pub struct FileBrowser {
    current_path: String,
    files: Vec<FileInfo>,
    selected_index: usize,
    scroll_offset: usize,
    visible_items: usize,
    directory_state: BTreeMap<String, (usize, usize)>,
    status_message: Option<String>,
}

impl FileBrowser {
    /// Number of items visible on screen
    const ITEMS_PER_PAGE: usize = 8;
    /// Entry height in pixels
    const ENTRY_HEIGHT: i32 = 44;
    /// Top margin
    const TOP_MARGIN: i32 = 44;
    /// Footer height
    const FOOTER_HEIGHT: i32 = 24;

    /// Create new file browser starting at given path
    pub fn new(start_path: &str) -> Self {
        Self {
            current_path: start_path.to_string(),
            files: Vec::new(),
            selected_index: 0,
            scroll_offset: 0,
            visible_items: Self::ITEMS_PER_PAGE,
            directory_state: BTreeMap::new(),
            status_message: None,
        }
    }

    /// Set current path (used for startup defaults)
    pub fn set_path(&mut self, path: &str) {
        self.current_path = path.to_string();
        self.selected_index = 0;
        self.scroll_offset = 0;
    }

    fn save_state(&mut self) {
        self.directory_state.insert(
            self.current_path.clone(),
            (self.selected_index, self.scroll_offset),
        );
    }

    fn restore_state(&mut self) {
        if let Some((selected, scroll)) = self.directory_state.get(&self.current_path).copied() {
            self.selected_index = selected.min(self.files.len().saturating_sub(1));
            self.scroll_offset = scroll.min(self.selected_index);
        }
    }

    /// Load files from filesystem
    pub fn load<FS: FileSystem + ?Sized>(
        &mut self,
        fs: &mut FS,
    ) -> Result<(), crate::filesystem::FileSystemError> {
        let mut files = fs.list_files(&self.current_path)?;

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
        self.status_message = None;
        self.restore_state();

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
        if matches!(event, InputEvent::Press(_)) && self.status_message.is_some() {
            self.status_message = None;
        }

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
                let file = match self.files.get(self.selected_index) {
                    Some(file) => file.clone(),
                    None => return (false, None),
                };
                log::info!(
                    "CONFIRM pressed - selected: {} (is_dir: {})",
                    file.name,
                    file.is_directory
                );
                if file.is_directory {
                    // Navigate into directory
                    if file.name == ".." {
                        // Go up
                        self.save_state();
                        let old_path = self.current_path.clone();
                        self.current_path =
                            crate::filesystem::dirname(&self.current_path).to_string();
                        log::info!("Navigating UP: {} -> {}", old_path, self.current_path);
                    } else {
                        // Go down
                        self.save_state();
                        let old_path = self.current_path.clone();
                        self.current_path =
                            crate::filesystem::join_path(&self.current_path, &file.name);
                        log::info!("Navigating DOWN: {} -> {}", old_path, self.current_path);
                    }
                    return (true, Some(String::new())); // Signal to reload
                } else {
                    // Selected a file - return its path
                    let full_path = crate::filesystem::join_path(&self.current_path, &file.name);
                    log::info!("Opening file: {}", full_path);
                    return (true, Some(full_path));
                }
            }
            InputEvent::Press(Button::Back) => {
                if self.current_path != "/" {
                    self.save_state();
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

    pub fn set_status_message(&mut self, message: String) {
        self.status_message = Some(message);
    }

    pub fn status_message(&self) -> Option<&str> {
        self.status_message.as_deref()
    }

    /// Render file browser
    pub fn render<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
    ) -> Result<(), D::Error> {
        let size = display.bounding_box().size;
        let width = size.width.min(size.height);
        let height = size.width.max(size.height);

        // Clear screen
        display.clear(BinaryColor::Off)?;

        // Header
        let header_style = MonoTextStyle::new(&FONT_10X20, BinaryColor::On);
        let subheader_style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
        let header_text = if self.current_path == "/" {
            "Library".to_string()
        } else {
            crate::filesystem::basename(&self.current_path).to_string()
        };
        Text::new(&header_text, Point::new(8, 18), header_style).draw(display)?;
        Text::new(&self.current_path, Point::new(8, 32), subheader_style).draw(display)?;
        Rectangle::new(Point::new(0, 36), Size::new(width, 1))
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(display)?;

        // File list
        let name_style = MonoTextStyle::new(&FONT_10X20, BinaryColor::On);
        let meta_style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
        let bar_style = PrimitiveStyle::with_fill(BinaryColor::On);

        let end_index = (self.scroll_offset + self.visible_items).min(self.files.len());

        for (i, file) in self.files[self.scroll_offset..end_index].iter().enumerate() {
            let actual_index = self.scroll_offset + i;
            let y = Self::TOP_MARGIN + (i as i32 * Self::ENTRY_HEIGHT);

            if actual_index == self.selected_index {
                Rectangle::new(
                    Point::new(0, y - 2),
                    Size::new(4, Self::ENTRY_HEIGHT as u32),
                )
                .into_styled(bar_style)
                .draw(display)?;
            }

            // File name (truncated if too long)
            let mut name = if file.name.len() > 32 {
                format!("{}...", &file.name[..29])
            } else {
                file.name.clone()
            };

            if file.is_directory && file.name != ".." {
                name.push('/');
            }

            let meta = if file.name == ".." {
                "Parent folder".to_string()
            } else if file.is_directory {
                "Folder".to_string()
            } else if file.name.to_lowercase().ends_with(".epub") {
                format!("EPUB  {}", format_size(file.size))
            } else if file.name.to_lowercase().ends_with(".txt") {
                format!("TXT  {}", format_size(file.size))
            } else {
                format!("FILE  {}", format_size(file.size))
            };

            Text::new(&name, Point::new(10, y + 12), name_style).draw(display)?;
            Text::new(&meta, Point::new(10, y + 30), meta_style).draw(display)?;

            Rectangle::new(
                Point::new(8, y + Self::ENTRY_HEIGHT - 2),
                Size::new(width.saturating_sub(16), 1),
            )
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(display)?;
        }

        // Footer hints + position
        let footer_style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
        let position = if self.files.is_empty() {
            "0/0".to_string()
        } else {
            format!("{}/{}", self.selected_index + 1, self.files.len())
        };
        let footer_text = format!("Vol=Move  OK=Open  Back=Up   {}", position);
        Rectangle::new(
            Point::new(0, height as i32 - Self::FOOTER_HEIGHT),
            Size::new(width, 1),
        )
        .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
        .draw(display)?;
        Text::new(&footer_text, Point::new(8, height as i32 - 8), footer_style).draw(display)?;

        if let Some(message) = &self.status_message {
            let bar_height = 16;
            let bar_top = height as i32 - Self::FOOTER_HEIGHT - bar_height;
            Rectangle::new(Point::new(0, bar_top), Size::new(width, bar_height as u32))
                .into_styled(PrimitiveStyle::with_fill(BinaryColor::Off))
                .draw(display)?;
            Rectangle::new(Point::new(0, bar_top), Size::new(width, bar_height as u32))
                .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
                .draw(display)?;
            Text::new(message, Point::new(8, bar_top + 12), footer_style).draw(display)?;
        }

        Ok(())
    }
}

fn format_size(size: u64) -> String {
    if size >= 1024 * 1024 {
        format!("{:.1}MB", size as f32 / (1024.0 * 1024.0))
    } else if size >= 1024 {
        format!("{:.0}KB", size as f32 / 1024.0)
    } else {
        format!("{}B", size)
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
    const TOP_MARGIN: i32 = 44;
    #[allow(dead_code)]
    const BOTTOM_MARGIN: i32 = 40;
    const CONTENT_TOP_MARGIN: i32 = 44;
    const CONTENT_BOTTOM_MARGIN: i32 = 44;

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
        let content_height =
            crate::DISPLAY_HEIGHT as i32 - Self::CONTENT_TOP_MARGIN - Self::CONTENT_BOTTOM_MARGIN;
        let lines_per_page = (content_height / 22).max(1) as usize;

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
        let size = display.bounding_box().size;
        let width = size.width.min(size.height);
        let height = size.width.max(size.height);
        let content_height = height as i32 - Self::CONTENT_TOP_MARGIN - Self::CONTENT_BOTTOM_MARGIN;

        // Clear screen
        display.clear(BinaryColor::Off)?;

        // Header
        let header_style = MonoTextStyle::new(&FONT_10X20, BinaryColor::On);
        let subheader_style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
        let truncated_title = if title.len() > 40 {
            format!("{}...", &title[..37])
        } else {
            title.to_string()
        };
        Text::new(&truncated_title, Point::new(8, 18), header_style).draw(display)?;
        let progress_text = format!("{}/{}", self.current_page + 1, self.total_pages());
        let progress_x = width as i32 - (progress_text.len() as i32 * 6) - 8;
        Text::new(&progress_text, Point::new(progress_x, 18), subheader_style).draw(display)?;
        Rectangle::new(Point::new(0, 36), Size::new(width, 1))
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
            Size::new(width.saturating_sub(20), content_height as u32),
        );

        // Get current page content
        let page_content = &self.pages[self.current_page];

        // Create and draw text box
        let text_box =
            TextBox::with_textbox_style(page_content, bounds, character_style, textbox_style);
        text_box.draw(display)?;

        // Footer (progress bar + hints)
        let footer_style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
        let bar_width = width.saturating_sub(20);
        let bar_x = 10;
        let bar_y = height as i32 - 18;
        let total_pages = self.total_pages().max(1);
        let filled = ((bar_width as usize * (self.current_page + 1)) / total_pages) as u32;
        Rectangle::new(Point::new(bar_x, bar_y), Size::new(bar_width, 6))
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
            .draw(display)?;
        Rectangle::new(Point::new(bar_x, bar_y), Size::new(filled, 6))
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(display)?;
        let footer_text = "Left/Right to turn pages";
        Text::new(footer_text, Point::new(10, height as i32 - 6), footer_style).draw(display)?;

        Ok(())
    }
}
