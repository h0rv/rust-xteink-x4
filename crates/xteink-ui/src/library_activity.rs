//! Library Browser Activity for Xteink X4 e-reader.
//!
//! Provides a scrollable book list with cover placeholders,
//! reading progress bars, and sorting options.

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use embedded_graphics::{
    mono_font::{ascii, MonoTextStyle, MonoTextStyleBuilder},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    text::Text,
};

use crate::input::{Button, InputEvent};
use crate::ui::{Activity, ActivityResult, Modal, Theme, ThemeMetrics, FONT_CHAR_WIDTH};
use crate::DISPLAY_HEIGHT;

/// Book information structure
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookInfo {
    pub title: String,
    pub author: String,
    pub path: String,
    pub progress_percent: u8,
    pub last_read: Option<u64>, // timestamp
}

impl BookInfo {
    /// Create a new book info
    pub fn new(
        title: impl Into<String>,
        author: impl Into<String>,
        path: impl Into<String>,
        progress_percent: u8,
        last_read: Option<u64>,
    ) -> Self {
        Self {
            title: title.into(),
            author: author.into(),
            path: path.into(),
            progress_percent: progress_percent.min(100),
            last_read,
        }
    }

    /// Get display title (truncated if needed)
    pub fn display_title(&self, max_chars: usize) -> &str {
        if self.title.len() <= max_chars {
            &self.title
        } else {
            &self.title[..max_chars]
        }
    }
}

/// Sort order for the library
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortOrder {
    #[default]
    Title,
    Author,
    Recent,
}

impl SortOrder {
    /// All sort variants
    pub const ALL: [Self; 3] = [Self::Title, Self::Author, Self::Recent];

    /// Get display label
    pub const fn label(self) -> &'static str {
        match self {
            Self::Title => "Title",
            Self::Author => "Author",
            Self::Recent => "Recent",
        }
    }

    /// Get next sort order
    pub const fn next(self) -> Self {
        match self {
            Self::Title => Self::Author,
            Self::Author => Self::Recent,
            Self::Recent => Self::Title,
        }
    }
}

/// Context menu actions for books
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BookAction {
    Open,
    MarkUnread,
    Delete,
    Cancel,
}

/// Library Browser Activity
#[derive(Debug, Clone)]
pub struct LibraryActivity {
    books: Vec<BookInfo>,
    filtered_books: Vec<usize>, // indices into books
    selected_index: usize,
    scroll_offset: usize,
    sort_order: SortOrder,
    theme: Theme,
    show_context_menu: bool,
    context_menu_index: usize,
    show_toast: bool,
    toast_message: String,
    toast_frames_remaining: u32,
    visible_count: usize,
}

impl LibraryActivity {
    /// Toast display duration in frames
    const TOAST_DURATION: u32 = 120; // ~2 seconds at 60fps

    /// Cover placeholder width
    const COVER_WIDTH: u32 = 50;

    /// Create a new library activity with empty book list
    pub fn new() -> Self {
        let theme = Theme::default();
        let visible_count = theme.metrics.visible_items(DISPLAY_HEIGHT);

        Self {
            books: Vec::new(),
            filtered_books: Vec::new(),
            selected_index: 0,
            scroll_offset: 0,
            sort_order: SortOrder::default(),
            theme,
            show_context_menu: false,
            context_menu_index: 0,
            show_toast: false,
            toast_message: String::new(),
            toast_frames_remaining: 0,
            visible_count,
        }
    }

    /// Create with book list
    pub fn with_books(books: Vec<BookInfo>) -> Self {
        let mut activity = Self::new();
        activity.set_books(books);
        activity
    }

    /// Create with mock books for testing
    pub fn with_mock_books() -> Self {
        Self::with_books(create_mock_books())
    }

    /// Set the book list and refresh
    pub fn set_books(&mut self, books: Vec<BookInfo>) {
        self.books = books;
        self.apply_sort();
        self.selected_index = 0;
        self.scroll_offset = 0;
    }

    /// Get book count
    pub fn book_count(&self) -> usize {
        self.books.len()
    }

    /// Check if library is empty
    pub fn is_empty(&self) -> bool {
        self.books.is_empty()
    }

