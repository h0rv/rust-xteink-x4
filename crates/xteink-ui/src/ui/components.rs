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
    mono_font::{ascii, MonoTextStyle, MonoTextStyleBuilder},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    text::Text,
};

use crate::ui::theme::Theme;

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

        let character_style = MonoTextStyle::new(&ascii::FONT_7X13, text_color);
        let label_text = Text::new(
            &self.label,
            Point::new(
                self.x + (self.width as i32) / 2,
                self.y + (theme.metrics.button_height as i32) / 2 + 4,
            ),
            character_style,
        );
        label_text.draw(display)?;

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

            // Background
            let bg_color = if is_selected {
                BinaryColor::On
            } else {
                BinaryColor::Off
            };
            Rectangle::new(
                Point::new(self.x, y),
                Size::new(self.width, item_height as u32),
            )
            .into_styled(PrimitiveStyle::with_fill(bg_color))
            .draw(display)?;

            // Text
            let text_color = if is_selected {
                BinaryColor::Off
            } else {
                BinaryColor::On
            };
            let character_style = MonoTextStyle::new(&ascii::FONT_7X13, text_color);
            Text::new(
                item,
                Point::new(self.x + theme.metrics.side_padding as i32, y + 28),
                character_style,
            )
            .draw(display)?;

            // Separator line (except for last visible)
            if i < self.visible_count - 1 {
                Rectangle::new(
                    Point::new(self.x + 10, y + item_height - 1),
                    Size::new(self.width - 20, 1),
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

        // Border
        Rectangle::new(Point::new(x, y), Size::new(modal_width, modal_height))
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 2))
            .draw(display)?;

        // Title
        let title_style = MonoTextStyleBuilder::new()
            .font(&ascii::FONT_7X13_BOLD)
            .text_color(BinaryColor::On)
            .build();
        Text::new(
            &self.title,
            Point::new(x + theme.metrics.side_padding as i32, y + 25),
            title_style,
        )
        .draw(display)?;

        // Separator line
        Rectangle::new(Point::new(x + 10, y + 35), Size::new(modal_width - 20, 1))
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(display)?;

        // Message
        let message_style = MonoTextStyle::new(&ascii::FONT_7X13, BinaryColor::On);
        Text::new(
            &self.message,
            Point::new(x + theme.metrics.side_padding as i32, y + 60),
            message_style,
        )
        .draw(display)?;

        // Buttons
        if !self.buttons.is_empty() {
            let button_width = (modal_width
                - (theme.metrics.spacing * (self.buttons.len() as u32 + 1)))
                / self.buttons.len() as u32;
            let button_y = y + modal_height as i32
                - theme.metrics.button_height as i32
                - theme.metrics.spacing as i32;

            for (i, button_label) in self.buttons.iter().enumerate() {
                let button_x = x
                    + theme.metrics.spacing as i32
                    + (i as i32) * (button_width as i32 + theme.metrics.spacing as i32);
                let is_selected = i == self.selected_button;

                // Button background
                let bg_color = if is_selected {
                    BinaryColor::On
                } else {
                    BinaryColor::Off
                };
                Rectangle::new(
                    Point::new(button_x, button_y),
                    Size::new(button_width, theme.metrics.button_height),
                )
                .into_styled(PrimitiveStyle::with_fill(bg_color))
                .draw(display)?;

                // Button border
                Rectangle::new(
                    Point::new(button_x, button_y),
                    Size::new(button_width, theme.metrics.button_height),
                )
                .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
                .draw(display)?;

                // Button text
                let text_color = if is_selected {
                    BinaryColor::Off
                } else {
                    BinaryColor::On
                };
                let text_style = MonoTextStyle::new(&ascii::FONT_7X13, text_color);
                Text::new(
                    button_label,
                    Point::new(button_x + (button_width as i32) / 2 - 10, button_y + 26),
                    text_style,
                )
                .draw(display)?;
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
        let character_style = MonoTextStyle::new(&ascii::FONT_7X13, BinaryColor::Off);
        Text::new(
            &self.message,
            Point::new(self.x + 15, self.y + 26),
            character_style,
        )
        .draw(display)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;
    use alloc::vec;
    use embedded_graphics::mock_display::MockDisplay;

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
}
