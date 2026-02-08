//! EPUB renderer font backend backed by Bookerly TTF faces.
//!
//! This is std-only and used by the desktop/simulator EPUB flow.

extern crate alloc;

use alloc::collections::BTreeMap;

use embedded_graphics::{pixelcolor::BinaryColor, prelude::*};
use epublet_embedded_graphics::{
    FontBackend, FontFaceRegistration, FontId, FontMetrics, FontSelection,
};
use epublet_render::ResolvedTextStyle;

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

const DEFAULT_FONT_ID: FontId = 0;
const MIN_SIZE_PX: f32 = 10.0;
const MAX_SIZE_PX: f32 = 36.0;

#[derive(Clone, Copy, Debug, PartialEq)]
struct FontSpec {
    size_px: f32,
    bold: bool,
    italic: bool,
}

impl FontSpec {
    fn from_style(style: &ResolvedTextStyle) -> Self {
        Self {
            size_px: style.size_px.clamp(MIN_SIZE_PX, MAX_SIZE_PX),
            bold: style.weight >= 700,
            italic: style.italic,
        }
    }

    fn font_name(self) -> &'static str {
        match (self.bold, self.italic) {
            (true, true) => FONT_BOLD_ITALIC,
            (true, false) => FONT_BOLD,
            (false, true) => FONT_ITALIC,
            (false, false) => FONT_REGULAR,
        }
    }
}

struct BackendState {
    cache: FontCache,
    next_id: FontId,
    specs: BTreeMap<FontId, FontSpec>,
    upstream_to_local: BTreeMap<u32, FontId>,
}

impl BackendState {
    fn new() -> Self {
        let mut cache = FontCache::new();
        let _ = cache.load_font(FONT_REGULAR, BOOKERLY_REGULAR);
        let _ = cache.load_font(FONT_BOLD, BOOKERLY_BOLD);
        let _ = cache.load_font(FONT_ITALIC, BOOKERLY_ITALIC);
        let _ = cache.load_font(FONT_BOLD_ITALIC, BOOKERLY_BOLD_ITALIC);
        Self {
            cache,
            next_id: 1,
            specs: BTreeMap::new(),
            upstream_to_local: BTreeMap::new(),
        }
    }

    fn spec_for_id(&self, font_id: FontId) -> FontSpec {
        self.specs.get(&font_id).copied().unwrap_or(FontSpec {
            size_px: 16.0,
            bold: false,
            italic: false,
        })
    }

    fn ensure_font_id_for_spec(&mut self, spec: FontSpec) -> FontId {
        if let Some((font_id, _)) = self.specs.iter().find(|(_, existing)| **existing == spec) {
            return *font_id;
        }

        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1).max(1);
        self.specs.insert(id, spec);
        id
    }
}

/// Font backend that renders with Bookerly faces and style-derived size/weight.
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
    fn register_faces(&mut self, _faces: &[FontFaceRegistration<'_>]) -> usize {
        0
    }

    fn resolve_font(&self, style: &ResolvedTextStyle, font_id: Option<u32>) -> FontSelection {
        let mut state = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let spec = FontSpec::from_style(style);
        let mapped = state.ensure_font_id_for_spec(spec);
        if let Some(raw) = font_id {
            state.upstream_to_local.insert(raw, mapped);
        }
        FontSelection {
            font_id: mapped,
            fallback_reason: None,
        }
    }

    fn metrics(&self, font_id: FontId) -> FontMetrics {
        let mut state = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let resolved_id = if font_id == DEFAULT_FONT_ID {
            DEFAULT_FONT_ID
        } else {
            state
                .upstream_to_local
                .get(&(font_id as u32))
                .copied()
                .unwrap_or(font_id)
        };
        let spec = state.spec_for_id(resolved_id);
        state.cache.set_font(spec.font_name());
        state.cache.set_font_size(spec.size_px);
        let space_width = state
            .cache
            .metrics(spec.font_name(), ' ')
            .map(|m| m.advance_width.round() as i32)
            .unwrap_or(6)
            .max(1);
        let char_width = state
            .cache
            .metrics(spec.font_name(), 'n')
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
        let resolved_id = if font_id == DEFAULT_FONT_ID {
            DEFAULT_FONT_ID
        } else {
            state
                .upstream_to_local
                .get(&(font_id as u32))
                .copied()
                .unwrap_or(font_id)
        };
        let spec = state.spec_for_id(resolved_id);
        state.cache.set_font(spec.font_name());
        state.cache.set_font_size(spec.size_px);
        let width = state
            .cache
            .measure_text(text, spec.font_name())
            .round()
            .max(0.0) as i32;
        state
            .cache
            .render_text(display, text, spec.font_name(), origin.x, origin.y)?;
        Ok(width)
    }
}
