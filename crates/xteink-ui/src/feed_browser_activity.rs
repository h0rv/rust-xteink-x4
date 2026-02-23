//! Feed Browser Activity for Xteink X4 e-reader.
//!
//! Browse and download ebooks from OPDS (Open Publication Distribution System) catalogs.
//! Supports preloaded sources like Project Gutenberg, Standard Ebooks, and Feedbooks.

extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};

use embedded_graphics::{
    mono_font::MonoTextStyle,
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    text::Text,
};

use crate::feed::{all_preloaded_sources, FeedType, OpdsCatalog};
use crate::input::{Button, InputEvent};
use crate::ui::theme::{
    layout, ui_font_body, ui_font_body_char_width, ui_font_small, ui_font_title, ui_text,
};
use crate::ui::{Activity, ActivityRefreshMode, ActivityResult, Theme};
use crate::DISPLAY_HEIGHT;

fn truncate_text(text: &str, max_px: i32, _size: Option<u32>) -> String {
    let char_width = ui_font_body_char_width();
    let max_chars = (max_px / char_width).max(0) as usize;
    if text.chars().count() <= max_chars {
        text.to_string()
    } else {
        let truncated: String = text.chars().take(max_chars.saturating_sub(3)).collect();
        format!("{}...", truncated)
    }
}

fn wrap_text(text: &str, max_px: i32, _size: Option<u32>) -> String {
    let char_width = ui_font_body_char_width();
    let max_chars = (max_px / char_width).max(1) as usize;
    let mut result = String::new();
    let mut current_len = 0;
    for word in text.split_whitespace() {
        let word_len = word.chars().count();
        if current_len + word_len + 1 > max_chars {
            if current_len > 0 {
                result.push('\n');
            }
            result.push_str(word);
            current_len = word_len;
        } else {
            if current_len > 0 {
                result.push(' ');
            }
            result.push_str(word);
            current_len += word_len + 1;
        }
    }
    result
}

#[derive(Debug, Clone, PartialEq)]
pub enum BrowserState {
    SourceList,
    Loading,
    BrowsingCatalog,
    BookDetail,
    Downloading(f32),
    Error(String),
}

#[derive(Debug, Clone)]
pub struct FeedBrowserActivity {
    state: BrowserState,
    selected_index: usize,
    scroll_offset: usize,
    current_catalog: Option<OpdsCatalog>,
    current_source_index: Option<usize>,
    pending_fetch_url: Option<String>,
    pending_download_url: Option<String>,
    pending_download_title: Option<String>,
    theme: Theme,
    visible_count: usize,
    status_message: Option<String>,
    sources: Vec<(&'static str, &'static str, FeedType)>,
}

impl FeedBrowserActivity {
    pub fn new() -> Self {
        let theme = Theme::default();
        let visible_count = theme.metrics.visible_items(DISPLAY_HEIGHT);

        Self {
            state: BrowserState::SourceList,
            selected_index: 0,
            scroll_offset: 0,
            current_catalog: None,
            current_source_index: None,
            pending_fetch_url: None,
            pending_download_url: None,
            pending_download_title: None,
            theme,
            visible_count,
            status_message: None,
            sources: all_preloaded_sources(),
        }
    }

    pub fn state(&self) -> &BrowserState {
        &self.state
    }

    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    pub fn current_catalog(&self) -> Option<&OpdsCatalog> {
        self.current_catalog.as_ref()
    }

    pub fn set_catalog(&mut self, catalog: OpdsCatalog) {
        self.current_catalog = Some(catalog);
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.state = BrowserState::BrowsingCatalog;
    }

    pub fn set_loading(&mut self) {
        self.state = BrowserState::Loading;
    }

    pub fn set_error(&mut self, message: String) {
        self.state = BrowserState::Error(message);
    }

    pub fn set_download_progress(&mut self, progress: f32) {
        self.state = BrowserState::Downloading(progress.clamp(0.0, 1.0));
    }