    /// Get currently selected book
    pub fn selected_book(&self) -> Option<&BookInfo> {
        self.filtered_books
            .get(self.selected_index)
            .and_then(|&idx| self.books.get(idx))
    }

    /// Apply current sort order
    fn apply_sort(&mut self) {
        self.filtered_books = (0..self.books.len()).collect();

        match self.sort_order {
            SortOrder::Title => {
                self.filtered_books
                    .sort_by(|&a, &b| self.books[a].title.cmp(&self.books[b].title));
            }
            SortOrder::Author => {
                self.filtered_books
                    .sort_by(|&a, &b| self.books[a].author.cmp(&self.books[b].author));
            }
            SortOrder::Recent => {
                self.filtered_books.sort_by(|&a, &b| {
                    match (self.books[a].last_read, self.books[b].last_read) {
                        (Some(ta), Some(tb)) => tb.cmp(&ta), // Most recent first
                        (Some(_), None) => core::cmp::Ordering::Less,
                        (None, Some(_)) => core::cmp::Ordering::Greater,
                        (None, None) => core::cmp::Ordering::Equal,
                    }
                });
            }
        }
    }

    /// Cycle to next sort order
    fn cycle_sort(&mut self) {
        self.sort_order = self.sort_order.next();
        self.apply_sort();
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.show_toast(format!("Sorted by: {}", self.sort_order.label()));
    }

