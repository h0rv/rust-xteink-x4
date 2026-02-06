//! Integration tests for EPUB functionality
//!
//! These tests verify EPUB parsing, tokenization, and layout without
//! requiring device reflash. Run with:
//!   cargo test -p xteink-ui --features std

extern crate alloc;

use alloc::vec::Vec;
use std::fs::File;
use std::io::Read;

use crate::epub::streaming_zip::StreamingZip;
use crate::epub::metadata::{parse_container_xml, parse_opf, EpubMetadata, ManifestItem};
use crate::epub::spine::{parse_spine, create_spine, Spine};
use crate::epub::tokenizer::{tokenize_html, Token, TokenizeError};
use crate::epub::layout::{LayoutEngine, Page, TextStyle, FontMetrics};

/// Path to the sample EPUB file used for testing
const SAMPLE_EPUB_PATH: &str = "../../../sample_books/Fundamental-Accessibility-Tests-Basic-Functionality-v2.0.0.epub";

// =============================================================================
// StreamingZip Tests
// =============================================================================

#[test]
fn test_zip_open_sample_epub() {
    let file = File::open(SAMPLE_EPUB_PATH).expect("Failed to open sample EPUB");
    let zip = StreamingZip::new(file).expect("Failed to parse ZIP");
    
    // Should find the container.xml file
    assert!(zip.get_entry("META-INF/container.xml").is_some(), 
            "Should find META-INF/container.xml");
    
    // Should have entries
    assert!(zip.num_entries() > 0, "ZIP should have entries");
}

#[test]
fn test_zip_read_container() {
    let file = File::open(SAMPLE_EPUB_PATH).expect("Failed to open sample EPUB");
    let mut zip = StreamingZip::new(file).expect("Failed to parse ZIP");
    
    // Get the container.xml entry
    let entry = zip.get_entry("META-INF/container.xml")
        .expect("container.xml not found")
        .clone();
    
    // Read the content
    let mut buf = vec![0u8; entry.uncompressed_size as usize];
    let bytes_read = zip.read_file(&entry, &mut buf).expect("Failed to read file");
    
    assert!(bytes_read > 0, "Should have read some bytes");
    
    // Verify it's valid XML
    let content = String::from_utf8_lossy(&buf[..bytes_read]);
    assert!(content.contains("container"), "Should contain container element");
    assert!(content.contains("rootfile"), "Should contain rootfile element");
    assert!(content.contains("EPUB/package.opf"), "Should reference package.opf");
}

#[test]
fn test_zip_list_entries() {
    let file = File::open(SAMPLE_EPUB_PATH).expect("Failed to open sample EPUB");
    let zip = StreamingZip::new(file).expect("Failed to parse ZIP");
    
    // Check that we can iterate over entries
    let entries: Vec<_> = zip.entries().collect();
    assert!(!entries.is_empty(), "Should have entries");
    
    // Verify expected files exist
    let filenames: Vec<_> = entries.iter().map(|e| e.filename.as_str()).collect();
    assert!(filenames.contains(&"META-INF/container.xml"), "Should have container.xml");
    assert!(filenames.contains(&"mimetype"), "Should have mimetype");
}

#[test]
fn test_zip_read_package_opf() {
    let file = File::open(SAMPLE_EPUB_PATH).expect("Failed to open sample EPUB");
    let mut zip = StreamingZip::new(file).expect("Failed to parse ZIP");
    
    // Read package.opf
    let entry = zip.get_entry("EPUB/package.opf")
        .expect("package.opf not found")
        .clone();
    
    let mut buf = vec![0u8; entry.uncompressed_size as usize];
    let bytes_read = zip.read_file(&entry, &mut buf).expect("Failed to read file");
    
    let content = String::from_utf8_lossy(&buf[..bytes_read]);
    assert!(content.contains("<package"), "Should be a package OPF");
    assert!(content.contains("<metadata"), "Should have metadata");
    assert!(content.contains("<manifest"), "Should have manifest");
}