    pub fn complete_download(&mut self) {
        self.state = BrowserState::BrowsingCatalog;
        self.status_message = Some("Download complete".to_string());
    }

    pub fn take_fetch_request(&mut self) -> Option<String> {
        self.pending_fetch_url.take()
    }

    pub fn take_download_request(&mut self) -> Option<String> {
        self.pending_download_url.take()
    }

    pub fn status_message(&self) -> Option<&str> {
        self.status_message.as_deref()
    }

    pub fn clear_status_message(&mut self) {
        self.status_message = None;
    }

    fn source_count(&self) -> usize {
        self.sources.len()
    }

    fn entry_count(&self) -> usize {
        self.current_catalog
            .as_ref()
            .map(|c| c.entries.len())
            .unwrap_or(0)
    }

    fn item_count(&self) -> usize {
        match self.state {
            BrowserState::SourceList => self.source_count(),
            BrowserState::BrowsingCatalog | BrowserState::BookDetail => self.entry_count(),
            BrowserState::Loading | BrowserState::Downloading(_) | BrowserState::Error(_) => 0,
        }
    }

    fn select_next(&mut self) {
        let count = self.item_count();
        if count == 0 {
            return;
        }
        self.selected_index = (self.selected_index + 1) % count;
        self.ensure_visible();
    }

    fn select_prev(&mut self) {
        let count = self.item_count();
        if count == 0 {
            return;
        }
        self.selected_index = if self.selected_index == 0 {
            count - 1
        } else {
            self.selected_index - 1
        };
        self.ensure_visible();
    }