    /// Move selection down (wraps around)
    fn select_next(&mut self) {
        if !self.filtered_books.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.filtered_books.len();
            self.ensure_visible();
        }
    }

    /// Move selection up (wraps around)
    fn select_prev(&mut self) {
        if !self.filtered_books.is_empty() {
            self.selected_index = if self.selected_index == 0 {
                self.filtered_books.len() - 1
            } else {
                self.selected_index - 1
            };
            self.ensure_visible();
        }
    }

    /// Ensure selected item is visible
    fn ensure_visible(&mut self) {
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        } else if self.selected_index >= self.scroll_offset + self.visible_count {
            self.scroll_offset = self.selected_index.saturating_sub(self.visible_count - 1);
        }
    }

    /// Show a toast notification
    fn show_toast(&mut self, message: impl Into<String>) {
        self.toast_message = message.into();
        self.show_toast = true;
        self.toast_frames_remaining = Self::TOAST_DURATION;
    }

    /// Update toast state (call once per frame)
    pub fn update(&mut self) {
        if self.show_toast && self.toast_frames_remaining > 0 {
            self.toast_frames_remaining -= 1;
            if self.toast_frames_remaining == 0 {
                self.show_toast = false;
            }
        }
    }

    /// Open context menu for selected book
    #[cfg(test)]
    fn open_context_menu(&mut self) {
        if !self.filtered_books.is_empty() {
            self.show_context_menu = true;
            self.context_menu_index = 0;
        }
    }

    /// Close context menu
    fn close_context_menu(&mut self) {
        self.show_context_menu = false;
    }

    /// Handle context menu navigation (wraps)
    fn context_menu_next(&mut self) {
        self.context_menu_index = (self.context_menu_index + 1) % 4; // 4 actions
    }

    fn context_menu_prev(&mut self) {
        self.context_menu_index = if self.context_menu_index == 0 {
            3
        } else {
            self.context_menu_index - 1
        };
    }

    /// Get current context menu action
    fn current_action(&self) -> BookAction {
        match self.context_menu_index {
            0 => BookAction::Open,
            1 => BookAction::MarkUnread,
            2 => BookAction::Delete,
            _ => BookAction::Cancel,
        }
    }

    /// Execute context menu action
    fn execute_action(&mut self, action: BookAction) -> ActivityResult {
        self.close_context_menu();

        match action {
            BookAction::Open => {
                if let Some(book) = self.selected_book() {
                    self.show_toast(format!("Opening: {}", book.title));
                    // In real implementation, navigate to reader
                    ActivityResult::Consumed
                } else {
                    ActivityResult::Consumed
                }
            }
            BookAction::MarkUnread => {
                if let Some(&idx) = self.filtered_books.get(self.selected_index) {
                    self.books[idx].progress_percent = 0;
                    self.show_toast("Marked as unread");
                }
                ActivityResult::Consumed
            }
            BookAction::Delete => {
                // In real implementation, show confirmation modal
                if let Some(&idx) = self.filtered_books.get(self.selected_index) {
                    let title = self.books[idx].title.clone();
                    self.show_toast(format!("Deleted: {}", title));
                    // Remove from list
                    self.books.remove(idx);
                    self.apply_sort();
                    if self.selected_index >= self.filtered_books.len() && self.selected_index > 0 {
                        self.selected_index -= 1;
                    }
                }
                ActivityResult::Consumed
            }
            BookAction::Cancel => ActivityResult::Consumed,
        }
    }

    /// Handle input when context menu is shown.
    /// Left/Right and VolumeUp/Down navigate buttons, Confirm selects, Back cancels.
    fn handle_context_menu_input(&mut self, event: InputEvent) -> ActivityResult {
        match event {
            InputEvent::Press(Button::Right) | InputEvent::Press(Button::VolumeDown) => {
                self.context_menu_next();
                ActivityResult::Consumed
            }
            InputEvent::Press(Button::Left) | InputEvent::Press(Button::VolumeUp) => {
                self.context_menu_prev();
                ActivityResult::Consumed
            }
            InputEvent::Press(Button::Confirm) => {
                let action = self.current_action();
                self.execute_action(action)
            }
            InputEvent::Press(Button::Back) => {
                self.close_context_menu();
                ActivityResult::Consumed
            }
            _ => ActivityResult::Ignored,
        }
    }

    /// Render header bar with title and sort button
    fn render_header<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
    ) -> Result<(), D::Error> {
        let display_width = display.bounding_box().size.width;
        let header_height = self.theme.metrics.header_height;
        let header_y = self.theme.metrics.header_text_y();

        // Header background
        Rectangle::new(Point::new(0, 0), Size::new(display_width, header_height))
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(display)?;

        // Title
        let title_style = MonoTextStyleBuilder::new()
            .font(&ascii::FONT_7X13_BOLD)
            .text_color(BinaryColor::Off)
            .build();
        Text::new(
            "Library",
            Point::new(self.theme.metrics.side_padding as i32, header_y),
            title_style,
        )
        .draw(display)?;

        // Sort button
        let sort_label = format!("[Sort: {}]", self.sort_order.label());
        let sort_width = ThemeMetrics::text_width(sort_label.len());
        let sort_style = MonoTextStyle::new(&ascii::FONT_7X13, BinaryColor::Off);
        Text::new(
            &sort_label,
            Point::new(
                display_width as i32 - sort_width - self.theme.metrics.side_padding as i32,
                header_y,
            ),
            sort_style,
        )
        .draw(display)?;

        Ok(())
    }

    /// Render book list or empty state
    fn render_book_list<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
    ) -> Result<(), D::Error> {
        if self.filtered_books.is_empty() {
            self.render_empty_state(display)?;
        } else {
            self.render_books(display)?;
        }
        Ok(())
    }

    /// Render empty state message
    fn render_empty_state<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
    ) -> Result<(), D::Error> {
        let display_width = display.bounding_box().size.width;
        let display_height = display.bounding_box().size.height;
        let center_y = (display_height / 2) as i32;

        let message = "No books found";
        let message_width = ThemeMetrics::text_width(message.len());
        let x = (display_width as i32 - message_width) / 2;

        let style = MonoTextStyleBuilder::new()
            .font(&ascii::FONT_7X13_BOLD)
            .text_color(BinaryColor::On)
            .build();

        Text::new(message, Point::new(x, center_y), style).draw(display)?;

        let sub_message = "Add EPUB files to your library";
        let sub_width = ThemeMetrics::text_width(sub_message.len());
        let sub_x = (display_width as i32 - sub_width) / 2;

        let sub_style = MonoTextStyle::new(&ascii::FONT_7X13, BinaryColor::On);
        Text::new(sub_message, Point::new(sub_x, center_y + 25), sub_style).draw(display)?;

        Ok(())
    }

    /// Render book items
    fn render_books<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
    ) -> Result<(), D::Error> {
        let display_width = display.bounding_box().size.width;
        let content_width = self.theme.metrics.content_width(display_width);
        let x = self.theme.metrics.side_padding as i32;
        let start_y = self.theme.metrics.header_height as i32;
        let item_height = self.theme.metrics.list_item_height;

        for (i, &book_idx) in self
            .filtered_books
            .iter()
            .skip(self.scroll_offset)
            .take(self.visible_count)
            .enumerate()
        {
            let list_index = self.scroll_offset + i;
            let y = start_y + (i as i32) * item_height as i32;
            let book = &self.books[book_idx];
            let is_selected = list_index == self.selected_index;

            self.render_book_item(display, book, x, y, content_width, item_height, is_selected)?;
        }

        // Render scroll indicator if needed
        if self.filtered_books.len() > self.visible_count {
            self.render_scroll_indicator(display)?;
        }

        Ok(())
    }

    /// Render a single book item
    #[allow(clippy::too_many_arguments)]
    fn render_book_item<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
        book: &BookInfo,
        x: i32,
        y: i32,
        width: u32,
        item_height: u32,
        is_selected: bool,
    ) -> Result<(), D::Error> {
        // Background
        let bg_color = if is_selected {
            BinaryColor::On
        } else {
            BinaryColor::Off
        };
        Rectangle::new(Point::new(x, y), Size::new(width, item_height))
            .into_styled(PrimitiveStyle::with_fill(bg_color))
            .draw(display)?;

        // Cover placeholder (rectangle)
        let cover_padding: u32 = 8;
        let cover_x = x + cover_padding as i32;
        let cover_y = y + cover_padding as i32;
        let cover_height = item_height - cover_padding * 2;

        // Draw filled rectangle as cover placeholder
        Rectangle::new(
            Point::new(cover_x, cover_y),
            Size::new(Self::COVER_WIDTH, cover_height),
        )
        .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
        .draw(display)?;

        // Text color based on selection
        let text_color = if is_selected {
            BinaryColor::Off
        } else {
            BinaryColor::On
        };

        let title_style = MonoTextStyle::new(&ascii::FONT_7X13_BOLD, text_color);
        let author_style = MonoTextStyle::new(&ascii::FONT_7X13, text_color);

        // Title
        let title_x = x + Self::COVER_WIDTH as i32 + (cover_padding * 2) as i32;
        let title_y = y + 20;
        let max_title_chars = ((width as i32 - title_x - x) / FONT_CHAR_WIDTH) as usize;
        let title = book.display_title(max_title_chars);
        Text::new(title, Point::new(title_x, title_y), title_style).draw(display)?;

        // Author
        let author_y = y + 40;
        let author = if book.author.len() > 25 {
            format!("{}...", &book.author[..22])
        } else {
            book.author.clone()
        };
        Text::new(&author, Point::new(title_x, author_y), author_style).draw(display)?;

        // Progress bar
        self.render_progress_bar(display, book.progress_percent, x, y, width, text_color)?;

        // Bottom separator
        let sep_y = y + item_height as i32 - 1;
        Rectangle::new(Point::new(x, sep_y), Size::new(width, 1))
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(display)?;

        Ok(())
    }

    /// Render progress bar
    fn render_progress_bar<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
        progress: u8,
        x: i32,
        y: i32,
        width: u32,
        text_color: BinaryColor,
    ) -> Result<(), D::Error> {
        let bar_y = y + 52;
        let bar_width = 100;
        let bar_height = 6;
        let bar_x = x + width as i32 - bar_width as i32 - self.theme.metrics.side_padding as i32;

        // Background bar
        Rectangle::new(Point::new(bar_x, bar_y), Size::new(bar_width, bar_height))
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
            .draw(display)?;

        // Progress fill
        let fill_width = (bar_width * progress as u32 / 100).min(bar_width - 2);
        if fill_width > 0 {
            Rectangle::new(
                Point::new(bar_x + 1, bar_y + 1),
                Size::new(fill_width, bar_height - 2),
            )
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(display)?;
        }

        // Percentage text
        let percent_label = format!("{}%", progress);
        let percent_x = bar_x - 35;
        let percent_style = MonoTextStyle::new(&ascii::FONT_6X9, text_color);
        Text::new(
            &percent_label,
            Point::new(percent_x, bar_y + 6),
            percent_style,
        )
        .draw(display)?;

        Ok(())
    }

    /// Render scroll indicator
    fn render_scroll_indicator<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
    ) -> Result<(), D::Error> {
        let display_width = display.bounding_box().size.width;
        let display_height = display.bounding_box().size.height;
        let indicator_y = display_height as i32 - 20;
        let indicator_width = 60;
        let indicator_x = (display_width as i32 - indicator_width) / 2;

        // Draw scroll bar background
        Rectangle::new(
            Point::new(indicator_x, indicator_y),
            Size::new(indicator_width as u32, 4),
        )
        .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
        .draw(display)?;

        // Calculate thumb position
        let total_items = self.filtered_books.len();
        let thumb_width = (self.visible_count * indicator_width as usize / total_items).max(10);
        let max_offset = total_items.saturating_sub(self.visible_count);
        let thumb_pos = if max_offset > 0 {
            (self.scroll_offset * (indicator_width as usize - thumb_width) / max_offset) as i32
        } else {
            0
        };

        Rectangle::new(
            Point::new(indicator_x + thumb_pos, indicator_y),
            Size::new(thumb_width as u32, 4),
        )
        .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
        .draw(display)?;

        Ok(())
    }

    /// Render context menu
    fn render_context_menu<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
    ) -> Result<(), D::Error> {
        let book = self.selected_book().cloned();

        if let Some(book) = book {
            let title = format!("Options: {}", book.title);
            let mut modal = Modal::new(&title, "Select an action")
                .with_button("Open")
                .with_button("Mark Unread")
                .with_button("Delete")
                .with_button("Cancel");
            modal.selected_button = self.context_menu_index;
            modal.render(display, &self.theme)?;
        }

        Ok(())
    }
}

