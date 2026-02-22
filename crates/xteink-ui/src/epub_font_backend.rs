//! EPUB renderer font backend using embedded bitmap fonts.
//!
//! Fonts are compiled into the firmware at build time (via build.rs),
//! eliminating runtime TTF loading and SD card dependencies.

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;

use embedded_graphics::{pixelcolor::BinaryColor, prelude::*};
use epub_stream_embedded_graphics::{
    BackendCapabilities, FontBackend, FontFaceRegistration, FontFallbackReason, FontId,
    FontMetrics, FontSelection,
};
use epub_stream_render::ResolvedTextStyle;
use epub_stream_render::TextMeasurer;

use crate::embedded_fonts::{EmbeddedFontCache, EmbeddedFontRegistry};
use crate::font_render::FontCache;

const FONT_REGULAR: &str = "bookerly-regular";
const FONT_BOLD: &str = "bookerly-bold";
const FONT_ITALIC: &str = "bookerly-italic";
const FONT_BOLD_ITALIC: &str = "bookerly-bold-italic";

const DEFAULT_SLOT_ID: FontId = 0;
const MIN_SIZE_PX: f32 = 10.0;
const MAX_SIZE_PX: f32 = 64.0;

#[allow(dead_code)]
const BOOKERLY_REGULAR_TTF: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/fonts/bookerly/Bookerly-Regular.ttf"
));
#[allow(dead_code)]
const BOOKERLY_BOLD_TTF: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/fonts/bookerly/Bookerly-Bold.ttf"
));
#[allow(dead_code)]
const BOOKERLY_ITALIC_TTF: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/fonts/bookerly/Bookerly-Italic.ttf"
));
#[allow(dead_code)]
const BOOKERLY_BOLD_ITALIC_TTF: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/fonts/bookerly/Bookerly-BoldItalic.ttf"
));

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct FontSlotKey {
    font_name: String,
    size_bits: u32,
}

impl FontSlotKey {
    fn new(font_name: String, size_px: f32) -> Self {
        Self {
            font_name,
            size_bits: size_px.clamp(MIN_SIZE_PX, MAX_SIZE_PX).to_bits(),
        }
    }

    fn size_px(&self) -> f32 {
        f32::from_bits(self.size_bits)
    }
}

struct BackendState {
    bitmap_cache: EmbeddedFontCache,
    #[allow(dead_code)]
    runtime_cache: FontCache,
    next_slot_id: FontId,
    slots_by_key: BTreeMap<FontSlotKey, FontId>,
    slot_keys_by_id: BTreeMap<FontId, FontSlotKey>,
    resolved_font_names_by_id: BTreeMap<u32, String>,
}

impl BackendState {
    fn new() -> Self {
        #[cfg(not(target_os = "espidf"))]
        let mut runtime_cache = FontCache::new();
        #[cfg(target_os = "espidf")]
        let runtime_cache = FontCache::new_with_glyph_cache_capacity(0);
        #[cfg(not(target_os = "espidf"))]
        {
            let _ = runtime_cache.load_font(FONT_REGULAR, BOOKERLY_REGULAR_TTF);
            let _ = runtime_cache.load_font(FONT_BOLD, BOOKERLY_BOLD_TTF);
            let _ = runtime_cache.load_font(FONT_ITALIC, BOOKERLY_ITALIC_TTF);
            let _ = runtime_cache.load_font(FONT_BOLD_ITALIC, BOOKERLY_BOLD_ITALIC_TTF);
        }

        let default_key = FontSlotKey::new(FONT_REGULAR.to_string(), 16.0);
        let mut slots_by_key = BTreeMap::new();
        slots_by_key.insert(default_key.clone(), DEFAULT_SLOT_ID);
        let mut slot_keys_by_id = BTreeMap::new();
        slot_keys_by_id.insert(DEFAULT_SLOT_ID, default_key);

        Self {
            bitmap_cache: EmbeddedFontCache::new(),
            runtime_cache,
            next_slot_id: 1,
            slots_by_key,
            slot_keys_by_id,
            resolved_font_names_by_id: BTreeMap::new(),
        }
    }

