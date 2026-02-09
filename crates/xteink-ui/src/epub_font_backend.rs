//! EPUB renderer font backend backed by Bookerly TTF faces.
//!
//! This is std-only and used by the desktop/simulator EPUB flow.

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;

use embedded_graphics::{pixelcolor::BinaryColor, prelude::*};
use mu_epub_embedded_graphics::{
    BackendCapabilities, FontBackend, FontFaceRegistration, FontFallbackReason, FontId,
    FontMetrics, FontSelection,
};
use mu_epub_render::ResolvedTextStyle;

use crate::font_render::FontCache;

const BOOKERLY_REGULAR: &[u8] = include_bytes!("../assets/fonts/bookerly/Bookerly-Regular.ttf");
const BOOKERLY_BOLD: &[u8] = include_bytes!("../assets/fonts/bookerly/Bookerly-Bold.ttf");
const BOOKERLY_ITALIC: &[u8] = include_bytes!("../assets/fonts/bookerly/Bookerly-Italic.ttf");
const BOOKERLY_BOLD_ITALIC: &[u8] =
    include_bytes!("../assets/fonts/bookerly/Bookerly-BoldItalic.ttf");

const FONT_REGULAR: &str = "bookerly-regular";
const FONT_BOLD: &str = "bookerly-bold";
const FONT_ITALIC: &str = "bookerly-italic";
const FONT_BOLD_ITALIC: &str = "bookerly-bold-italic";

const DEFAULT_SLOT_ID: FontId = 0;
const MIN_SIZE_PX: f32 = 10.0;
const MAX_SIZE_PX: f32 = 36.0;

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
    cache: FontCache,
    next_slot_id: FontId,
    slots_by_key: BTreeMap<FontSlotKey, FontId>,
    slot_keys_by_id: BTreeMap<FontId, FontSlotKey>,
    embedded_font_names_by_resolved_id: BTreeMap<u32, String>,
}

impl BackendState {
    fn new() -> Self {
        let mut cache = FontCache::new();
        let _ = cache.load_font(FONT_REGULAR, BOOKERLY_REGULAR);
        let _ = cache.load_font(FONT_BOLD, BOOKERLY_BOLD);
        let _ = cache.load_font(FONT_ITALIC, BOOKERLY_ITALIC);
        let _ = cache.load_font(FONT_BOLD_ITALIC, BOOKERLY_BOLD_ITALIC);

        let default_key = FontSlotKey::new(FONT_REGULAR.to_string(), 16.0);
        let mut slots_by_key = BTreeMap::new();
        slots_by_key.insert(default_key.clone(), DEFAULT_SLOT_ID);
        let mut slot_keys_by_id = BTreeMap::new();
        slot_keys_by_id.insert(DEFAULT_SLOT_ID, default_key);

        Self {
            cache,
            next_slot_id: 1,
            slots_by_key,
            slot_keys_by_id,
            embedded_font_names_by_resolved_id: BTreeMap::new(),
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
}

/// Font backend that renders with Bookerly by default and supports
/// dynamic registration of EPUB embedded fonts.
pub struct BookerlyFontBackend {
    state: std::sync::Mutex<BackendState>,
}

impl Default for BookerlyFontBackend {
    fn default() -> Self {
        Self {
            state: std::sync::Mutex::new(BackendState::new()),
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
            let font_name = format!(
                "embedded-{}-{}-{}-{}",
                index,
                face.family,
                face.weight,
                if face.italic { "i" } else { "n" }
            );
            if state.cache.load_font(&font_name, face.data).is_ok() {
                state
                    .embedded_font_names_by_resolved_id
                    .insert((index as u32) + 1, font_name);
                accepted += 1;
            }
        }
        accepted
    }

    fn resolve_font(&self, style: &ResolvedTextStyle, font_id: Option<u32>) -> FontSelection {
        let mut state = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };

        let use_embedded = !BackendState::is_generic_family(&style.family);
        let chosen_name = if use_embedded {
            font_id
                .and_then(|id| state.embedded_font_names_by_resolved_id.get(&id).cloned())
                .unwrap_or_else(|| BackendState::default_font_name_for_style(style).to_string())
        } else {
            BackendState::default_font_name_for_style(style).to_string()
        };
        let fallback_reason = if use_embedded {
            font_id
                .filter(|id| !state.embedded_font_names_by_resolved_id.contains_key(id))
                .map(|_| FontFallbackReason::UnknownFontId)
        } else {
            None
        };

        let slot_id = state.ensure_slot_for(FontSlotKey::new(chosen_name, style.size_px));
        FontSelection {
            font_id: slot_id,
            fallback_reason,
        }
    }

    fn metrics(&self, font_id: FontId) -> FontMetrics {
        let mut state = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let slot = state.slot_key_for_id(font_id);
        state.cache.set_font(&slot.font_name);
        state.cache.set_font_size(slot.size_px());
        let space_width = state
            .cache
            .metrics(&slot.font_name, ' ')
            .map(|m| m.advance_width.round() as i32)
            .unwrap_or(6)
            .max(1);
        let char_width = state
            .cache
            .metrics(&slot.font_name, 'n')
            .map(|m| m.advance_width.round() as i32)
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
        state.cache.set_font(&slot.font_name);
        state.cache.set_font_size(slot.size_px());
        let width = state
            .cache
            .measure_text(text, &slot.font_name)
            .round()
            .max(0.0) as i32;
        state
            .cache
            .render_text(display, text, &slot.font_name, origin.x, origin.y)?;
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