    fn ensure_visible(&mut self) {
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        } else if self.selected_index >= self.scroll_offset + self.visible_count {
            self.scroll_offset = self.selected_index.saturating_sub(self.visible_count - 1);
        }
    }

    fn handle_source_list_input(&mut self, event: InputEvent) -> ActivityResult {
        match event {
            InputEvent::Press(Button::Down) | InputEvent::Press(Button::Aux2) => {
                self.select_next();
                ActivityResult::Consumed
            }
            InputEvent::Press(Button::Up) | InputEvent::Press(Button::Aux1) => {
                self.select_prev();
                ActivityResult::Consumed
            }
            InputEvent::Press(Button::Confirm) | InputEvent::Press(Button::Right) => {
                if let Some((_, url, _)) = self.sources.get(self.selected_index) {
                    self.pending_fetch_url = Some(url.to_string());
                    self.current_source_index = Some(self.selected_index);
                    self.state = BrowserState::Loading;
                }
                ActivityResult::Consumed
            }
            InputEvent::Press(Button::Back) => ActivityResult::NavigateBack,
            _ => ActivityResult::Ignored,
        }
    }

    fn handle_loading_input(&mut self, event: InputEvent) -> ActivityResult {
        match event {
            InputEvent::Press(Button::Back) => {
                self.state = BrowserState::SourceList;
                self.current_source_index = None;
                ActivityResult::Consumed
            }
            _ => ActivityResult::Ignored,
        }
    }

    fn handle_error_input(&mut self, event: InputEvent) -> ActivityResult {
        match event {
            InputEvent::Press(Button::Confirm) | InputEvent::Press(Button::Back) => {
                self.state = BrowserState::SourceList;
                ActivityResult::Consumed
            }
            _ => ActivityResult::Ignored,
        }
    }

    fn handle_catalog_input(&mut self, event: InputEvent) -> ActivityResult {
        match event {
            InputEvent::Press(Button::Down) | InputEvent::Press(Button::Aux2) => {
                self.select_next();
                ActivityResult::Consumed
            }
            InputEvent::Press(Button::Up) | InputEvent::Press(Button::Aux1) => {
                self.select_prev();
                ActivityResult::Consumed
            }
            InputEvent::Press(Button::Confirm) | InputEvent::Press(Button::Right) => {
                if self
                    .current_catalog
                    .as_ref()
                    .and_then(|c| c.entries.get(self.selected_index))
                    .is_some()
                {
                    self.state = BrowserState::BookDetail;
                }
                ActivityResult::Consumed
            }
            InputEvent::Press(Button::Back) | InputEvent::Press(Button::Left) => {
                self.current_catalog = None;
                self.selected_index = 0;
                self.scroll_offset = 0;
                self.state = BrowserState::SourceList;
                ActivityResult::Consumed
            }
            _ => ActivityResult::Ignored,
        }
    }

    fn handle_detail_input(&mut self, event: InputEvent) -> ActivityResult {
        let Some(catalog) = &self.current_catalog else {
            self.state = BrowserState::SourceList;
            return ActivityResult::Consumed;
        };
        let Some(entry) = catalog.entries.get(self.selected_index) else {
            self.state = BrowserState::BrowsingCatalog;
            return ActivityResult::Consumed;
        };

        match event {
            InputEvent::Press(Button::Confirm) | InputEvent::Press(Button::Right) => {
                if let Some(url) = &entry.download_url {
                    self.pending_download_url = Some(url.clone());
                    self.pending_download_title = Some(entry.title.clone());
                    self.state = BrowserState::Downloading(0.0);
                } else {
                    self.status_message = Some("No download available".to_string());
                }
                ActivityResult::Consumed
            }
            InputEvent::Press(Button::Back) | InputEvent::Press(Button::Left) => {
                self.state = BrowserState::BrowsingCatalog;
                ActivityResult::Consumed
            }
            _ => ActivityResult::Ignored,
        }
    }

    fn handle_downloading_input(&mut self, event: InputEvent) -> ActivityResult {
        match event {
            InputEvent::Press(Button::Confirm) | InputEvent::Press(Button::Back) => {
                self.state = BrowserState::BrowsingCatalog;
                self.status_message = Some("Download cancelled".to_string());
                ActivityResult::Consumed
            }
            _ => ActivityResult::Ignored,
        }
    }

    fn render_header<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
    ) -> Result<(), D::Error> {
        let title = match &self.state {
            BrowserState::SourceList => "Online Catalogs",
            BrowserState::Loading => "Loading...",
            BrowserState::BrowsingCatalog => self
                .current_catalog
                .as_ref()
                .map(|c| c.title.as_str())
                .unwrap_or("Catalog"),
            BrowserState::BookDetail => "Book Details",
            BrowserState::Downloading(_) => "Downloading",
            BrowserState::Error(_) => "Error",
        };

        let header = crate::ui::Header::new(title);
        header.render(display, &self.theme)
    }

    fn render_source_list<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
    ) -> Result<(), D::Error> {
        let display_width = display.bounding_box().size.width;
        let content_width = self.theme.metrics.content_width(display_width);
        let x = self.theme.metrics.side_padding as i32;
        let start_y = self.theme.metrics.header_height as i32 + self.theme.metrics.spacing as i32;
        let item_height = self.theme.metrics.list_item_height;

        for (i, (name, _url, _feed_type)) in self
            .sources
            .iter()
            .skip(self.scroll_offset)
            .take(self.visible_count)
            .enumerate()
        {
            let list_index = self.scroll_offset + i;
            let y = start_y + (i as i32) * item_height as i32;
            let is_selected = list_index == self.selected_index;

            self.render_list_item(display, name, x, y, content_width, item_height, is_selected)?;
        }

        Ok(())
    }

    fn render_catalog<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
    ) -> Result<(), D::Error> {
        let Some(catalog) = &self.current_catalog else {
            return self.render_empty_state(display, "No catalog loaded");
        };

        if catalog.entries.is_empty() {
            return self.render_empty_state(display, "No books in catalog");
        }

        let display_width = display.bounding_box().size.width;
        let content_width = self.theme.metrics.content_width(display_width);
        let x = self.theme.metrics.side_padding as i32;
        let start_y = self.theme.metrics.header_height as i32 + self.theme.metrics.spacing as i32;
        let item_height = self.theme.metrics.list_item_height;

        for (i, entry) in catalog
            .entries
            .iter()
            .skip(self.scroll_offset)
            .take(self.visible_count)
            .enumerate()
        {
            let list_index = self.scroll_offset + i;
            let y = start_y + (i as i32) * item_height as i32;
            let is_selected = list_index == self.selected_index;

            let label = if let Some(author) = &entry.author {
                format!("{} - {}", entry.title, author)
            } else {
                entry.title.clone()
            };

            self.render_list_item(
                display,
                &label,
                x,
                y,
                content_width,
                item_height,
                is_selected,
            )?;
        }

        Ok(())
    }

    fn render_book_detail<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
    ) -> Result<(), D::Error> {
        let Some(catalog) = &self.current_catalog else {
            return self.render_empty_state(display, "No book selected");
        };
        let Some(entry) = catalog.entries.get(self.selected_index) else {
            return self.render_empty_state(display, "Invalid selection");
        };

        let display_width = display.bounding_box().size.width;
        let x = self.theme.metrics.side_padding as i32;
        let mut y = self.theme.metrics.header_height as i32 + self.theme.metrics.spacing as i32;
        let max_text_width = display_width as i32 - x * 2;

        let title_style = MonoTextStyle::new(ui_font_title(), BinaryColor::On);
        let body_style = MonoTextStyle::new(ui_font_body(), BinaryColor::On);
        let small_style = MonoTextStyle::new(ui_font_small(), BinaryColor::On);

        let truncated_title = truncate_text(&entry.title, max_text_width, Some(18));
        Text::new(&truncated_title, Point::new(x, y + 14), title_style).draw(display)?;
        y += 28;

        if let Some(author) = &entry.author {
            let author_text = format!("by {}", author);
            Text::new(&author_text, Point::new(x, y + 12), body_style).draw(display)?;
            y += 24;
        }

        y += self.theme.metrics.spacing as i32;

        if let Some(summary) = &entry.summary {
            let wrapped = wrap_text(summary, max_text_width, Some(14));
            for line in wrapped.lines().take(6) {
                Text::new(line, Point::new(x, y + 10), small_style).draw(display)?;
                y += 14;
            }
        }

        y += self.theme.metrics.spacing as i32;

        if let Some(size) = entry.size {
            let size_text = format!("Size: {} KB", size / 1024);
            Text::new(&size_text, Point::new(x, y + 10), small_style).draw(display)?;
            y += 14;
        }

        if let Some(format) = &entry.format {
            let format_text = format!("Format: {}", format);
            Text::new(&format_text, Point::new(x, y + 10), small_style).draw(display)?;
        }

        let footer_y = display.bounding_box().size.height as i32 - 20;
        let hint = if entry.download_url.is_some() {
            "Confirm: Download | Back: Return"
        } else {
            "Back: Return"
        };
        Text::new(hint, Point::new(x, footer_y), small_style).draw(display)?;

        Ok(())
    }

    fn render_downloading<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
        progress: f32,
    ) -> Result<(), D::Error> {
        let display_width = display.bounding_box().size.width;
        let display_height = display.bounding_box().size.height;
        let center_y = display_height as i32 / 2;

        let title_style = MonoTextStyle::new(ui_font_title(), BinaryColor::On);
        let body_style = MonoTextStyle::new(ui_font_body(), BinaryColor::On);

        let message = "Downloading...";
        let msg_width = ui_text::width(message, Some(18)) as i32;
        let x = (display_width as i32 - msg_width) / 2;
        Text::new(message, Point::new(x, center_y - 20), title_style).draw(display)?;

        if let Some(title) = &self.pending_download_title {
            let truncated = truncate_text(title, display_width as i32 - 40, Some(14));
            let title_width = ui_text::width(&truncated, Some(14)) as i32;
            let title_x = (display_width as i32 - title_width) / 2;
            Text::new(&truncated, Point::new(title_x, center_y + 5), body_style).draw(display)?;
        }

        let bar_width = (display_width as f32 * 0.7) as u32;
        let bar_height = 12u32;
        let bar_x = ((display_width - bar_width) / 2) as i32;
        let bar_y = center_y + 30;

        Rectangle::new(Point::new(bar_x, bar_y), Size::new(bar_width, bar_height))
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
            .draw(display)?;

        let fill_width = ((bar_width as f32) * progress) as u32;
        if fill_width > 0 {
            Rectangle::new(Point::new(bar_x, bar_y), Size::new(fill_width, bar_height))
                .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
                .draw(display)?;
        }

        let percent_text = format!("{:.0}%", progress * 100.0);
        let percent_width = ui_text::width(&percent_text, Some(14)) as i32;
        let percent_x = (display_width as i32 - percent_width) / 2;
        Text::new(
            &percent_text,
            Point::new(percent_x, bar_y + bar_height as i32 + 14),
            body_style,
        )
        .draw(display)?;

        let hint = "Confirm: Cancel";
        let hint_width = ui_text::width(hint, Some(12)) as i32;
        let hint_x = (display_width as i32 - hint_width) / 2;
        Text::new(
            hint,
            Point::new(hint_x, display_height as i32 - 20),
            MonoTextStyle::new(ui_font_small(), BinaryColor::On),
        )
        .draw(display)?;

        Ok(())
    }

    fn render_loading<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
    ) -> Result<(), D::Error> {
        let display_width = display.bounding_box().size.width;
        let display_height = display.bounding_box().size.height;
        let center_y = display_height as i32 / 2;

        let body_style = MonoTextStyle::new(ui_font_body(), BinaryColor::On);
        let small_style = MonoTextStyle::new(ui_font_small(), BinaryColor::On);

        let source_name = self
            .current_source_index
            .and_then(|idx| self.sources.get(idx))
            .map(|(name, _, _)| *name)
            .unwrap_or("Catalog");

        let message = format!("Fetching {}...", source_name);
        let msg_width = ui_text::width(&message, Some(14)) as i32;
        let x = (display_width as i32 - msg_width) / 2;
        Text::new(&message, Point::new(x, center_y), body_style).draw(display)?;

        let hint = "Back: Cancel";
        let hint_width = ui_text::width(hint, Some(12)) as i32;
        let hint_x = (display_width as i32 - hint_width) / 2;
        Text::new(hint, Point::new(hint_x, center_y + 30), small_style).draw(display)?;

        Ok(())
    }

    fn render_error<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
        message: &str,
    ) -> Result<(), D::Error> {
        let display_width = display.bounding_box().size.width;
        let display_height = display.bounding_box().size.height;
        let center_y = display_height as i32 / 2;

        let body_style = MonoTextStyle::new(ui_font_body(), BinaryColor::On);
        let small_style = MonoTextStyle::new(ui_font_small(), BinaryColor::On);

        let msg_width = ui_text::width(message, Some(14)) as i32;
        let x = (display_width as i32 - msg_width) / 2;
        Text::new(message, Point::new(x, center_y), body_style).draw(display)?;

        let hint = "Press any key to continue";
        let hint_width = ui_text::width(hint, Some(12)) as i32;
        let hint_x = (display_width as i32 - hint_width) / 2;
        Text::new(hint, Point::new(hint_x, center_y + 30), small_style).draw(display)?;

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn render_list_item<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        is_selected: bool,
    ) -> Result<(), D::Error> {
        if is_selected {
            Rectangle::new(Point::new(x, y), Size::new(width, height))
                .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
                .draw(display)?;
        }

        let text_color = if is_selected {
            BinaryColor::Off
        } else {
            BinaryColor::On
        };

        let truncated = truncate_text(text, width as i32 - 8, Some(14));
        let style = MonoTextStyle::new(ui_font_body(), text_color);
        Text::new(
            &truncated,
            Point::new(x + 4, y + ui_text::center_y(height, Some(14))),
            style,
        )
        .draw(display)?;

        if !is_selected {
            Rectangle::new(Point::new(x, y + height as i32 - 1), Size::new(width, 1))
                .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
                .draw(display)?;
        }

        Ok(())
    }

    fn render_empty_state<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
        message: &str,
    ) -> Result<(), D::Error> {
        let display_width = display.bounding_box().size.width;
        let display_height = display.bounding_box().size.height;
        let center_y = display_height as i32 / 2;

        let msg_width = ui_text::width(message, Some(14)) as i32;
        let x = (display_width as i32 - msg_width) / 2;

        let style = MonoTextStyle::new(ui_font_body(), BinaryColor::On);
        Text::new(message, Point::new(x, center_y), style).draw(display)?;

        Ok(())
    }

    fn render_status_message<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
    ) -> Result<(), D::Error> {
        let Some(message) = &self.status_message else {
            return Ok(());
        };

        let display_width = display.bounding_box().size.width;
        let display_height = display.bounding_box().size.height;
        let y = display_height as i32 - 18;

        Rectangle::new(Point::new(0, y), Size::new(display_width, 18))
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(display)?;

        let style = MonoTextStyle::new(ui_font_small(), BinaryColor::Off);
        Text::new(message, Point::new(layout::GAP_SM, y + 12), style).draw(display)?;

        Ok(())
    }
}

