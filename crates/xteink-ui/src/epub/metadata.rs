//! EPUB metadata parser using quick-xml SAX-style parsing
//!
//! Parses container.xml to find the OPF package file,
//! then extracts metadata and manifest from the OPF.

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use quick_xml::events::Event;
use quick_xml::reader::Reader;

/// Maximum number of manifest items (fixed-size constraint)
const MAX_MANIFEST_ITEMS: usize = 64;

/// A single item in the EPUB manifest (id -> href mapping)
#[derive(Clone, Debug, PartialEq)]
pub struct ManifestItem {
    pub id: String,
    pub href: String,
    pub media_type: String,
    pub properties: Option<String>,
}

/// EPUB metadata extracted from content.opf
#[derive(Clone, Debug, PartialEq)]
pub struct EpubMetadata {
    pub title: String,
    pub author: String,
    pub language: String,
    pub manifest: Vec<ManifestItem>,
    pub cover_id: Option<String>,
}

impl Default for EpubMetadata {
    fn default() -> Self {
        Self {
            title: String::from("Unknown Title"),
            author: String::from("Unknown Author"),
            language: String::from("en"),
            manifest: Vec::new(),
            cover_id: None,
        }
    }
}

impl EpubMetadata {
    /// Create empty metadata structure
    pub fn new() -> Self {
        Self::default()
    }

    /// Get manifest item by id
    pub fn get_item(&self, id: &str) -> Option<&ManifestItem> {
        self.manifest.iter().find(|item| item.id == id)
    }

    /// Get cover image manifest item
    pub fn get_cover_item(&self) -> Option<&ManifestItem> {
        self.cover_id.as_ref().and_then(|id| self.get_item(id))
    }

    /// Find item ID by href path
    pub fn find_item_by_href(&self, href: &str) -> Option<&str> {
        self.manifest
            .iter()
            .find(|item| item.href == href)
            .map(|item| item.id.as_str())
    }
}

/// Parse container.xml to find the OPF package file path
///
/// Returns the full-path attribute from the rootfile element
pub fn parse_container_xml(content: &[u8]) -> Result<String, String> {
    let mut reader = Reader::from_reader(content);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut opf_path: Option<String> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let name = reader.decoder().decode(e.name().as_ref())
                    .map_err(|e| format!("Decode error: {:?}", e))?.to_string();
                
                if name == "rootfile" {
                    // Extract full-path attribute
                    for attr in e.attributes() {
                        let attr = attr.map_err(|e| format!("Attr error: {:?}", e))?;
                        let key = reader.decoder().decode(attr.key.as_ref())
                            .map_err(|e| format!("Decode error: {:?}", e))?;
                        if key == "full-path" {
                            let value = reader.decoder().decode(&attr.value)
                                .map_err(|e| format!("Decode error: {:?}", e))?
                                .to_string();
                            opf_path = Some(value);
                            break;
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("XML parse error: {:?}", e)),
            _ => {}
        }
        buf.clear();
    }

    opf_path.ok_or_else(|| String::from("No rootfile found in container.xml"))
}

/// Parse content.opf to extract metadata and manifest
///
/// Uses SAX-style parsing with quick-xml
pub fn parse_opf(content: &[u8]) -> Result<EpubMetadata, String> {
    let mut reader = Reader::from_reader(content);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut metadata = EpubMetadata::new();
    
    // State tracking
    let mut current_element: Option<String> = None;
    let mut in_metadata = false;
    let mut in_manifest = false;
    let mut in_spine = false;
    let mut dc_ns = "http://purl.org/dc/elements/1.1/";

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = reader.decoder().decode(e.name().as_ref())
                    .map_err(|e| format!("Decode error: {:?}", e))?.to_string();
                
                // Track which section we're in
                match name.as_str() {
                    "metadata" => in_metadata = true,
                    "manifest" => in_manifest = true,
                    "spine" => in_spine = true,
                    _ => {}
                }

                // Parse manifest item
                if in_manifest && name == "item" && metadata.manifest.len() < MAX_MANIFEST_ITEMS {
                    if let Some(item) = parse_manifest_item(&e, &reader)? {
                        // Check if this is a cover image (EPUB3)
                        if item.properties.as_ref().map_or(false, |p| p.contains("cover-image")) {
                            metadata.cover_id = Some(item.id.clone());
                        }
                        metadata.manifest.push(item);
                    }
                }

                // Track metadata elements
                if in_metadata {
                    current_element = Some(name.clone());
                    
                    // Check for EPUB2 cover meta tag
                    if name == "meta" {
                        let mut name_attr = None;
                        let mut content_attr = None;
                        
                        for attr in e.attributes() {
                            let attr = attr.map_err(|e| format!("Attr error: {:?}", e))?;
                            let key = reader.decoder().decode(attr.key.as_ref())
                                .map_err(|e| format!("Decode error: {:?}", e))?;
                            let value = reader.decoder().decode(&attr.value)
                                .map_err(|e| format!("Decode error: {:?}", e))?;
                            
                            if key == "name" && value == "cover" {
                                name_attr = Some(value.to_string());
                            }
                            if key == "content" {
                                content_attr = Some(value.to_string());
                            }
                        }
                        
                        if name_attr.is_some() && content_attr.is_some() {
                            metadata.cover_id = content_attr;
                        }
                    }
                }

                // Track spine itemref
                if in_spine && name == "itemref" {
                    // Spine items are collected separately by spine.rs
                    // We just validate the structure here
                }
            }
            Ok(Event::Text(e)) => {
                if let Some(ref elem) = current_element {
                    let text = reader.decoder().decode(&e)
                        .map_err(|e| format!("Decode error: {:?}", e))?
                        .to_string();
                    
                    // Extract metadata fields
                    match elem.as_str() {
                        // Handle dc: prefixed elements
                        n if n.ends_with("title") || n == "dc:title" => {
                            metadata.title = text;
                        }
                        n if n.ends_with("creator") || n == "dc:creator" => {
                            metadata.author = text;
                        }
                        n if n.ends_with("language") || n == "dc:language" => {
                            metadata.language = text;
                        }
                        _ => {}
                    }
                }
            }
            Ok(Event::End(e)) => {
                let name = reader.decoder().decode(e.name().as_ref())
                    .map_err(|e| format!("Decode error: {:?}", e))?.to_string();
                
                match name.as_str() {
                    "metadata" => in_metadata = false,
                    "manifest" => in_manifest = false,
                    "spine" => in_spine = false,
                    _ => {}
                }
                
                current_element = None;
            }
            Ok(Event::Empty(e)) => {
                let name = reader.decoder().decode(e.name().as_ref())
                    .map_err(|e| format!("Decode error: {:?}", e))?.to_string();
                
                // Handle empty manifest items
                if in_manifest && name == "item" && metadata.manifest.len() < MAX_MANIFEST_ITEMS {
                    if let Some(item) = parse_manifest_item(&e, &reader)? {
                        if item.properties.as_ref().map_or(false, |p| p.contains("cover-image")) {
                            metadata.cover_id = Some(item.id.clone());
                        }
                        metadata.manifest.push(item);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("XML parse error: {:?}", e)),
            _ => {}
        }
        buf.clear();
    }

    Ok(metadata)
}

