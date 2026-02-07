//! EPUB parsing module for Xteink X4
//!
//! Provides SAX-style XML parsing for EPUB metadata and spine
//! using quick-xml for memory-efficient processing.

#[cfg(feature = "std")]
pub mod metadata;

#[cfg(feature = "std")]
pub mod spine;

#[cfg(feature = "quick-xml")]
pub mod tokenizer;

#[cfg(feature = "std")]
pub mod layout;

#[cfg(feature = "std")]
pub mod streaming_zip;

// Test module - only compiled during tests
#[cfg(all(test, feature = "std"))]
pub mod tests;

// Re-export main types for convenience
#[cfg(feature = "std")]
pub use metadata::{extract_metadata, parse_container_xml, parse_opf, EpubMetadata, ManifestItem};

#[cfg(feature = "std")]
pub use spine::{create_spine, parse_spine, Spine, SpineItem};

#[cfg(feature = "quick-xml")]
pub use tokenizer::{tokenize_html, Token, TokenizeError};

#[cfg(feature = "std")]
pub use layout::{FontMetrics, LayoutConfig, LayoutEngine, Line, Page, TextStyle};

#[cfg(feature = "std")]
pub use streaming_zip::{open_epub, CdEntry, StreamingZip, ZipError};