impl Activity for FeedBrowserActivity {
    fn on_enter(&mut self) {
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.status_message = None;
    }

    fn on_exit(&mut self) {
        self.status_message = None;
    }

    fn handle_input(&mut self, event: InputEvent) -> ActivityResult {
        let result = match &self.state {
            BrowserState::SourceList => self.handle_source_list_input(event),
            BrowserState::Loading => self.handle_loading_input(event),
            BrowserState::BrowsingCatalog => self.handle_catalog_input(event),
            BrowserState::BookDetail => self.handle_detail_input(event),
            BrowserState::Downloading(_) => self.handle_downloading_input(event),
            BrowserState::Error(_) => self.handle_error_input(event),
        };

        if result == ActivityResult::Consumed {
            self.status_message = None;
        }

        result
    }

    fn render<D: DrawTarget<Color = BinaryColor>>(&self, display: &mut D) -> Result<(), D::Error> {
        display.clear(BinaryColor::Off)?;

        self.render_header(display)?;

        match &self.state {
            BrowserState::SourceList => self.render_source_list(display)?,
            BrowserState::Loading => self.render_loading(display)?,
            BrowserState::BrowsingCatalog => self.render_catalog(display)?,
            BrowserState::BookDetail => self.render_book_detail(display)?,
            BrowserState::Downloading(progress) => self.render_downloading(display, *progress)?,
            BrowserState::Error(msg) => self.render_error(display, msg)?,
        }

        if self.status_message.is_some() && !matches!(self.state, BrowserState::Downloading(_)) {
            self.render_status_message(display)?;
        }

        Ok(())
    }

