//! EPUB spine parser and chapter navigation
//!
//! The spine defines the reading order of chapters. This module parses
//! the spine from content.opf and provides navigation utilities.

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use quick_xml::events::Event;
use quick_xml::reader::Reader;

/// Maximum number of spine items (fixed-size constraint)
const MAX_SPINE_ITEMS: usize = 256;

/// A single item in the EPUB spine (chapter reference)
#[derive(Clone, Debug, PartialEq)]
pub struct SpineItem {
    pub idref: String,
    pub id: Option<String>,
    pub linear: bool,
    pub properties: Option<String>,
}

/// Spine represents the reading order of an EPUB
/// 
/// Tracks the ordered list of chapter IDs and provides navigation.
#[derive(Clone, Debug, PartialEq)]
pub struct Spine {
    pub items: Vec<SpineItem>,
    pub current: usize,
}

impl Default for Spine {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            current: 0,
        }
    }
}

impl Spine {
    /// Create a new empty spine
    pub fn new() -> Self {
        Self::default()
    }

    /// Create spine from a list of chapter IDs
    pub fn from_idrefs(idrefs: Vec<String>) -> Self {
        let items = idrefs
            .into_iter()
            .map(|idref| SpineItem {
                idref,
                id: None,
                linear: true,
                properties: None,
            })
            .collect();
        
        Self { items, current: 0 }
    }

    /// Get total number of chapters
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Check if spine is empty
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Get current chapter ID
    pub fn current_id(&self) -> Option<&str> {
        self.items.get(self.current).map(|item| item.idref.as_str())
    }

    /// Get current spine item
    pub fn current_item(&self) -> Option<&SpineItem> {
        self.items.get(self.current)
    }

    /// Get chapter ID at specific index
    pub fn get_id(&self, index: usize) -> Option<&str> {
        self.items.get(index).map(|item| item.idref.as_str())
    }

    /// Get spine item at specific index
    pub fn get_item(&self, index: usize) -> Option<&SpineItem> {
        self.items.get(index)
    }

    /// Get current position (0-indexed)
    pub fn position(&self) -> usize {
        self.current
    }

    /// Navigate to next chapter
    /// Returns true if navigation succeeded, false if at end
    pub fn next(&mut self) -> bool {
        if self.current + 1 < self.items.len() {
            self.current += 1;
            true
        } else {
            false
        }
    }

    /// Navigate to previous chapter
    /// Returns true if navigation succeeded, false if at start
    pub fn prev(&mut self) -> bool {
        if self.current > 0 {
            self.current -= 1;
            true
        } else {
            false
        }
    }

    /// Navigate to specific chapter by index
    /// Returns true if successful
    pub fn go_to(&mut self, index: usize) -> bool {
        if index < self.items.len() {
            self.current = index;
            true
        } else {
            false
        }
    }

    /// Navigate to chapter by ID
    /// Returns true if found and navigated
    pub fn go_to_id(&mut self, idref: &str) -> bool {
        if let Some(index) = self.items.iter().position(|item| item.idref == idref) {
            self.current = index;
            true
        } else {
            false
        }
    }

    /// Get progress as percentage (0-100)
    pub fn progress_percent(&self) -> u8 {
        if self.items.is_empty() {
            0
        } else {
            ((self.current * 100) / self.items.len()).min(100) as u8
        }
    }

    /// Get progress as fraction (current, total)
    pub fn progress(&self) -> (usize, usize) {
        (self.current, self.items.len())
    }

    /// Check if at first chapter
    pub fn is_first(&self) -> bool {
        self.current == 0
    }

    /// Check if at last chapter
    pub fn is_last(&self) -> bool {
        self.current + 1 >= self.items.len()
    }

    /// Get all chapter IDs as strings
    pub fn chapter_ids(&self) -> Vec<&str> {
        self.items.iter().map(|item| item.idref.as_str()).collect()
    }
}

/// Parse spine from OPF content
///
/// Extracts the ordered list of itemrefs from the spine element.
pub fn parse_spine(content: &[u8]) -> Result<Spine, String> {
    let mut reader = Reader::from_reader(content);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut spine = Spine::new();
    let mut in_spine = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let name = reader.decoder().decode(e.name().as_ref())
                    .map_err(|e| format!("Decode error: {:?}", e))?.to_string();
                
                if name == "spine" {
                    in_spine = true;
                    
                    // Check for toc attribute (EPUB2 NCX reference)
                    for attr in e.attributes() {
                        let attr = attr.map_err(|e| format!("Attr error: {:?}", e))?;
                        let key = reader.decoder().decode(attr.key.as_ref())
                            .map_err(|e| format!("Decode error: {:?}", e))?;
                        // toc attribute is useful but we don't store it currently
                        let _ = key;
                    }
                }

                if in_spine && name == "itemref" && spine.items.len() < MAX_SPINE_ITEMS {
                    if let Some(item) = parse_spine_item(&e, &reader)? {
                        spine.items.push(item);
                    }
                }
            }
            Ok(Event::End(e)) => {
                let name = reader.decoder().decode(e.name().as_ref())
                    .map_err(|e| format!("Decode error: {:?}", e))?.to_string();
                
                if name == "spine" {
                    in_spine = false;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("XML parse error: {:?}", e)),
            _ => {}
        }
        buf.clear();
    }

    Ok(spine)
}

