//! UI Components for e-ink displays.
//!
//! Components are designed with e-ink constraints in mind:
//! - High contrast (black/white only)
//! - No animations or gradients
//! - Clear focus states for accessibility
//! - Touch-friendly sizes

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use embedded_graphics::{
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
};

use crate::ui::theme::{ui_text, Theme};

/// Button component with focus state
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Button {
    pub label: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub focused: bool,
    pub enabled: bool,
}

impl Button {
    /// Create a new button with the given label and position
    pub fn new(label: impl Into<String>, x: i32, y: i32, width: u32) -> Self {
        Self {
            label: label.into(),
            x,
            y,
            width,
            focused: false,
            enabled: true,
        }
    }

    /// Set focus state
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Set enabled state
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Get button height from theme
    pub fn height(&self, theme: &Theme) -> u32 {
        theme.metrics.button_height
    }

    /// Get button bounds as rectangle
    pub fn bounds(&self, theme: &Theme) -> Rectangle {
        Rectangle::new(
            Point::new(self.x, self.y),
            Size::new(self.width, self.height(theme)),
        )
    }

    /// Check if point is inside button
    pub fn contains(&self, point: Point, theme: &Theme) -> bool {
        self.bounds(theme).contains(point)
    }

    /// Render the button to display
    pub fn render<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
        theme: &Theme,
    ) -> Result<(), D::Error> {
        let bounds = self.bounds(theme);

        // Background fill
        let bg_color = if self.focused {
            BinaryColor::On
        } else {
            BinaryColor::Off
        };

        let bg_style = PrimitiveStyle::with_fill(bg_color);
        bounds.into_styled(bg_style).draw(display)?;

        // Border (always draw, thicker when focused)
        let border_width = if self.focused { 2 } else { 1 };
        let border_color = if self.enabled {
            BinaryColor::On
        } else {
            BinaryColor::Off
        };

        let border_style = PrimitiveStyle::with_stroke(border_color, border_width);
        bounds.into_styled(border_style).draw(display)?;

        // Label text
        let text_color = if self.focused {
            BinaryColor::Off
        } else {
            BinaryColor::On
        };

        let text_width = ui_text::width(&self.label, Some(ui_text::DEFAULT_SIZE)) as i32;
        let text_x = self.x + (self.width as i32 - text_width) / 2;
        let text_y =
            self.y + ui_text::center_y(theme.metrics.button_height, Some(ui_text::DEFAULT_SIZE));
        ui_text::draw_colored(
            display,
            &self.label,
            text_x,
            text_y,
            Some(ui_text::DEFAULT_SIZE),
            text_color,
        )?;

        Ok(())
    }
}

/// List component with scrollable, selectable items
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct List {
    pub items: Vec<String>,
    pub selected_index: usize,
    pub scroll_offset: usize,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub visible_count: usize,
}

impl List {
    /// Create a new list
    pub fn new(items: Vec<String>, x: i32, y: i32, width: u32, visible_count: usize) -> Self {
        Self {
            items,
            selected_index: 0,
            scroll_offset: 0,
            x,
            y,
            width,
            visible_count,
        }
    }

    /// Move selection down
    pub fn select_next(&mut self) {
        if self.selected_index + 1 < self.items.len() {
            self.selected_index += 1;
            self.ensure_visible();
        }
    }

    /// Move selection up
    pub fn select_prev(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
            self.ensure_visible();
        }
    }

    /// Get currently selected item
    pub fn selected(&self) -> Option<&str> {
        self.items.get(self.selected_index).map(|s| s.as_str())
    }

    /// Ensure selected item is visible
    fn ensure_visible(&mut self) {
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        } else if self.selected_index >= self.scroll_offset + self.visible_count {
            self.scroll_offset = self.selected_index.saturating_sub(self.visible_count - 1);
        }
    }

    /// Get item height from theme
    fn item_height(&self, theme: &Theme) -> u32 {
        theme.metrics.list_item_height
    }

    /// Get list height
    pub fn height(&self, theme: &Theme) -> u32 {
        self.visible_count as u32 * self.item_height(theme)
    }

    /// Render the list
    pub fn render<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
        theme: &Theme,
    ) -> Result<(), D::Error> {
        let item_height = self.item_height(theme) as i32;

        for (i, item) in self
            .items
            .iter()
            .skip(self.scroll_offset)
            .take(self.visible_count)
            .enumerate()
        {
            let index = self.scroll_offset + i;
            let y = self.y + (i as i32) * item_height;

            let is_selected = index == self.selected_index;

            // Background - only fill if selected
            if is_selected {
                Rectangle::new(
                    Point::new(self.x, y),
                    Size::new(self.width, item_height as u32),
                )
                .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
                .draw(display)?;
            }

            // Text
            let text_color = if is_selected {
                BinaryColor::Off
            } else {
                BinaryColor::On
            };
            let text_y = y + ui_text::center_y(item_height as u32, Some(ui_text::DEFAULT_SIZE));
            ui_text::draw_colored(
                display,
                item,
                self.x + theme.metrics.side_padding as i32,
                text_y,
                Some(ui_text::DEFAULT_SIZE),
                text_color,
            )?;

            // Subtle bottom divider (only for non-selected items)
            if !is_selected && i < self.visible_count - 1 {
                Rectangle::new(
                    Point::new(self.x, y + item_height - 1),
                    Size::new(self.width, 1),
                )
                .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
                .draw(display)?;
            }
        }

        Ok(())
    }
}