    fn refresh_mode(&self) -> ActivityRefreshMode {
        if matches!(self.state, BrowserState::Downloading(_)) {
            ActivityRefreshMode::Fast
        } else {
            ActivityRefreshMode::default()
        }
    }
}

impl Default for FeedBrowserActivity {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feed::OpdsEntry;

    fn mock_catalog() -> OpdsCatalog {
        OpdsCatalog {
            title: "Test Catalog".to_string(),
            subtitle: None,
            entries: vec![
                OpdsEntry {
                    id: "1".to_string(),
                    title: "Pride and Prejudice".to_string(),
                    author: Some("Jane Austen".to_string()),
                    summary: Some("A classic novel.".to_string()),
                    cover_url: None,
                    download_url: Some("https://example.com/pride.epub".to_string()),
                    format: Some("application/epub+zip".to_string()),
                    size: Some(500000),
                },
                OpdsEntry {
                    id: "2".to_string(),
                    title: "Frankenstein".to_string(),
                    author: Some("Mary Shelley".to_string()),
                    summary: Some("A Gothic novel.".to_string()),
                    cover_url: None,
                    download_url: Some("https://example.com/frankenstein.epub".to_string()),
                    format: Some("application/epub+zip".to_string()),
                    size: Some(400000),
                },
            ],
            links: vec![],
        }
    }