impl Activity for LibraryActivity {
    fn on_enter(&mut self) {
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.show_context_menu = false;
        self.show_toast = false;
    }

    fn on_exit(&mut self) {
        // Cleanup if needed
    }

    fn handle_input(&mut self, event: InputEvent) -> ActivityResult {
        if self.show_context_menu {
            return self.handle_context_menu_input(event);
        }

        match event {
            InputEvent::Press(Button::Back) => ActivityResult::NavigateBack,
            InputEvent::Press(Button::VolumeUp) => {
                self.select_prev();
                ActivityResult::Consumed
            }
            InputEvent::Press(Button::VolumeDown) => {
                self.select_next();
                ActivityResult::Consumed
            }
            InputEvent::Press(Button::Left) => {
                self.cycle_sort();
                ActivityResult::Consumed
            }
            InputEvent::Press(Button::Right) | InputEvent::Press(Button::Confirm) => {
                if let Some(book) = self.selected_book() {
                    self.show_toast(format!("Opening: {}", book.title));
                    ActivityResult::Consumed
                } else {
                    ActivityResult::Consumed
                }
            }
            _ => ActivityResult::Ignored,
        }
    }

    fn render<D: DrawTarget<Color = BinaryColor>>(&self, display: &mut D) -> Result<(), D::Error> {
        // Clear background
        Rectangle::new(
            Point::new(0, 0),
            Size::new(
                display.bounding_box().size.width,
                display.bounding_box().size.height,
            ),
        )
        .into_styled(PrimitiveStyle::with_fill(BinaryColor::Off))
        .draw(display)?;

        // Header
        self.render_header(display)?;

        // Book list
        self.render_book_list(display)?;

        // Toast notification
        if self.show_toast {
            let display_width = display.bounding_box().size.width;
            let display_height = display.bounding_box().size.height;
            let toast =
                crate::ui::Toast::bottom_center(&self.toast_message, display_width, display_height);
            toast.render(display)?;
        }

        // Context menu modal
        if self.show_context_menu {
            self.render_context_menu(display)?;
        }

        Ok(())
    }
}