/// Modal dialog for overlays
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Modal {
    pub title: String,
    pub message: String,
    pub buttons: Vec<String>,
    pub selected_button: usize,
}

impl Modal {
    /// Create a new modal dialog
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            buttons: Vec::new(),
            selected_button: 0,
        }
    }

    /// Add a button to the modal
    pub fn with_button(mut self, label: impl Into<String>) -> Self {
        self.buttons.push(label.into());
        self
    }

    /// Select next button
    pub fn select_next_button(&mut self) {
        if !self.buttons.is_empty() {
            self.selected_button = (self.selected_button + 1) % self.buttons.len();
        }
    }

    /// Select previous button
    pub fn select_prev_button(&mut self) {
        if !self.buttons.is_empty() {
            self.selected_button = if self.selected_button == 0 {
                self.buttons.len() - 1
            } else {
                self.selected_button - 1
            };
        }
    }

    /// Get selected button label
    pub fn selected_button_label(&self) -> Option<&str> {
        self.buttons.get(self.selected_button).map(|s| s.as_str())
    }

    /// Calculate modal dimensions
    fn dimensions(&self, display_width: u32, _display_height: u32, theme: &Theme) -> (u32, u32) {
        let width = (display_width * 4) / 5;
        let button_height = if self.buttons.is_empty() {
            0
        } else {
            theme.metrics.button_height + theme.metrics.spacing
        };
        let height = 120 + button_height;
        (width, height)
    }

    /// Render modal centered on display
    pub fn render<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
        theme: &Theme,
    ) -> Result<(), D::Error> {
        let display_width = display.bounding_box().size.width;
        let display_height = display.bounding_box().size.height;
        let (modal_width, modal_height) = self.dimensions(display_width, display_height, theme);

        let x = (display_width - modal_width) as i32 / 2;
        let y = (display_height - modal_height) as i32 / 2;

        // Modal background (white)
        Rectangle::new(Point::new(x, y), Size::new(modal_width, modal_height))
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::Off))
            .draw(display)?;

        // Clean border (1px)
        Rectangle::new(Point::new(x, y), Size::new(modal_width, modal_height))
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
            .draw(display)?;

        // Title
        ui_text::draw(
            display,
            &self.title,
            x + theme.metrics.side_padding as i32,
            y + ui_text::center_y(40, Some(ui_text::HEADER_SIZE)),
            Some(ui_text::HEADER_SIZE),
        )?;

        // Separator line
        Rectangle::new(Point::new(x, y + 30), Size::new(modal_width, 1))
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(display)?;

        // Message
        ui_text::draw(
            display,
            &self.message,
            x + theme.metrics.side_padding as i32,
            y + 30 + ui_text::center_y(40, Some(ui_text::SMALL_SIZE)),
            Some(ui_text::SMALL_SIZE),
        )?;

        // Buttons
        if !self.buttons.is_empty() {
            let button_width = (modal_width
                - (theme.metrics.spacing * (self.buttons.len() as u32 + 1)))
                / self.buttons.len() as u32;
            let button_y = y + modal_height as i32
                - theme.metrics.button_height as i32
                - theme.metrics.spacing as i32;
            let btn_text_y =
                ui_text::center_y(theme.metrics.button_height, Some(ui_text::DEFAULT_SIZE));

            for (i, button_label) in self.buttons.iter().enumerate() {
                let button_x = x
                    + theme.metrics.spacing as i32
                    + (i as i32) * (button_width as i32 + theme.metrics.spacing as i32);
                let is_selected = i == self.selected_button;

                // Button background - only fill if selected
                if is_selected {
                    Rectangle::new(
                        Point::new(button_x, button_y),
                        Size::new(button_width, theme.metrics.button_height),
                    )
                    .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
                    .draw(display)?;
                }

                // Button border
                Rectangle::new(
                    Point::new(button_x, button_y),
                    Size::new(button_width, theme.metrics.button_height),
                )
                .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
                .draw(display)?;

                // Button text (centered)
                let text_color = if is_selected {
                    BinaryColor::Off
                } else {
                    BinaryColor::On
                };
                let label_width = ui_text::width(button_label, Some(ui_text::DEFAULT_SIZE)) as i32;
                ui_text::draw_colored(
                    display,
                    button_label,
                    button_x + (button_width as i32 - label_width) / 2,
                    button_y + btn_text_y,
                    Some(ui_text::DEFAULT_SIZE),
                    text_color,
                )?;
            }
        }

        Ok(())
    }
}