    #[test]
    fn feed_browser_new_starts_at_source_list() {
        let activity = FeedBrowserActivity::new();
        assert!(matches!(activity.state(), BrowserState::SourceList));
        assert_eq!(activity.selected_index(), 0);
    }

    #[test]
    fn feed_browser_navigates_sources() {
        let mut activity = FeedBrowserActivity::new();
        assert_eq!(activity.selected_index(), 0);

        activity.handle_input(InputEvent::Press(Button::Down));
        assert_eq!(activity.selected_index(), 1);

        activity.handle_input(InputEvent::Press(Button::Down));
        assert_eq!(activity.selected_index(), 2);

        activity.handle_input(InputEvent::Press(Button::Up));
        assert_eq!(activity.selected_index(), 1);
    }

    #[test]
    fn feed_browser_requests_fetch_on_select() {
        let mut activity = FeedBrowserActivity::new();

        let result = activity.handle_input(InputEvent::Press(Button::Confirm));
        assert_eq!(result, ActivityResult::Consumed);
        assert!(matches!(activity.state(), BrowserState::Loading));

        let fetch_url = activity.take_fetch_request();
        assert!(fetch_url.is_some());
        assert_eq!(fetch_url.unwrap(), "https://m.gutenberg.org/ebooks.opds/");
    }