    fn default_font_name_for_style(style: &ResolvedTextStyle) -> &'static str {
        match (style.weight >= 700, style.italic) {
            (true, true) => FONT_BOLD_ITALIC,
            (true, false) => FONT_BOLD,
            (false, true) => FONT_ITALIC,
            (false, false) => FONT_REGULAR,
        }
    }

    fn is_generic_family(family: &str) -> bool {
        matches!(
            family.trim().to_ascii_lowercase().as_str(),
            "serif" | "sans-serif" | "sans" | "monospace" | "mono" | "fixed"
        )
    }

    fn ensure_slot_for(&mut self, key: FontSlotKey) -> FontId {
        if let Some(existing) = self.slots_by_key.get(&key) {
            return *existing;
        }

        let id = self.next_slot_id;
        self.next_slot_id = self.next_slot_id.saturating_add(1).max(1);
        self.slots_by_key.insert(key.clone(), id);
        self.slot_keys_by_id.insert(id, key);
        id
    }

    fn slot_key_for_id(&self, font_id: FontId) -> FontSlotKey {
        self.slot_keys_by_id
            .get(&font_id)
            .cloned()
            .unwrap_or_else(|| FontSlotKey::new(FONT_REGULAR.to_string(), 16.0))
    }

    fn font_name_for_weight(weight: u16, italic: bool) -> &'static str {
        match (weight >= 700, italic) {
            (true, true) => FONT_BOLD_ITALIC,
            (true, false) => FONT_BOLD,
            (false, true) => FONT_ITALIC,
            (false, false) => FONT_REGULAR,
        }
    }

    #[allow(dead_code)]
    fn runtime_font_available(&self, font_name: &str, size_px: f32) -> bool {
        self.runtime_cache
            .metrics(font_name, 'n')
            .or_else(|| self.runtime_cache.metrics(font_name, 'a'))
            .or_else(|| self.runtime_cache.metrics(font_name, ' '))
            .is_some_and(|m| {
                let _ = size_px;
                m.advance_width > 0.0
            })
    }

    fn measure_text_for_slot(&mut self, slot: &FontSlotKey, text: &str) -> f32 {
        #[cfg(not(target_os = "espidf"))]
        {
            self.runtime_cache.set_font_size(slot.size_px());
            if self.runtime_font_available(&slot.font_name, slot.size_px()) {
                return self
                    .runtime_cache
                    .measure_text(text, &slot.font_name)
                    .max(0.0);
            }
        }

        self.bitmap_cache.set_font(&slot.font_name);
        self.bitmap_cache.set_font_size(slot.size_px());
        self.bitmap_cache
            .measure_text(text, &slot.font_name)
            .max(0.0)
    }
}

/// Font backend that renders with embedded Bookerly bitmap fonts.
///
/// Fonts are pre-compiled at build time and embedded in firmware flash.
/// No SD card font files or runtime TTF parsing required.
pub struct BookerlyFontBackend {
    state: Arc<std::sync::Mutex<BackendState>>,
}

impl Default for BookerlyFontBackend {
    fn default() -> Self {
        Self {
            state: Arc::new(std::sync::Mutex::new(BackendState::new())),
        }
    }
}

impl Clone for BookerlyFontBackend {
    fn clone(&self) -> Self {
        Self {
            state: Arc::clone(&self.state),
        }
    }
}

impl FontBackend for BookerlyFontBackend {
    fn register_faces(&mut self, faces: &[FontFaceRegistration<'_>]) -> usize {
        let mut state = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };

        let mut accepted = 0usize;
        for (index, face) in faces.iter().enumerate() {
            let resolved_id = (index as u32) + 1;
            #[cfg(not(target_os = "espidf"))]
            let runtime_name = alloc::format!(
                "epub-face-{}-{}-{}-{}",
                resolved_id,
                face.family,
                face.weight,
                if face.italic { "i" } else { "n" }
            );
            #[cfg(not(target_os = "espidf"))]
            let chosen_name = if state
                .runtime_cache
                .load_font(&runtime_name, face.data)
                .is_ok()
            {
                runtime_name
            } else {
                BackendState::font_name_for_weight(face.weight, face.italic).to_string()
            };
            #[cfg(target_os = "espidf")]
            let chosen_name =
                BackendState::font_name_for_weight(face.weight, face.italic).to_string();
            state
                .resolved_font_names_by_id
                .insert(resolved_id, chosen_name);
            accepted = accepted.saturating_add(1);
        }
        accepted
    }

    fn resolve_font(&self, style: &ResolvedTextStyle, font_id: Option<u32>) -> FontSelection {
        let mut state = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };

        let mut fallback_reason = None;
        let chosen_name = if let Some(id) = font_id {
            match state.resolved_font_names_by_id.get(&id) {
                Some(name) => name.clone(),
                None => {
                    fallback_reason = Some(FontFallbackReason::UnknownFontId);
                    BackendState::default_font_name_for_style(style).to_string()
                }
            }
        } else if BackendState::is_generic_family(&style.family) {
            BackendState::default_font_name_for_style(style).to_string()
        } else {
            fallback_reason = Some(FontFallbackReason::UnknownFamily);
            BackendState::default_font_name_for_style(style).to_string()
        };

