//! XHTML to token stream converter for EPUB content
//!
//! Converts XHTML chapters into a simplified token format that's easier
//! to layout. Uses quick_xml for SAX-style parsing to handle large
//! documents efficiently without loading the entire DOM.

extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use quick_xml::events::Event;
use quick_xml::reader::Reader;

/// Token types for simplified XHTML representation
#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    /// Plain text content
    Text(String),
    /// New paragraph break
    ParagraphBreak,
    /// Heading with level 1-6
    Heading(u8),
    /// Start (true) or end (false) of italic emphasis
    Emphasis(bool),
    /// Start (true) or end (false) of bold strong
    Strong(bool),
    /// Line break (<br>)
    LineBreak,
}

/// Error type for tokenization failures
#[derive(Clone, Debug, PartialEq)]
pub enum TokenizeError {
    /// XML parsing error
    ParseError(String),
    /// Invalid HTML structure
    InvalidStructure(String),
}

impl core::fmt::Display for TokenizeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            TokenizeError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            TokenizeError::InvalidStructure(msg) => write!(f, "Invalid structure: {}", msg),
        }
    }
}

/// Convert XHTML string into a token stream
///
/// Parses HTML tags: p, h1-h6, em, strong, br, span, div
/// Strips out: script, style, head, attributes (except class for styling)
/// Extracts text content and converts HTML entities
///
/// # Example
/// ```
/// use xteink_ui::epub::tokenizer::{tokenize_html, Token};
///
/// let html = "<p>Hello <em>world</em></p>";
/// let tokens = tokenize_html(html).unwrap();
/// ```
pub fn tokenize_html(html: &str) -> Result<Vec<Token>, TokenizeError> {
    let mut reader = Reader::from_str(html);
    reader.config_mut().trim_text(false);
    // Enable entity expansion (converts &lt; to <, &amp; to &, etc.)
    reader.config_mut().expand_empty_elements = false;

    let mut buf = Vec::new();
    let mut tokens = Vec::new();

    // Stack to track nested elements for proper closing
    let mut element_stack: Vec<ElementType> = Vec::new();
    // Track if we're inside a tag that should be skipped (script, style, head)
    let mut skip_depth: usize = 0;
    // Track if we need a paragraph break after current block element
    let mut pending_paragraph_break: bool = false;
    // Track if we need a heading close after text content
    let mut pending_heading_close: Option<u8> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = decode_name(e.name().as_ref(), &reader)?;

                // Check if we should skip this element and its children
                if should_skip_element(&name) {
                    skip_depth += 1;
                    continue;
                }

                // If skipping, don't process anything
                if skip_depth > 0 {
                    continue;
                }

                // Flush any pending paragraph break from previous block
                if pending_paragraph_break && !tokens.is_empty() {
                    tokens.push(Token::ParagraphBreak);
                    pending_paragraph_break = false;
                }

                // Flush any pending heading close
                if let Some(level) = pending_heading_close.take() {
                    tokens.push(Token::Heading(level));
                    pending_paragraph_break = true;
                }

                match name.as_str() {
                    "p" | "div" => {
                        element_stack.push(ElementType::Paragraph);
                    }
                    "span" => {
                        element_stack.push(ElementType::Span);
                    }
                    h if h.starts_with('h') && h.len() == 2 => {
                        if let Some(level) = h.chars().nth(1).and_then(|c| c.to_digit(10)) {
                            if level >= 1 && level <= 6 {
                                element_stack.push(ElementType::Heading(level as u8));
                                pending_heading_close = Some(level as u8);
                            }
                        }
                    }
                    "em" | "i" => {
                        element_stack.push(ElementType::Emphasis);
                        tokens.push(Token::Emphasis(true));
                    }
                    "strong" | "b" => {
                        element_stack.push(ElementType::Strong);
                        tokens.push(Token::Strong(true));
                    }
                    _ => {
                        // Unknown element, treat as generic container
                        element_stack.push(ElementType::Generic);
                    }
                }
            }
            Ok(Event::Text(e)) => {
                // Skip text if we're inside a script/style/head block
                if skip_depth > 0 {
                    continue;
                }

                let text = reader
                    .decoder()
                    .decode(&e)
                    .map_err(|e| TokenizeError::ParseError(format!("Decode error: {:?}", e)))?
                    .to_string();

                // Normalize whitespace: collapse multiple spaces/newlines
                let normalized = normalize_whitespace(&text);

                if !normalized.is_empty() {
                    // Flush any pending heading close
                    if let Some(level) = pending_heading_close.take() {
                        tokens.push(Token::Heading(level));
                    }
                    tokens.push(Token::Text(normalized));
                }
            }
            Ok(Event::End(e)) => {
                let name = decode_name(e.name().as_ref(), &reader)?;

                // Check if we're ending a skip element
                if should_skip_element(&name) {
                    if skip_depth > 0 {
                        skip_depth -= 1;
                    }
                    continue;
                }

                // If skipping, don't process end tags
                if skip_depth > 0 {
                    continue;
                }

                // Pop the element from stack and emit appropriate close token
                if let Some(element) = element_stack.pop() {
                    match element {
                        ElementType::Paragraph => {
                            pending_paragraph_break = true;
                        }
                        ElementType::Heading(level) => {
                            // Heading already emitted on start, just mark for paragraph break
                            pending_paragraph_break = true;
                            // Clear any pending close since we already handled it
                            pending_heading_close = None;
                        }
                        ElementType::Emphasis => {
                            tokens.push(Token::Emphasis(false));
                        }
                        ElementType::Strong => {
                            tokens.push(Token::Strong(false));
                        }
                        ElementType::Span | ElementType::Generic => {
                            // No tokens needed for these
                        }
                    }
                }
            }
            Ok(Event::Empty(e)) => {
                let name = decode_name(e.name().as_ref(), &reader)?;

                // Skip empty elements inside script/style blocks
                if skip_depth > 0 {
                    continue;
                }

                // Flush any pending paragraph break
                if pending_paragraph_break && !tokens.is_empty() {
                    tokens.push(Token::ParagraphBreak);
                    pending_paragraph_break = false;
                }

                // Flush any pending heading close
                if let Some(level) = pending_heading_close.take() {
                    tokens.push(Token::Heading(level));
                    pending_paragraph_break = true;
                }

                match name.as_str() {
                    "br" => {
                        tokens.push(Token::LineBreak);
                    }
                    "p" | "div" => {
                        // Empty paragraph still creates a paragraph break
                        pending_paragraph_break = true;
                    }
                    h if h.starts_with('h') && h.len() == 2 => {
                        if let Some(level) = h.chars().nth(1).and_then(|c| c.to_digit(10)) {
                            if level >= 1 && level <= 6 {
                                // Empty heading - just emit the heading token
                                tokens.push(Token::Heading(level as u8));
                                pending_paragraph_break = true;
                            }
                        }
                    }
                    _ => {
                        // Other empty elements are ignored
                    }
                }
            }
            Ok(Event::CData(e)) => {
                // CDATA content is treated as raw text
                if skip_depth == 0 {
                    let text = reader
                        .decoder()
                        .decode(&e)
                        .map_err(|e| TokenizeError::ParseError(format!("Decode error: {:?}", e)))?
                        .to_string();

                    let normalized = normalize_whitespace(&text);
                    if !normalized.is_empty() {
                        if let Some(level) = pending_heading_close.take() {
                            tokens.push(Token::Heading(level));
                        }
                        tokens.push(Token::Text(normalized));
                    }
                }
            }
            Ok(Event::Comment(_)) => {
                // Comments are ignored
            }
            Ok(Event::Decl(_)) => {
                // XML declaration is ignored
            }
            Ok(Event::PI(_)) => {
                // Processing instructions are ignored
            }
            Ok(Event::DocType(_)) => {
                // DOCTYPE is ignored
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(TokenizeError::ParseError(format!("XML error: {:?}", e)));
            }
            _ => {}
        }
        buf.clear();
    }

    // Flush any remaining pending paragraph break
    if pending_paragraph_break && !tokens.is_empty() {
        // Don't add trailing paragraph break
        // tokens.push(Token::ParagraphBreak);
    }

    // Close any unclosed formatting tags
    while let Some(element) = element_stack.pop() {
        match element {
            ElementType::Emphasis => {
                tokens.push(Token::Emphasis(false));
            }
            ElementType::Strong => {
                tokens.push(Token::Strong(false));
            }
            ElementType::Paragraph | ElementType::Heading(_) => {
                // These already handled via pending_paragraph_break
            }
            _ => {}
        }
    }

    // Flush any pending heading close
    if let Some(level) = pending_heading_close {
        tokens.push(Token::Heading(level));
    }

    Ok(tokens)
}