#[test]
fn test_zip_entry_not_found() {
    let file = File::open(SAMPLE_EPUB_PATH).expect("Failed to open sample EPUB");
    let zip = StreamingZip::new(file).expect("Failed to parse ZIP");
    
    // Try to get a non-existent file
    assert!(zip.get_entry("nonexistent/file.txt").is_none(),
            "Should return None for missing file");
}

// =============================================================================
// Metadata Tests
// =============================================================================

#[test]
fn test_parse_opf_from_sample() {
    // Read the OPF from the sample EPUB
    let file = File::open(SAMPLE_EPUB_PATH).expect("Failed to open sample EPUB");
    let mut zip = StreamingZip::new(file).expect("Failed to parse ZIP");
    
    let entry = zip.get_entry("EPUB/package.opf")
        .expect("package.opf not found")
        .clone();
    
    let mut buf = vec![0u8; entry.uncompressed_size as usize];
    let bytes_read = zip.read_file(&entry, &mut buf).expect("Failed to read file");
    
    // Parse the OPF
    let metadata = parse_opf(&buf[..bytes_read]).expect("Failed to parse OPF");
    
    // Verify metadata was extracted
    assert!(!metadata.title.is_empty(), "Should have a title");
    assert!(!metadata.author.is_empty(), "Should have an author");
    assert!(!metadata.manifest.is_empty(), "Should have manifest items");
}

#[test]
fn test_manifest_lookup() {
    // Read the OPF from the sample EPUB
    let file = File::open(SAMPLE_EPUB_PATH).expect("Failed to open sample EPUB");
    let mut zip = StreamingZip::new(file).expect("Failed to parse ZIP");
    
    let entry = zip.get_entry("EPUB/package.opf")
        .expect("package.opf not found")
        .clone();
    
    let mut buf = vec![0u8; entry.uncompressed_size as usize];
    let bytes_read = zip.read_file(&entry, &mut buf).expect("Failed to read file");
    
    let metadata = parse_opf(&buf[..bytes_read]).expect("Failed to parse OPF");
    
    // Test getting manifest items by ID
    let cover_item = metadata.get_item("cover");
    assert!(cover_item.is_some(), "Should find cover item");
    
    if let Some(item) = cover_item {
        assert!(item.href.ends_with(".xhtml") || item.href.ends_with(".html"),
                "Cover should be an XHTML file");
        assert_eq!(item.media_type, "application/xhtml+xml",
                   "Cover should have correct media type");
    }
    
    // Test non-existent item
    assert!(metadata.get_item("nonexistent").is_none(),
            "Should return None for missing item");
}

#[test]
fn test_manifest_items_have_valid_properties() {
    let file = File::open(SAMPLE_EPUB_PATH).expect("Failed to open sample EPUB");
    let mut zip = StreamingZip::new(file).expect("Failed to parse ZIP");
    
    let entry = zip.get_entry("EPUB/package.opf")
        .expect("package.opf not found")
        .clone();
    
    let mut buf = vec![0u8; entry.uncompressed_size as usize];
    let bytes_read = zip.read_file(&entry, &mut buf).expect("Failed to read file");
    
    let metadata = parse_opf(&buf[..bytes_read]).expect("Failed to parse OPF");
    
    // Verify all manifest items have required properties
    for item in &metadata.manifest {
        assert!(!item.id.is_empty(), "Manifest item should have an ID");
        assert!(!item.href.is_empty(), "Manifest item should have an href");
        assert!(!item.media_type.is_empty(), "Manifest item should have a media type");
    }
}