        let slot_id = state.ensure_slot_for(FontSlotKey::new(chosen_name, style.size_px));
        FontSelection {
            font_id: slot_id,
            fallback_reason,
        }
    }

    fn metrics(&self, font_id: FontId) -> FontMetrics {
        #[cfg(not(target_os = "espidf"))]
        let mut state = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        #[cfg(target_os = "espidf")]
        let state = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let slot = state.slot_key_for_id(font_id);
        #[cfg(not(target_os = "espidf"))]
        {
            state.runtime_cache.set_font_size(slot.size_px());
        }
        #[cfg(not(target_os = "espidf"))]
        if state.runtime_font_available(&slot.font_name, slot.size_px()) {
            let space_width = state
                .runtime_cache
                .metrics(&slot.font_name, ' ')
                .map(|m| m.advance_width.round() as i32)
                .unwrap_or(6)
                .max(1);
            let char_width = state
                .runtime_cache
                .metrics(&slot.font_name, 'n')
                .or_else(|| state.runtime_cache.metrics(&slot.font_name, 'a'))
                .map(|m| m.advance_width.round() as i32)
                .unwrap_or(space_width)
                .max(1);
            return FontMetrics {
                char_width,
                space_width,
            };
        }

        // Get metrics from embedded font
        let space_width =
            EmbeddedFontRegistry::get_font_nearest(&slot.font_name, slot.size_px() as u32)
                .and_then(|f| f.glyph(' '))
                .map(|g| g.advance_width as i32)
                .unwrap_or(6)
                .max(1);

        let char_width =
            EmbeddedFontRegistry::get_font_nearest(&slot.font_name, slot.size_px() as u32)
                .and_then(|f| f.glyph('n'))
                .map(|g| g.advance_width as i32)
                .unwrap_or(space_width)
                .max(1);

        FontMetrics {
            char_width,
            space_width,
        }
    }

    fn draw_text_run<D>(
        &self,
        display: &mut D,
        font_id: FontId,
        text: &str,
        origin: Point,
    ) -> Result<i32, D::Error>
    where
        D: DrawTarget<Color = BinaryColor>,
    {
        let mut state = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let slot = state.slot_key_for_id(font_id);
        const BASELINE_SAFETY_PX: i32 = 2;
        let top_y = origin.y;
        #[cfg(not(target_os = "espidf"))]
        if state.runtime_font_available(&slot.font_name, slot.size_px()) {
            let baseline_y =
                top_y + state.runtime_cache.baseline_offset(&slot.font_name) + BASELINE_SAFETY_PX;
            let width = state.measure_text_for_slot(&slot, text).round().max(0.0) as i32;
            state.runtime_cache.render_text(
                display,
                text,
                &slot.font_name,
                origin.x,
                baseline_y,
            )?;
            return Ok(width);
        }

        state.bitmap_cache.set_font(&slot.font_name);
        state.bitmap_cache.set_font_size(slot.size_px());
        let baseline_y =
            top_y + state.bitmap_cache.baseline_offset(&slot.font_name) + BASELINE_SAFETY_PX;

        let width = state.measure_text_for_slot(&slot, text).round().max(0.0) as i32;
        state
            .bitmap_cache
            .render_text(display, text, &slot.font_name, origin.x, baseline_y)?;
        Ok(width)
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            ttf: true,
            images: false,
            svg: false,
            justification: true,
        }
    }
}

impl TextMeasurer for BookerlyFontBackend {
    fn measure_text_px(&self, text: &str, style: &ResolvedTextStyle) -> f32 {
        if text.is_empty() {
            return 0.0;
        }
        let selection = <Self as FontBackend>::resolve_font(self, style, style.font_id);
        let mut state = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let slot = state.slot_key_for_id(selection.font_id);
        state.measure_text_for_slot(&slot, text)
    }
}

#[cfg(test)]
mod tests {
    use embedded_graphics::{mock_display::MockDisplay, pixelcolor::BinaryColor};
    use epub_stream::BlockRole;
    use epub_stream_render::JustifyMode;

    use super::*;

    fn body_style() -> ResolvedTextStyle {
        ResolvedTextStyle {
            font_id: None,
            family: "serif".to_string(),
            weight: 400,
            italic: false,
            size_px: 18.0,
            line_height: 1.3,
            letter_spacing: 0.0,
            role: BlockRole::Body,
            justify_mode: JustifyMode::None,
        }
    }

    #[test]
    fn text_measurer_matches_draw_width_for_same_font_path() {
        let backend = BookerlyFontBackend::default();
        let style = body_style();
        let text = "Layout parity across glyph widths";

        let measured = backend.measure_text_px(text, &style).round() as i32;
        let resolved = backend.resolve_font(&style, style.font_id);
        let mut display: MockDisplay<BinaryColor> = MockDisplay::new();
        display.set_allow_overdraw(true);
        display.set_allow_out_of_bounds_drawing(true);
        let drawn = backend
            .draw_text_run(&mut display, resolved.font_id, text, Point::new(0, 0))
            .expect("draw_text_run should succeed");

        assert_eq!(measured, drawn);
    }

    #[test]
    fn text_measurer_uses_same_fallback_resolution_as_draw_path() {
        let backend = BookerlyFontBackend::default();
        let mut style = body_style();
        style.family = "unknown-family".to_string();
        style.font_id = Some(9999);
        let text = "Fallback still needs stable widths";

        let measured = backend.measure_text_px(text, &style).round() as i32;
        let resolved = backend.resolve_font(&style, style.font_id);
        let mut display: MockDisplay<BinaryColor> = MockDisplay::new();
        display.set_allow_overdraw(true);
        display.set_allow_out_of_bounds_drawing(true);
        let drawn = backend
            .draw_text_run(&mut display, resolved.font_id, text, Point::new(0, 0))
            .expect("draw_text_run should succeed");

        assert_eq!(measured, drawn);
    }
}