/// Types of elements we track in the stack
#[derive(Clone, Copy, Debug, PartialEq)]
enum ElementType {
    Paragraph,
    Heading(u8),
    Emphasis,
    Strong,
    Span,
    Generic,
}

/// Check if an element should be skipped entirely (with its children)
fn should_skip_element(name: &str) -> bool {
    matches!(
        name,
        "script" | "style" | "head" | "nav" | "header" | "footer" | "aside" | "noscript"
    )
}

/// Normalize whitespace in text content
/// Collapses multiple spaces/newlines and trims ends
fn normalize_whitespace(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut prev_was_space = true; // Start true to trim leading whitespace

    for ch in text.chars() {
        if ch.is_whitespace() {
            if !prev_was_space {
                result.push(' ');
                prev_was_space = true;
            }
        } else {
            result.push(ch);
            prev_was_space = false;
        }
    }

    // Trim trailing space if present
    if result.ends_with(' ') {
        result.pop();
    }

    result
}

/// Decode element name from bytes
fn decode_name(name: &[u8], reader: &Reader<&[u8]>) -> Result<String, TokenizeError> {
    reader
        .decoder()
        .decode(name)
        .map_err(|e| TokenizeError::ParseError(format!("Decode error: {:?}", e)))
        .map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn test_tokenize_simple_paragraph() {
        let html = "<p>Hello world</p>";
        let tokens = tokenize_html(html).unwrap();

        assert_eq!(
            tokens,
            vec![
                Token::Text("Hello world".to_string()),
                Token::ParagraphBreak
            ]
        );
    }

    #[test]
    fn test_tokenize_emphasis() {
        let html = "<p>This is <em>italic</em> and <strong>bold</strong> text.</p>";
        let tokens = tokenize_html(html).unwrap();

        assert_eq!(
            tokens,
            vec![
                Token::Text("This is ".to_string()),
                Token::Emphasis(true),
                Token::Text("italic".to_string()),
                Token::Emphasis(false),
                Token::Text(" and ".to_string()),
                Token::Strong(true),
                Token::Text("bold".to_string()),
                Token::Strong(false),
                Token::Text(" text.".to_string()),
                Token::ParagraphBreak,
            ]
        );
    }

    #[test]
    fn test_tokenize_heading_and_paragraphs() {
        let html = "<h1>Chapter Title</h1><p>First paragraph.</p><p>Second paragraph.</p>";
        let tokens = tokenize_html(html).unwrap();

        assert_eq!(
            tokens,
            vec![
                Token::Heading(1),
                Token::Text("Chapter Title".to_string()),
                Token::ParagraphBreak,
                Token::Text("First paragraph.".to_string()),
                Token::ParagraphBreak,
                Token::Text("Second paragraph.".to_string()),
                Token::ParagraphBreak,
            ]
        );
    }

    #[test]
    fn test_tokenize_multiple_headings() {
        let html = "<h1>Title</h1><h2>Subtitle</h2><h3>Section</h3>";
        let tokens = tokenize_html(html).unwrap();

        assert_eq!(
            tokens,
            vec![
                Token::Heading(1),
                Token::Text("Title".to_string()),
                Token::ParagraphBreak,
                Token::Heading(2),
                Token::Text("Subtitle".to_string()),
                Token::ParagraphBreak,
                Token::Heading(3),
                Token::Text("Section".to_string()),
                Token::ParagraphBreak,
            ]
        );
    }

    #[test]
    fn test_tokenize_line_break() {
        let html = "<p>Line one<br>Line two</p>";
        let tokens = tokenize_html(html).unwrap();

        assert_eq!(
            tokens,
            vec![
                Token::Text("Line one".to_string()),
                Token::LineBreak,
                Token::Text("Line two".to_string()),
                Token::ParagraphBreak,
            ]
        );
    }

    #[test]
    fn test_tokenize_nested_formatting() {
        let html = "<p>Text with <strong>bold and <em>italic nested</em></strong>.</p>";
        let tokens = tokenize_html(html).unwrap();

        assert_eq!(
            tokens,
            vec![
                Token::Text("Text with ".to_string()),
                Token::Strong(true),
                Token::Text("bold and ".to_string()),
                Token::Emphasis(true),
                Token::Text("italic nested".to_string()),
                Token::Emphasis(false),
                Token::Strong(false),
                Token::Text(".".to_string()),
                Token::ParagraphBreak,
            ]
        );
    }

    #[test]
    fn test_strip_script_and_style() {
        let html = r#"<p>Visible text</p><script>alert("hidden");</script><style>.hidden{}</style><p>More visible</p>"#;
        let tokens = tokenize_html(html).unwrap();

        assert_eq!(
            tokens,
            vec![
                Token::Text("Visible text".to_string()),
                Token::ParagraphBreak,
                Token::Text("More visible".to_string()),
                Token::ParagraphBreak,
            ]
        );
    }

    #[test]
    fn test_strip_head() {
        let html = "<head><title>Title</title></head><body><p>Content</p></body>";
        let tokens = tokenize_html(html).unwrap();

        assert_eq!(
            tokens,
            vec![Token::Text("Content".to_string()), Token::ParagraphBreak,]
        );
    }

    #[test]
    fn test_whitespace_normalization() {
        let html = "<p>  Multiple   spaces   and\n\nnewlines  </p>";
        let tokens = tokenize_html(html).unwrap();

        assert_eq!(
            tokens,
            vec![
                Token::Text("Multiple spaces and newlines".to_string()),
                Token::ParagraphBreak,
            ]
        );
    }

    #[test]
    fn test_empty_paragraph() {
        let html = "<p></p>";
        let tokens = tokenize_html(html).unwrap();

        // Empty paragraph should still produce a paragraph break
        assert_eq!(tokens, vec![Token::ParagraphBreak]);
    }

    #[test]
    fn test_unclosed_tags() {
        let html = "<p>Text with <em>italic</p>";
        let tokens = tokenize_html(html).unwrap();

        // Should auto-close unclosed tags
        assert_eq!(
            tokens,
            vec![
                Token::Text("Text with ".to_string()),
                Token::Emphasis(true),
                Token::Text("italic".to_string()),
                Token::Emphasis(false),
                Token::ParagraphBreak,
            ]
        );
    }

    #[test]
    fn test_b_and_i_tags() {
        let html = "<p><b>bold</b> and <i>italic</i></p>";
        let tokens = tokenize_html(html).unwrap();

        assert_eq!(
            tokens,
            vec![
                Token::Strong(true),
                Token::Text("bold".to_string()),
                Token::Strong(false),
                Token::Text(" and ".to_string()),
                Token::Emphasis(true),
                Token::Text("italic".to_string()),
                Token::Emphasis(false),
                Token::ParagraphBreak,
            ]
        );
    }

    #[test]
    fn test_div_handling() {
        let html = "<div>Block content</div><div>Another block</div>";
        let tokens = tokenize_html(html).unwrap();

        assert_eq!(
            tokens,
            vec![
                Token::Text("Block content".to_string()),
                Token::ParagraphBreak,
                Token::Text("Another block".to_string()),
                Token::ParagraphBreak,
            ]
        );
    }

    #[test]
    fn test_span_handling() {
        let html = "<p>Text with <span>spanned</span> content</p>";
        let tokens = tokenize_html(html).unwrap();

        assert_eq!(
            tokens,
            vec![
                Token::Text("Text with ".to_string()),
                Token::Text("spanned".to_string()),
                Token::Text(" content".to_string()),
                Token::ParagraphBreak,
            ]
        );
    }

    #[test]
    fn test_example_from_spec() {
        let html = r#"<p>This is <em>italic</em> and <strong>bold</strong> text.</p>
<h1>Chapter Title</h1>
<p>Another paragraph.</p>"#;

        let tokens = tokenize_html(html).unwrap();

        let expected = vec![
            Token::Text("This is ".to_string()),
            Token::Emphasis(true),
            Token::Text("italic".to_string()),
            Token::Emphasis(false),
            Token::Text(" and ".to_string()),
            Token::Strong(true),
            Token::Text("bold".to_string()),
            Token::Strong(false),
            Token::Text(" text.".to_string()),
            Token::ParagraphBreak,
            Token::Heading(1),
            Token::Text("Chapter Title".to_string()),
            Token::ParagraphBreak,
            Token::Text("Another paragraph.".to_string()),
            Token::ParagraphBreak,
        ];

        assert_eq!(tokens, expected);
    }

    #[test]
    fn test_all_heading_levels() {
        let html = "<h1>H1</h1><h2>H2</h2><h3>H3</h3><h4>H4</h4><h5>H5</h5><h6>H6</h6>";
        let tokens = tokenize_html(html).unwrap();

        assert_eq!(
            tokens,
            vec![
                Token::Heading(1),
                Token::Text("H1".to_string()),
                Token::ParagraphBreak,
                Token::Heading(2),
                Token::Text("H2".to_string()),
                Token::ParagraphBreak,
                Token::Heading(3),
                Token::Text("H3".to_string()),
                Token::ParagraphBreak,
                Token::Heading(4),
                Token::Text("H4".to_string()),
                Token::ParagraphBreak,
                Token::Heading(5),
                Token::Text("H5".to_string()),
                Token::ParagraphBreak,
                Token::Heading(6),
                Token::Text("H6".to_string()),
                Token::ParagraphBreak,
            ]
        );
    }
}