/// Toast notification for brief messages
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toast {
    pub message: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
}

impl Toast {
    /// Create a new toast notification
    pub fn new(message: impl Into<String>, x: i32, y: i32, width: u32) -> Self {
        Self {
            message: message.into(),
            x,
            y,
            width,
        }
    }

    /// Create a centered toast at the bottom of the screen
    pub fn bottom_center(
        message: impl Into<String>,
        display_width: u32,
        display_height: u32,
    ) -> Self {
        let width = (display_width * 3) / 4;
        let x = ((display_width - width) / 2) as i32;
        let y = display_height as i32 - 80;
        Self::new(message, x, y, width)
    }

    /// Standard toast height
    pub fn height(&self) -> u32 {
        40
    }

    /// Render the toast
    pub fn render<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
    ) -> Result<(), D::Error> {
        // Semi-transparent background effect (solid black with white text for high contrast)
        Rectangle::new(
            Point::new(self.x, self.y),
            Size::new(self.width, self.height()),
        )
        .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
        .draw(display)?;

        // Text (inverted for contrast)
        ui_text::draw_colored(
            display,
            &self.message,
            self.x + 15,
            self.y + ui_text::center_y(self.height(), Some(ui_text::SMALL_SIZE)),
            Some(ui_text::SMALL_SIZE),
            BinaryColor::Off,
        )?;

        Ok(())
    }
}

/// Simple header bar component with clean, minimal design
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    pub title: String,
    pub right_text: Option<String>,
}

impl Header {
    /// Create a new header with just a title
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            right_text: None,
        }
    }

    /// Create a header with title and right-side text
    pub fn with_right_text(mut self, text: impl Into<String>) -> Self {
        self.right_text = Some(text.into());
        self
    }

    /// Render the header with clean design using Bookerly font
    pub fn render<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
        theme: &Theme,
    ) -> Result<(), D::Error> {
        let display_width = display.bounding_box().size.width;
        let header_height = theme.metrics.header_height;
        let text_y = ui_text::center_y(header_height, Some(ui_text::HEADER_SIZE));

        // Title on the left (Bookerly Bold)
        ui_text::draw(
            display,
            &self.title,
            theme.metrics.side_padding as i32,
            text_y,
            Some(ui_text::HEADER_SIZE),
        )?;

        // Right text if provided (Bookerly Bold, smaller size)
        if let Some(right) = &self.right_text {
            let text_width = ui_text::width(right, Some(ui_text::DEFAULT_SIZE));
            ui_text::draw(
                display,
                right,
                display_width as i32 - text_width as i32 - theme.metrics.side_padding as i32,
                text_y,
                Some(ui_text::DEFAULT_SIZE),
            )?;
        }

        // Bottom border line
        Rectangle::new(
            Point::new(0, header_height as i32 - 1),
            Size::new(display_width, 1),
        )
        .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
        .draw(display)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;
    use alloc::vec;

    #[test]
    fn button_bounds_calculation() {
        let theme = Theme::default();
        let button = Button::new("Test", 10, 20, 100);

        let bounds = button.bounds(&theme);
        assert_eq!(bounds.top_left, Point::new(10, 20));
        assert_eq!(bounds.size.width, 100);
        assert_eq!(bounds.size.height, theme.metrics.button_height);
    }

    #[test]
    fn list_navigation() {
        let items = vec![
            "Item 1".to_string(),
            "Item 2".to_string(),
            "Item 3".to_string(),
        ];
        let mut list = List::new(items, 0, 0, 200, 3);

        assert_eq!(list.selected(), Some("Item 1"));

        list.select_next();
        assert_eq!(list.selected(), Some("Item 2"));

        list.select_prev();
        assert_eq!(list.selected(), Some("Item 1"));
    }

    #[test]
    fn modal_button_selection() {
        let mut modal = Modal::new("Title", "Message")
            .with_button("OK")
            .with_button("Cancel");

        assert_eq!(modal.selected_button_label(), Some("OK"));

        modal.select_next_button();
        assert_eq!(modal.selected_button_label(), Some("Cancel"));

        modal.select_next_button();
        assert_eq!(modal.selected_button_label(), Some("OK"));
    }

    #[test]
    fn toast_positioning() {
        let toast = Toast::bottom_center("Test", 480, 800);
        assert_eq!(toast.width, 360); // 3/4 of 480
        assert_eq!(toast.x, 60); // (480 - 360) / 2
        assert_eq!(toast.y, 720); // 800 - 80
    }

    #[test]
    fn font_char_width_runtime() {
        // Default profile 4 → body = FONT_9X18_BOLD → char width = 9
        assert!(crate::ui::theme::ui_font_body_char_width() > 0);
    }
}