#[test]
fn test_find_item_by_href() {
    let file = File::open(SAMPLE_EPUB_PATH).expect("Failed to open sample EPUB");
    let mut zip = StreamingZip::new(file).expect("Failed to parse ZIP");
    
    let entry = zip.get_entry("EPUB/package.opf")
        .expect("package.opf not found")
        .clone();
    
    let mut buf = vec![0u8; entry.uncompressed_size as usize];
    let bytes_read = zip.read_file(&entry, &mut buf).expect("Failed to read file");
    
    let metadata = parse_opf(&buf[..bytes_read]).expect("Failed to parse OPF");
    
    // Find by href should work for existing items
    if let Some(first_item) = metadata.manifest.first() {
        let found_id = metadata.find_item_by_href(&first_item.href);
        assert!(found_id.is_some(), "Should find item by href");
        assert_eq!(found_id.unwrap(), first_item.id, "Found ID should match");
    }
    
    // Non-existent href should return None
    assert!(metadata.find_item_by_href("nonexistent.xhtml").is_none());
}

// =============================================================================
// Tokenizer Tests
// =============================================================================

#[test]
fn test_tokenize_simple_html() {
    let html = "<p>Hello <em>world</em></p>";
    let tokens = tokenize_html(html).expect("Failed to tokenize");
    
    // Expected: Text("Hello"), Emphasis(true), Text("world"), Emphasis(false), ParagraphBreak
    assert_eq!(tokens.len(), 5, "Should have 5 tokens");
    assert!(matches!(tokens[0], Token::Text(ref t) if t == "Hello"));
    assert_eq!(tokens[1], Token::Emphasis(true));
    assert!(matches!(tokens[2], Token::Text(ref t) if t == "world"));
    assert_eq!(tokens[3], Token::Emphasis(false));
    assert_eq!(tokens[4], Token::ParagraphBreak);
}

#[test]
fn test_tokenize_chapter_from_sample() {
    // Read a chapter from the sample EPUB
    let file = File::open(SAMPLE_EPUB_PATH).expect("Failed to open sample EPUB");
    let mut zip = StreamingZip::new(file).expect("Failed to parse ZIP");
    
    // Find and read a chapter file
    let chapter_entry = zip.entries()
        .find(|e| e.filename.contains("Basic-functionality-tests.xhtml"))
        .expect("Chapter not found")
        .clone();
    
    let mut buf = vec![0u8; chapter_entry.uncompressed_size as usize];
    let bytes_read = zip.read_file(&chapter_entry, &mut buf).expect("Failed to read file");
    
    let html = String::from_utf8_lossy(&buf[..bytes_read]);
    let tokens = tokenize_html(&html).expect("Failed to tokenize chapter");
    
    // Should produce meaningful tokens
    assert!(!tokens.is_empty(), "Should have tokens");
    
    // Should have some text tokens
    let text_tokens: Vec<_> = tokens.iter()
        .filter(|t| matches!(t, Token::Text(_)))
        .collect();
    assert!(!text_tokens.is_empty(), "Should have text tokens");
}

#[test]
fn test_tokenize_with_headings() {
    let html = "<h1>Title</h1><p>Content</p>";
    let tokens = tokenize_html(html).expect("Failed to tokenize");
    
    assert!(tokens.iter().any(|t| matches!(t, Token::Heading(1))),
            "Should have h1 heading");
    assert!(tokens.iter().any(|t| matches!(t, Token::Text(ref txt) if txt == "Title")));
    assert!(tokens.iter().any(|t| matches!(t, Token::Text(ref txt) if txt == "Content")));
}

#[test]
fn test_tokenize_complex_formatting() {
    let html = r#"<p>Normal <strong>bold <em>bold+italic</em> bold</strong> normal</p>"#;
    let tokens = tokenize_html(html).expect("Failed to tokenize");
    
    // Verify the token sequence is correct
    let mut found_bold_start = false;
    let mut found_italic_start = false;
    let mut found_italic_end = false;
    let mut found_bold_end = false;
    
    for token in &tokens {
        match token {
            Token::Strong(true) => found_bold_start = true,
            Token::Emphasis(true) => found_italic_start = true,
            Token::Emphasis(false) => found_italic_end = true,
            Token::Strong(false) => found_bold_end = true,
            _ => {}
        }
    }
    
    assert!(found_bold_start, "Should have bold start");
    assert!(found_italic_start, "Should have italic start");
    assert!(found_italic_end, "Should have italic end");
    assert!(found_bold_end, "Should have bold end");
}