/// Parse a spine itemref from XML element attributes
fn parse_spine_item<'a>(
    e: &quick_xml::events::BytesStart<'a>,
    reader: &Reader<&[u8]>,
) -> Result<Option<SpineItem>, String> {
    let mut idref = None;
    let mut id = None;
    let mut linear = true;
    let mut properties = None;

    for attr in e.attributes() {
        let attr = attr.map_err(|e| format!("Attr error: {:?}", e))?;
        let key = reader.decoder().decode(attr.key.as_ref())
            .map_err(|e| format!("Decode error: {:?}", e))?;
        let value = reader.decoder().decode(&attr.value)
            .map_err(|e| format!("Decode error: {:?}", e))?
            .to_string();

        match key {
            "idref" => idref = Some(value),
            "id" => id = Some(value),
            "linear" => linear = value != "no",
            "properties" => properties = Some(value),
            _ => {}
        }
    }

    idref.map(|idref| {
        Ok(SpineItem {
            idref,
            id,
            linear,
            properties,
        })
    }).transpose()
}

/// Parse both metadata and spine from OPF content
///
/// Convenience function that extracts both structures in one pass.
/// Note: This is less efficient than separate parsing if you only need one.
pub fn parse_opf_spine(content: &[u8]) -> Result<Spine, String> {
    parse_spine(content)
}

/// Create a spine from raw chapter IDs (for testing or simple EPUBs)
pub fn create_spine(chapter_ids: &[&str]) -> Spine {
    let items = chapter_ids
        .iter()
        .map(|id| SpineItem {
            idref: id.to_string(),
            id: None,
            linear: true,
            properties: None,
        })
        .collect();
    
    Spine { items, current: 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_spine_basic() {
        let opf = br#"<?xml version="1.0"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0">
  <spine>
    <itemref idref="cover"/>
    <itemref idref="chapter1"/>
    <itemref idref="chapter2"/>
    <itemref idref="chapter3"/>
  </spine>
</package>"#;
        
        let spine = parse_spine(opf).unwrap();
        assert_eq!(spine.len(), 4);
        assert_eq!(spine.get_id(0), Some("cover"));
        assert_eq!(spine.get_id(1), Some("chapter1"));
        assert_eq!(spine.get_id(2), Some("chapter2"));
        assert_eq!(spine.get_id(3), Some("chapter3"));
    }

    #[test]
    fn test_spine_navigation() {
        let mut spine = create_spine(&["a", "b", "c", "d"]);
        
        assert_eq!(spine.position(), 0);
        assert_eq!(spine.current_id(), Some("a"));
        assert!(spine.is_first());
        assert!(!spine.is_last());
        
        assert!(spine.next());
        assert_eq!(spine.position(), 1);
        assert_eq!(spine.current_id(), Some("b"));
        
        assert!(spine.next());
        assert!(spine.next());
        assert_eq!(spine.position(), 3);
        assert!(spine.is_last());
        
        // Can't go past end
        assert!(!spine.next());
        assert_eq!(spine.position(), 3);
        
        // Go back
        assert!(spine.prev());
        assert_eq!(spine.position(), 2);
        
        // Jump to position
        assert!(spine.go_to(0));
        assert_eq!(spine.position(), 0);
        
        // Invalid jump
        assert!(!spine.go_to(100));
        assert_eq!(spine.position(), 0);
    }

    #[test]
    fn test_go_to_id() {
        let mut spine = create_spine(&["cover", "ch1", "ch2"]);
        
        assert!(spine.go_to_id("ch1"));
        assert_eq!(spine.position(), 1);
        
        assert!(!spine.go_to_id("nonexistent"));
        assert_eq!(spine.position(), 1); // Unchanged
    }

    #[test]
    fn test_progress() {
        let mut spine = create_spine(&["a", "b", "c", "d"]);
        
        assert_eq!(spine.progress(), (0, 4));
        assert_eq!(spine.progress_percent(), 0);
        
        spine.go_to(2);
        assert_eq!(spine.progress(), (2, 4));
        assert_eq!(spine.progress_percent(), 50);
        
        spine.go_to(3);
        assert_eq!(spine.progress_percent(), 75);
    }

    #[test]
    fn test_parse_spine_with_attributes() {
        let opf = br#"<?xml version="1.0"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0">
  <spine toc="ncx">
    <itemref idref="cover" id="item-1" linear="yes"/>
    <itemref idref="nav" id="item-2" linear="no" properties="nav"/>
    <itemref idref="chapter1"/>
  </spine>
</package>"#;
        
        let spine = parse_spine(opf).unwrap();
        assert_eq!(spine.len(), 3);
        
        let item0 = spine.get_item(0).unwrap();
        assert_eq!(item0.idref, "cover");
        assert_eq!(item0.id, Some("item-1".to_string()));
        assert!(item0.linear);
        
        let item1 = spine.get_item(1).unwrap();
        assert_eq!(item1.idref, "nav");
        assert_eq!(item1.id, Some("item-2".to_string()));
        assert!(!item1.linear); // linear="no"
        assert_eq!(item1.properties, Some("nav".to_string()));
    }

    #[test]
    fn test_empty_spine() {
        let spine = Spine::new();
        assert!(spine.is_empty());
        assert_eq!(spine.progress_percent(), 0);
        assert!(!spine.next());
        assert!(!spine.prev());
    }

    #[test]
    fn test_chapter_ids() {
        let spine = create_spine(&["a", "b", "c"]);
        let ids = spine.chapter_ids();
        assert_eq!(ids, vec!["a", "b", "c"]);
    }
}