impl Default for LibraryActivity {
    fn default() -> Self {
        Self::new()
    }
}

/// Create mock books for testing
pub fn create_mock_books() -> Vec<BookInfo> {
    vec![
        BookInfo::new(
            "The Great Gatsby",
            "F. Scott Fitzgerald",
            "/books/gatsby.epub",
            75,
            Some(1704067200), // 2024-01-01
        ),
        BookInfo::new(
            "1984",
            "George Orwell",
            "/books/1984.epub",
            30,
            Some(1703980800), // 2023-12-31
        ),
        BookInfo::new(
            "Pride and Prejudice",
            "Jane Austen",
            "/books/pride.epub",
            100,
            Some(1703894400), // 2023-12-30
        ),
        BookInfo::new(
            "To Kill a Mockingbird",
            "Harper Lee",
            "/books/mockingbird.epub",
            0,
            None,
        ),
        BookInfo::new(
            "The Catcher in the Rye",
            "J.D. Salinger",
            "/books/catcher.epub",
            45,
            Some(1703808000), // 2023-12-29
        ),
        BookInfo::new(
            "Moby Dick",
            "Herman Melville",
            "/books/moby.epub",
            12,
            Some(1703721600), // 2023-12-28
        ),
        BookInfo::new(
            "War and Peace",
            "Leo Tolstoy",
            "/books/war_and_peace.epub",
            8,
            Some(1703635200), // 2023-12-27
        ),
        BookInfo::new(
            "The Hobbit",
            "J.R.R. Tolkien",
            "/books/hobbit.epub",
            100,
            Some(1703548800), // 2023-12-26
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn book_info_creation() {
        let book = BookInfo::new(
            "Test Title",
            "Test Author",
            "/path/to/book.epub",
            50,
            Some(1234567890),
        );

        assert_eq!(book.title, "Test Title");
        assert_eq!(book.author, "Test Author");
        assert_eq!(book.path, "/path/to/book.epub");
        assert_eq!(book.progress_percent, 50);
        assert_eq!(book.last_read, Some(1234567890));
    }

    #[test]
    fn book_info_progress_clamped() {
        let book = BookInfo::new("Test", "Author", "/path", 150, None);
        assert_eq!(book.progress_percent, 100);
    }

    #[test]
    fn sort_order_cycling() {
        let mut order = SortOrder::Title;
        assert_eq!(order.label(), "Title");

        order = order.next();
        assert_eq!(order, SortOrder::Author);

        order = order.next();
        assert_eq!(order, SortOrder::Recent);

        order = order.next();
        assert_eq!(order, SortOrder::Title);
    }

    #[test]
    fn library_activity_new() {
        let activity = LibraryActivity::new();
        assert!(activity.is_empty());
        assert_eq!(activity.book_count(), 0);
    }

    #[test]
    fn library_activity_with_books() {
        let books = create_mock_books();
        let activity = LibraryActivity::with_books(books.clone());

        assert_eq!(activity.book_count(), 8);
        assert!(!activity.is_empty());
    }

    #[test]
    fn library_activity_with_mock_books() {
        let activity = LibraryActivity::with_mock_books();
        assert_eq!(activity.book_count(), 8);
    }

    #[test]
    fn library_activity_selection() {
        let activity = LibraryActivity::with_mock_books();

        // First book should be selected by default
        let selected = activity.selected_book();
        assert!(selected.is_some());
    }

    #[test]
    fn library_activity_navigation() {
        let mut activity = LibraryActivity::with_mock_books();
        activity.on_enter();

        assert_eq!(activity.selected_index, 0);

        // Navigate down
        activity.select_next();
        assert_eq!(activity.selected_index, 1);

        activity.select_next();
        assert_eq!(activity.selected_index, 2);

        // Navigate up
        activity.select_prev();
        assert_eq!(activity.selected_index, 1);
    }

    #[test]
    fn library_activity_navigation_wraps() {
        let mut activity = LibraryActivity::with_mock_books();
        activity.on_enter();

        // Wrap backward from 0
        activity.select_prev();
        assert_eq!(activity.selected_index, 7); // Last of 8 books

        // Wrap forward from last
        activity.select_next();
        assert_eq!(activity.selected_index, 0);
    }

    #[test]
    fn library_activity_sort_by_title() {
        let mut activity = LibraryActivity::with_mock_books();
        activity.sort_order = SortOrder::Title;
        activity.apply_sort();

        let first = activity.selected_book().unwrap();
        assert_eq!(first.title, "1984"); // Alphabetically first
    }

    #[test]
    fn library_activity_sort_by_author() {
        let mut activity = LibraryActivity::with_mock_books();
        activity.sort_order = SortOrder::Author;
        activity.apply_sort();

        let first = activity.selected_book().unwrap();
        assert_eq!(first.author, "F. Scott Fitzgerald"); // Alphabetically first
    }

    #[test]
    fn library_activity_sort_by_recent() {
        let mut activity = LibraryActivity::with_mock_books();
        activity.sort_order = SortOrder::Recent;
        activity.apply_sort();

        let first = activity.selected_book().unwrap();
        assert_eq!(first.title, "The Great Gatsby"); // Most recent
    }

    #[test]
    fn library_activity_cycle_sort() {
        let mut activity = LibraryActivity::with_mock_books();
        activity.on_enter();

        assert_eq!(activity.sort_order, SortOrder::Title);

        activity.cycle_sort();
        assert_eq!(activity.sort_order, SortOrder::Author);

        activity.cycle_sort();
        assert_eq!(activity.sort_order, SortOrder::Recent);

        activity.cycle_sort();
        assert_eq!(activity.sort_order, SortOrder::Title);
    }

    #[test]
    fn library_activity_input_back() {
        let mut activity = LibraryActivity::with_mock_books();
        let result = activity.handle_input(InputEvent::Press(Button::Back));
        assert_eq!(result, ActivityResult::NavigateBack);
    }

    #[test]
    fn library_activity_input_navigation() {
        let mut activity = LibraryActivity::with_mock_books();
        activity.on_enter();

        let result = activity.handle_input(InputEvent::Press(Button::VolumeDown));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(activity.selected_index, 1);

        let result = activity.handle_input(InputEvent::Press(Button::VolumeUp));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(activity.selected_index, 0);
    }

    #[test]
    fn library_activity_input_volume_buttons() {
        let mut activity = LibraryActivity::with_mock_books();
        activity.on_enter();

        let result = activity.handle_input(InputEvent::Press(Button::VolumeDown));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(activity.selected_index, 1);

        let result = activity.handle_input(InputEvent::Press(Button::VolumeUp));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(activity.selected_index, 0);
    }

    #[test]
    fn library_activity_input_sort() {
        let mut activity = LibraryActivity::with_mock_books();
        activity.on_enter();

        assert_eq!(activity.sort_order, SortOrder::Title);

        let result = activity.handle_input(InputEvent::Press(Button::Left));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(activity.sort_order, SortOrder::Author);
        assert!(activity.show_toast);
    }

    #[test]
    fn library_activity_context_menu() {
        let mut activity = LibraryActivity::with_mock_books();
        activity.on_enter();

        // Open context menu
        activity.open_context_menu();
        assert!(activity.show_context_menu);
        assert_eq!(activity.context_menu_index, 0);

        // Navigate within menu
        activity.context_menu_next();
        assert_eq!(activity.context_menu_index, 1);

        activity.context_menu_prev();
        assert_eq!(activity.context_menu_index, 0);

        // Close menu
        activity.close_context_menu();
        assert!(!activity.show_context_menu);
    }

    #[test]
    fn library_activity_context_menu_actions() {
        let mut activity = LibraryActivity::with_mock_books();
        activity.on_enter();

        activity.open_context_menu();

        assert_eq!(activity.current_action(), BookAction::Open);

        activity.context_menu_next();
        assert_eq!(activity.current_action(), BookAction::MarkUnread);

        activity.context_menu_next();
        assert_eq!(activity.current_action(), BookAction::Delete);

        activity.context_menu_next();
        assert_eq!(activity.current_action(), BookAction::Cancel);
    }

    #[test]
    fn library_activity_mark_unread() {
        let mut activity = LibraryActivity::with_mock_books();
        activity.on_enter();

        // Find a book with progress
        let first = activity.selected_book().unwrap();
        assert!(first.progress_percent > 0);

        // Mark as unread
        activity.execute_action(BookAction::MarkUnread);

        let first = activity.selected_book().unwrap();
        assert_eq!(first.progress_percent, 0);
    }

    #[test]
    fn library_activity_delete_book() {
        let mut activity = LibraryActivity::with_mock_books();
        activity.on_enter();

        let initial_count = activity.book_count();

        // Delete first book
        activity.execute_action(BookAction::Delete);

        assert_eq!(activity.book_count(), initial_count - 1);
    }

    #[test]
    fn library_activity_toast() {
        let mut activity = LibraryActivity::new();

        activity.show_toast("Test message");
        assert!(activity.show_toast);
        assert_eq!(activity.toast_message, "Test message");
        assert_eq!(
            activity.toast_frames_remaining,
            LibraryActivity::TOAST_DURATION
        );

        // Update toast
        activity.update();
        assert_eq!(
            activity.toast_frames_remaining,
            LibraryActivity::TOAST_DURATION - 1
        );

        // Simulate full duration
        for _ in 0..LibraryActivity::TOAST_DURATION {
            activity.update();
        }

        assert!(!activity.show_toast);
    }

    #[test]
    fn library_activity_render() {
        let mut activity = LibraryActivity::with_mock_books();
        activity.on_enter();

        let mut display = crate::test_display::TestDisplay::default_size();
        let result = activity.render(&mut display);
        assert!(result.is_ok());
    }

    #[test]
    fn library_activity_render_empty() {
        let mut activity = LibraryActivity::new();
        activity.on_enter();

        let mut display = crate::test_display::TestDisplay::default_size();
        let result = activity.render(&mut display);
        assert!(result.is_ok());
    }

    #[test]
    fn library_activity_render_with_context_menu() {
        let mut activity = LibraryActivity::with_mock_books();
        activity.on_enter();
        activity.open_context_menu();

        let mut display = crate::test_display::TestDisplay::default_size();
        let result = activity.render(&mut display);
        assert!(result.is_ok());
    }

    #[test]
    fn library_activity_scroll_visibility() {
        let mut activity = LibraryActivity::with_mock_books();
        activity.visible_count = 3; // Small for testing
        activity.on_enter();

        // Select beyond visible area
        activity.selected_index = 5;
        activity.ensure_visible();

        // Scroll offset should have adjusted
        assert!(activity.scroll_offset > 0);
    }

    #[test]
    fn mock_books_created() {
        let books = create_mock_books();
        assert_eq!(books.len(), 8);

        // Verify variety of progress values
        let progresses: Vec<u8> = books.iter().map(|b| b.progress_percent).collect();
        assert!(progresses.contains(&0));
        assert!(progresses.contains(&100));
        assert!(progresses.contains(&50) || progresses.contains(&45) || progresses.contains(&75));
    }

    #[test]
    fn book_info_display_title() {
        let book = BookInfo::new(
            "A Very Long Title That Needs Truncating",
            "Author",
            "/path",
            0,
            None,
        );

        let short = book.display_title(10);
        assert_eq!(short.len(), 10);

        let exact = book.display_title(5);
        assert_eq!(exact, "A Ver");
    }

    #[test]
    fn context_menu_input_handling() {
        let mut activity = LibraryActivity::with_mock_books();
        activity.on_enter();
        activity.open_context_menu();

        // Navigate with Right
        let result = activity.handle_input(InputEvent::Press(Button::Right));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(activity.context_menu_index, 1);

        // Navigate with VolumeDown
        let result = activity.handle_input(InputEvent::Press(Button::VolumeDown));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(activity.context_menu_index, 2);

        // Navigate back with VolumeUp
        let result = activity.handle_input(InputEvent::Press(Button::VolumeUp));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(activity.context_menu_index, 1);

        // Cancel with Back
        let result = activity.handle_input(InputEvent::Press(Button::Back));
        assert_eq!(result, ActivityResult::Consumed);
        assert!(!activity.show_context_menu);
    }

    #[test]
    fn context_menu_confirm_action() {
        let mut activity = LibraryActivity::with_mock_books();
        activity.on_enter();
        activity.open_context_menu();

        // Confirm should open the book
        let result = activity.handle_input(InputEvent::Press(Button::Confirm));
        assert_eq!(result, ActivityResult::Consumed);
        assert!(!activity.show_context_menu);
        assert!(activity.show_toast);
    }
}