    #[test]
    fn feed_browser_set_catalog_transitions_state() {
        let mut activity = FeedBrowserActivity::new();
        activity.set_catalog(mock_catalog());

        assert!(matches!(activity.state(), BrowserState::BrowsingCatalog));
        assert!(activity.current_catalog().is_some());
    }

    #[test]
    fn feed_browser_navigates_catalog_entries() {
        let mut activity = FeedBrowserActivity::new();
        activity.set_catalog(mock_catalog());

        assert_eq!(activity.selected_index(), 0);

        activity.handle_input(InputEvent::Press(Button::Down));
        assert_eq!(activity.selected_index(), 1);

        activity.handle_input(InputEvent::Press(Button::Up));
        assert_eq!(activity.selected_index(), 0);
    }

    #[test]
    fn feed_browser_shows_book_detail() {
        let mut activity = FeedBrowserActivity::new();
        activity.set_catalog(mock_catalog());

        let result = activity.handle_input(InputEvent::Press(Button::Confirm));
        assert_eq!(result, ActivityResult::Consumed);
        assert!(matches!(activity.state(), BrowserState::BookDetail));
    }

    #[test]
    fn feed_browser_requests_download() {
        let mut activity = FeedBrowserActivity::new();
        activity.set_catalog(mock_catalog());
        activity.handle_input(InputEvent::Press(Button::Confirm));

        let result = activity.handle_input(InputEvent::Press(Button::Confirm));
        assert_eq!(result, ActivityResult::Consumed);
        assert!(matches!(activity.state(), BrowserState::Downloading(0.0)));

        let download = activity.take_download_request();
        assert!(download.is_some());
        assert_eq!(download.unwrap(), "https://example.com/pride.epub");
    }

    #[test]
    fn feed_browser_back_from_detail_returns_to_catalog() {
        let mut activity = FeedBrowserActivity::new();
        activity.set_catalog(mock_catalog());
        activity.handle_input(InputEvent::Press(Button::Confirm));
        assert!(matches!(activity.state(), BrowserState::BookDetail));

        let result = activity.handle_input(InputEvent::Press(Button::Back));
        assert_eq!(result, ActivityResult::Consumed);
        assert!(matches!(activity.state(), BrowserState::BrowsingCatalog));
    }

    #[test]
    fn feed_browser_back_from_catalog_returns_to_sources() {
        let mut activity = FeedBrowserActivity::new();
        activity.set_catalog(mock_catalog());

        let result = activity.handle_input(InputEvent::Press(Button::Back));
        assert_eq!(result, ActivityResult::Consumed);
        assert!(matches!(activity.state(), BrowserState::SourceList));
        assert!(activity.current_catalog().is_none());
    }