// =============================================================================
// Layout Tests
// =============================================================================

#[test]
fn test_layout_single_page() {
    // Short text that should fit on one page
    let tokens = vec![
        Token::Text("Short text.".to_string()),
        Token::ParagraphBreak,
    ];
    
    let mut engine = LayoutEngine::new(460.0, 650.0, 20.0);
    let pages = engine.layout_tokens(tokens);
    
    assert_eq!(pages.len(), 1, "Short text should fit on one page");
    assert_eq!(pages[0].page_number, 1);
    assert!(!pages[0].is_empty(), "Page should have content");
}

#[test]
fn test_pagination() {
    // Create enough text to force pagination
    let mut tokens = Vec::new();
    for i in 0..100 {
        tokens.push(Token::Text(format!("This is paragraph {} with enough text to fill some space. ", i)));
        tokens.push(Token::Text("Here is additional text to make the paragraph longer. ".to_string()));
        tokens.push(Token::Text("And even more content to ensure proper pagination testing.".to_string()));
        tokens.push(Token::ParagraphBreak);
    }
    
    let mut engine = LayoutEngine::new(460.0, 300.0, 20.0); // Small page height
    let pages = engine.layout_tokens(tokens);
    
    assert!(pages.len() > 1, "Should have multiple pages");
    
    // Page numbers should be sequential
    for (i, page) in pages.iter().enumerate() {
        assert_eq!(page.page_number, i + 1, "Page numbers should be sequential");
    }
}

#[test]
fn test_layout_with_formatting() {
    let tokens = vec![
        Token::Text("Normal ".to_string()),
        Token::Strong(true),
        Token::Text("bold".to_string()),
        Token::Strong(false),
        Token::Text(" text.".to_string()),
        Token::ParagraphBreak,
    ];
    
    let mut engine = LayoutEngine::new(460.0, 650.0, 20.0);
    let pages = engine.layout_tokens(tokens);
    
    assert!(!pages.is_empty(), "Should have pages");
    assert!(!pages[0].is_empty(), "Should have lines");
}

#[test]
fn test_layout_headings() {
    let tokens = vec![
        Token::Heading(1),
        Token::Text("Chapter Title".to_string()),
        Token::ParagraphBreak,
        Token::Text("Chapter content here.".to_string()),
        Token::ParagraphBreak,
    ];
    
    let mut engine = LayoutEngine::new(460.0, 650.0, 20.0);
    let pages = engine.layout_tokens(tokens);
    
    assert!(!pages.is_empty(), "Should have pages");
    
    // Check that the heading text appears
    let all_text: String = pages.iter()
        .flat_map(|p| &p.lines)
        .map(|l| l.text.as_str())
        .collect();
    
    assert!(all_text.contains("Chapter Title"), "Should contain heading text");
    assert!(all_text.contains("Chapter content"), "Should contain content text");
}

#[test]
fn test_layout_line_breaking() {
    // Create a long line that must wrap
    let long_text = "a".repeat(200); // Very long string
    let tokens = vec![
        Token::Text(long_text),
        Token::ParagraphBreak,
    ];
    
    let mut engine = LayoutEngine::new(100.0, 200.0, 20.0); // Narrow page
    let pages = engine.layout_tokens(tokens);
    
    assert!(!pages.is_empty(), "Should have pages");
    
    // Should have multiple lines due to wrapping
    let total_lines: usize = pages.iter().map(|p| p.line_count()).sum();
    assert!(total_lines > 1, "Long text should wrap to multiple lines");
}

// =============================================================================
// Integration Tests
// =============================================================================

