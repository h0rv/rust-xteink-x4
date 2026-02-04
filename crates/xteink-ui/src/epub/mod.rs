//! EPUB parsing module for Xteink X4
//!
//! Provides SAX-style XML parsing for EPUB metadata and spine
//! using quick-xml for memory-efficient processing.

#[cfg(feature = "std")]
pub mod metadata;

#[cfg(feature = "std")]
pub mod spine;

// Re-export main types for convenience
#[cfg(feature = "std")]
pub use metadata::{EpubMetadata, ManifestItem, parse_container_xml, parse_opf, extract_metadata};

#[cfg(feature = "std")]
pub use spine::{Spine, SpineItem, parse_spine, create_spine};
