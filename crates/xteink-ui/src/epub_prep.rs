//! Thin integration helpers around mu_epub render-prep APIs.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use mu_epub::EpubMetadata;
pub use mu_epub::{
    BlockRole, ChapterStylesheets, ComputedTextStyle, EmbeddedFontFace, EmbeddedFontStyle,
    FontLimits, FontPolicy, FontResolutionTrace, FontResolver, LayoutHints, PreparedChapter,
    RenderPrep, RenderPrepError, RenderPrepOptions, ResolvedFontFace, StyleConfig, StyleLimits,
    StyledChapter, StyledEvent, StyledEventOrRun, StyledRun, Styler, StylesheetSource,
};

/// Derive embedded font faces from metadata when only manifest context is available.
///
/// This is a lightweight fallback helper used by UI code paths that currently
/// hold parsed metadata but not an `EpubBook` handle.
pub fn embedded_fonts_from_metadata(metadata: &EpubMetadata) -> Vec<EmbeddedFontFace> {
    metadata
        .manifest
        .iter()
        .filter_map(|item| {
            let mt = item.media_type.to_ascii_lowercase();
            let href = item.href.to_ascii_lowercase();
            let is_font = mt.starts_with("font/")
                || href.ends_with(".ttf")
                || href.ends_with(".otf")
                || href.ends_with(".woff")
                || href.ends_with(".woff2");
            if !is_font {
                return None;
            }

            let normalized_family = normalize_font_family_from_href(&item.href);
            let (weight, style) = infer_weight_style_from_href(&href);
            Some(EmbeddedFontFace {
                family: normalized_family,
                weight,
                style,
                stretch: None,
                href: item.href.clone(),
                format: None,
            })
        })
        .collect()
}

fn normalize_font_family_from_href(href: &str) -> String {
    let filename = href.rsplit('/').next().unwrap_or(href);
    let stem = filename.split('.').next().unwrap_or(filename);
    stem.replace(['-', '_'], " ").trim().to_lowercase()
}

fn infer_weight_style_from_href(href: &str) -> (u16, EmbeddedFontStyle) {
    let black = href.contains("black") || href.contains("heavy");
    let bold = href.contains("bold");
    let semibold = href.contains("semibold")
        || href.contains("semi-bold")
        || href.contains("demibold")
        || href.contains("demi-bold");
    let medium = href.contains("medium");
    let light = href.contains("light") || href.contains("thin");
    let italic = href.contains("italic");
    let oblique = href.contains("oblique");

    let weight = if black {
        900
    } else if bold {
        700
    } else if semibold {
        600
    } else if medium {
        500
    } else if light {
        300
    } else {
        400
    };
    let style = if italic {
        EmbeddedFontStyle::Italic
    } else if oblique {
        EmbeddedFontStyle::Oblique
    } else {
        EmbeddedFontStyle::Normal
    };
    (weight, style)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infer_weight_style_from_href_recognizes_weight_keywords() {
        assert_eq!(
            infer_weight_style_from_href("fonts/SourceSans-SemiBold.ttf"),
            (600, EmbeddedFontStyle::Normal)
        );
        assert_eq!(
            infer_weight_style_from_href("fonts/SourceSans-BlackItalic.ttf"),
            (900, EmbeddedFontStyle::Italic)
        );
        assert_eq!(
            infer_weight_style_from_href("fonts/Bookerly-Light.otf"),
            (300, EmbeddedFontStyle::Normal)
        );
    }
}