/// Integration test: Load EPUB, extract metadata, parse spine, tokenize chapter, and layout
#[test]
fn test_epub_full_pipeline() {
    // Step 1: Open EPUB
    let file = File::open(SAMPLE_EPUB_PATH).expect("Failed to open sample EPUB");
    let mut zip = StreamingZip::new(file).expect("Failed to parse ZIP");
    
    // Step 2: Read and parse OPF
    let opf_entry = zip.get_entry("EPUB/package.opf")
        .expect("package.opf not found")
        .clone();
    let mut opf_buf = vec![0u8; opf_entry.uncompressed_size as usize];
    let opf_bytes = zip.read_file(&opf_entry, &mut opf_buf).expect("Failed to read OPF");
    
    let metadata = parse_opf(&opf_buf[..opf_bytes]).expect("Failed to parse OPF");
    
    // Verify metadata
    assert!(!metadata.title.is_empty(), "Should have title");
    assert!(!metadata.manifest.is_empty(), "Should have manifest");
    
    // Step 3: Parse spine
    let spine = parse_spine(&opf_buf[..opf_bytes]).expect("Failed to parse spine");
    assert!(!spine.is_empty(), "Should have spine items");
    
    // Step 4: Find first chapter and tokenize it
    if let Some(first_chapter_id) = spine.current_id() {
        // Find the chapter in the manifest
        let chapter_item = metadata.get_item(first_chapter_id)
            .expect("Chapter not in manifest");
        
        // Build the full path to the chapter
        let chapter_path = if chapter_item.href.starts_with("xhtml/") || 
                              chapter_item.href.starts_with("EPUB/xhtml/") {
            if chapter_item.href.starts_with("EPUB/") {
                chapter_item.href.clone()
            } else {
                format!("EPUB/{}", chapter_item.href)
            }
        } else {
            format!("EPUB/xhtml/{}", chapter_item.href)
        };
        
        // Read and tokenize the chapter
        if let Some(chapter_entry) = zip.get_entry(&chapter_path) {
            let entry = chapter_entry.clone();
            let mut chapter_buf = vec![0u8; entry.uncompressed_size as usize];
            let chapter_bytes = zip.read_file(&entry, &mut chapter_buf)
                .expect("Failed to read chapter");
            
            let html = String::from_utf8_lossy(&chapter_buf[..chapter_bytes]);
            let tokens = tokenize_html(&html).expect("Failed to tokenize");
            
            // Step 5: Layout the tokens
            let mut engine = LayoutEngine::with_defaults();
            let pages = engine.layout_tokens(tokens);
            
            assert!(!pages.is_empty(), "Should have at least one page");
            
            // Verify pages have content
            let total_lines: usize = pages.iter().map(|p| p.line_count()).sum();
            assert!(total_lines > 0, "Should have content lines");
        }
    }
}

/// Test that we can navigate through the spine of the sample EPUB
#[test]
fn test_spine_navigation_integration() {
    let file = File::open(SAMPLE_EPUB_PATH).expect("Failed to open sample EPUB");
    let mut zip = StreamingZip::new(file).expect("Failed to parse ZIP");
    
    let opf_entry = zip.get_entry("EPUB/package.opf")
        .expect("package.opf not found")
        .clone();
    let mut opf_buf = vec![0u8; opf_entry.uncompressed_size as usize];
    let opf_bytes = zip.read_file(&opf_entry, &mut opf_buf).expect("Failed to read OPF");
    
    let mut spine = parse_spine(&opf_buf[..opf_bytes]).expect("Failed to parse spine");
    
    let initial_position = spine.position();
    assert_eq!(initial_position, 0, "Should start at position 0");
    
    // Navigate forward
    let chapter_count = spine.len();
    if chapter_count > 1 {
        assert!(spine.next(), "Should be able to go to next chapter");
        assert_eq!(spine.position(), 1);
        
        // Go back
        assert!(spine.prev(), "Should be able to go to previous chapter");
        assert_eq!(spine.position(), 0);
    }
    
    // Check progress
    let (current, total) = spine.progress();
    assert_eq!(current, 0);
    assert_eq!(total, chapter_count);
}