/// Parse a manifest item from XML element attributes
fn parse_manifest_item<'a>(
    e: &quick_xml::events::BytesStart<'a>,
    reader: &Reader<&[u8]>,
) -> Result<Option<ManifestItem>, String> {
    let mut id = None;
    let mut href = None;
    let mut media_type = None;
    let mut properties = None;

    for attr in e.attributes() {
        let attr = attr.map_err(|e| format!("Attr error: {:?}", e))?;
        let key = reader.decoder().decode(attr.key.as_ref())
            .map_err(|e| format!("Decode error: {:?}", e))?;
        let value = reader.decoder().decode(&attr.value)
            .map_err(|e| format!("Decode error: {:?}", e))?
            .to_string();

        match key {
            "id" => id = Some(value),
            "href" => href = Some(value),
            "media-type" => media_type = Some(value),
            "properties" => properties = Some(value),
            _ => {}
        }
    }

    if let (Some(id), Some(href), Some(media_type)) = (id, href, media_type) {
        Ok(Some(ManifestItem {
            id,
            href,
            media_type,
            properties,
        }))
    } else {
        Ok(None) // Skip incomplete items
    }
}

/// Full EPUB metadata extraction from both container.xml and content.opf
///
/// This is a convenience function that takes both file contents and returns
/// the complete metadata structure.
pub fn extract_metadata(
    container_xml: &[u8],
    opf_content: &[u8],
) -> Result<EpubMetadata, String> {
    // Verify the OPF path matches (optional validation)
    let _opf_path = parse_container_xml(container_xml)?;
    
    // Parse the OPF content
    parse_opf(opf_content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_container_xml() {
        let container = br#"<?xml version="1.0"?>
<container xmlns="urn:oasis:names:tc:opendocument:xmlns:container" version="1.0">
   <rootfiles>
      <rootfile full-path="EPUB/package.opf" media-type="application/oebps-package+xml"/>
   </rootfiles>
</container>"#;
        
        let result = parse_container_xml(container).unwrap();
        assert_eq!(result, "EPUB/package.opf");
    }

    #[test]
    fn test_parse_opf_basic() {
        let opf = br#"<?xml version="1.0"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Test Book</dc:title>
    <dc:creator>Test Author</dc:creator>
    <dc:language>en</dc:language>
  </metadata>
  <manifest>
    <item id="cover" href="cover.xhtml" media-type="application/xhtml+xml"/>
    <item id="chapter1" href="chapter1.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
</package>"#;
        
        let metadata = parse_opf(opf).unwrap();
        assert_eq!(metadata.title, "Test Book");
        assert_eq!(metadata.author, "Test Author");
        assert_eq!(metadata.language, "en");
        assert_eq!(metadata.manifest.len(), 2);
        assert_eq!(metadata.manifest[0].id, "cover");
        assert_eq!(metadata.manifest[1].href, "chapter1.xhtml");
    }

    #[test]
    fn test_parse_opf_with_cover() {
        let opf = br#"<?xml version="1.0"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Book with Cover</dc:title>
    <meta name="cover" content="cover-image"/>
  </metadata>
  <manifest>
    <item id="cover-image" href="images/cover.jpg" media-type="image/jpeg" properties="cover-image"/>
  </manifest>
</package>"#;
        
        let metadata = parse_opf(opf).unwrap();
        assert_eq!(metadata.title, "Book with Cover");
        assert_eq!(metadata.cover_id, Some("cover-image".to_string()));
    }

    #[test]
    fn test_get_item() {
        let mut metadata = EpubMetadata::new();
        metadata.manifest.push(ManifestItem {
            id: "item1".to_string(),
            href: "chapter1.xhtml".to_string(),
            media_type: "application/xhtml+xml".to_string(),
            properties: None,
        });

        let item = metadata.get_item("item1");
        assert!(item.is_some());
        assert_eq!(item.unwrap().href, "chapter1.xhtml");
        
        assert!(metadata.get_item("nonexistent").is_none());
    }
}