    #[test]
    fn feed_browser_back_from_sources_navigates_back() {
        let mut activity = FeedBrowserActivity::new();

        let result = activity.handle_input(InputEvent::Press(Button::Back));
        assert_eq!(result, ActivityResult::NavigateBack);
    }

    #[test]
    fn feed_browser_cancel_download() {
        let mut activity = FeedBrowserActivity::new();
        activity.set_catalog(mock_catalog());
        activity.handle_input(InputEvent::Press(Button::Confirm));
        activity.handle_input(InputEvent::Press(Button::Confirm));
        assert!(matches!(activity.state(), BrowserState::Downloading(_)));

        let result = activity.handle_input(InputEvent::Press(Button::Back));
        assert_eq!(result, ActivityResult::Consumed);
        assert!(matches!(activity.state(), BrowserState::BrowsingCatalog));
    }

    #[test]
    fn feed_browser_loading_cancel() {
        let mut activity = FeedBrowserActivity::new();
        activity.handle_input(InputEvent::Press(Button::Confirm));
        assert!(matches!(activity.state(), BrowserState::Loading));

        let result = activity.handle_input(InputEvent::Press(Button::Back));
        assert_eq!(result, ActivityResult::Consumed);
        assert!(matches!(activity.state(), BrowserState::SourceList));
    }

    #[test]
    fn feed_browser_error_dismiss() {
        let mut activity = FeedBrowserActivity::new();
        activity.set_error("Network error".to_string());
        assert!(matches!(activity.state(), BrowserState::Error(_)));

        let result = activity.handle_input(InputEvent::Press(Button::Confirm));
        assert_eq!(result, ActivityResult::Consumed);
        assert!(matches!(activity.state(), BrowserState::SourceList));
    }

    #[test]
    fn feed_browser_render_smoke_test() {
        let activity = FeedBrowserActivity::new();
        let mut display = crate::test_display::TestDisplay::default_size();
        let result = activity.render(&mut display);
        assert!(result.is_ok());
    }

    #[test]
    fn feed_browser_render_catalog_smoke_test() {
        let mut activity = FeedBrowserActivity::new();
        activity.set_catalog(mock_catalog());

        let mut display = crate::test_display::TestDisplay::default_size();
        let result = activity.render(&mut display);
        assert!(result.is_ok());
    }

    #[test]
    fn feed_browser_render_detail_smoke_test() {
        let mut activity = FeedBrowserActivity::new();
        activity.set_catalog(mock_catalog());
        activity.handle_input(InputEvent::Press(Button::Confirm));

        let mut display = crate::test_display::TestDisplay::default_size();
        let result = activity.render(&mut display);
        assert!(result.is_ok());
    }

    #[test]
    fn feed_browser_render_downloading_smoke_test() {
        let mut activity = FeedBrowserActivity::new();
        activity.set_catalog(mock_catalog());
        activity.handle_input(InputEvent::Press(Button::Confirm));
        activity.handle_input(InputEvent::Press(Button::Confirm));

        let mut display = crate::test_display::TestDisplay::default_size();
        let result = activity.render(&mut display);
        assert!(result.is_ok());
    }

    #[test]
    fn feed_browser_render_loading_smoke_test() {
        let mut activity = FeedBrowserActivity::new();
        activity.handle_input(InputEvent::Press(Button::Confirm));

        let mut display = crate::test_display::TestDisplay::default_size();
        let result = activity.render(&mut display);
        assert!(result.is_ok());
    }

    #[test]
    fn feed_browser_render_error_smoke_test() {
        let mut activity = FeedBrowserActivity::new();
        activity.set_error("Test error".to_string());

        let mut display = crate::test_display::TestDisplay::default_size();
        let result = activity.render(&mut display);
        assert!(result.is_ok());
    }
}