/// Test reading cover image metadata
#[test]
fn test_cover_detection() {
    let file = File::open(SAMPLE_EPUB_PATH).expect("Failed to open sample EPUB");
    let mut zip = StreamingZip::new(file).expect("Failed to parse ZIP");
    
    let opf_entry = zip.get_entry("EPUB/package.opf")
        .expect("package.opf not found")
        .clone();
    let mut opf_buf = vec![0u8; opf_entry.uncompressed_size as usize];
    let opf_bytes = zip.read_file(&opf_entry, &mut opf_buf).expect("Failed to read OPF");
    
    let metadata = parse_opf(&opf_buf[..opf_bytes]).expect("Failed to parse OPF");
    
    // Check if there's a cover image in the manifest
    let cover_items: Vec<_> = metadata.manifest.iter()
        .filter(|item| {
            item.id.to_lowercase().contains("cover") ||
            item.properties.as_ref()
                .map_or(false, |p| p.contains("cover-image"))
        })
        .collect();
    
    // The sample EPUB should have a cover
    assert!(!cover_items.is_empty(), "Should have cover-related items");
}

/// Test that all spine items have corresponding manifest entries
#[test]
fn test_spine_manifest_consistency() {
    let file = File::open(SAMPLE_EPUB_PATH).expect("Failed to open sample EPUB");
    let mut zip = StreamingZip::new(file).expect("Failed to parse ZIP");
    
    let opf_entry = zip.get_entry("EPUB/package.opf")
        .expect("package.opf not found")
        .clone();
    let mut opf_buf = vec![0u8; opf_entry.uncompressed_size as usize];
    let opf_bytes = zip.read_file(&opf_entry, &mut opf_buf).expect("Failed to read OPF");
    
    let metadata = parse_opf(&opf_buf[..opf_bytes]).expect("Failed to parse OPF");
    let spine = parse_spine(&opf_buf[..opf_bytes]).expect("Failed to parse spine");
    
    // Every spine item should reference a manifest item
    for item in &spine.items {
        let manifest_item = metadata.get_item(&item.idref);
        assert!(
            manifest_item.is_some(),
            "Spine item '{}' should have corresponding manifest entry",
            item.idref
        );
    }
}

/// Test reading all chapters from the EPUB
#[test]
fn test_read_all_chapters() {
    let file = File::open(SAMPLE_EPUB_PATH).expect("Failed to open sample EPUB");
    let mut zip = StreamingZip::new(file).expect("Failed to parse ZIP");
    
    let opf_entry = zip.get_entry("EPUB/package.opf")
        .expect("package.opf not found")
        .clone();
    let mut opf_buf = vec![0u8; opf_entry.uncompressed_size as usize];
    let opf_bytes = zip.read_file(&opf_entry, &mut opf_buf).expect("Failed to read OPF");
    
    let metadata = parse_opf(&opf_buf[..opf_bytes]).expect("Failed to parse OPF");
    let spine = parse_spine(&opf_buf[..opf_bytes]).expect("Failed to parse spine");
    
    let mut chapters_read = 0;
    
    for spine_item in &spine.items {
        if let Some(manifest_item) = metadata.get_item(&spine_item.idref) {
            // Build chapter path
            let chapter_path = if manifest_item.href.starts_with("xhtml/") {
                format!("EPUB/{}", manifest_item.href)
            } else if manifest_item.href.starts_with("EPUB/") {
                manifest_item.href.clone()
            } else {
                format!("EPUB/xhtml/{}", manifest_item.href)
            };
            
            // Try to read the chapter
            if let Some(entry) = zip.get_entry(&chapter_path) {
                let entry = entry.clone();
                let mut buf = vec![0u8; entry.uncompressed_size as usize];
                if zip.read_file(&entry, &mut buf).is_ok() {
                    chapters_read += 1;
                    
                    // Verify it's valid XHTML
                    let content = String::from_utf8_lossy(&buf);
                    assert!(
                        content.contains("<html") || content.contains("<body"),
                        "Chapter should be valid HTML/XHTML"
                    );
                }
            }
        }
    }
    
    assert!(chapters_read > 0, "Should be able to read at least one chapter");
    assert_eq!(chapters_read, spine.len(), "Should read all chapters in spine");
}
