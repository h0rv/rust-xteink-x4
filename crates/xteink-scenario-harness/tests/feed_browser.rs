//! Integration tests for FeedBrowserActivity.
//!
//! Tests the OPDS feed browser for browsing and downloading ebooks from online catalogs.

use xteink_ui::{
    BrowserState, Button, FeedBrowserActivity, InputEvent, OpdsCatalog, OpdsEntry,
    TestDisplay,
};
use xteink_ui::ui::{Activity, ActivityResult};

fn mock_catalog() -> OpdsCatalog {
    OpdsCatalog {
        title: "Test Catalog".to_string(),
        subtitle: None,
        entries: vec![
            OpdsEntry {
                id: "1".to_string(),
                title: "Pride and Prejudice".to_string(),
                author: Some("Jane Austen".to_string()),
                summary: Some("A classic novel.".to_string()),
                cover_url: None,
                download_url: Some("https://example.com/pride.epub".to_string()),
                format: Some("application/epub+zip".to_string()),
                size: Some(500000),
            },
            OpdsEntry {
                id: "2".to_string(),
                title: "Frankenstein".to_string(),
                author: Some("Mary Shelley".to_string()),
                summary: Some("A Gothic novel.".to_string()),
                cover_url: None,
                download_url: Some("https://example.com/frankenstein.epub".to_string()),
                format: Some("application/epub+zip".to_string()),
                size: Some(400000),
            },
        ],
        links: vec![],
    }
}

fn mock_catalog_with_many_entries() -> OpdsCatalog {
    let mut entries = Vec::new();
    for i in 0..20 {
        entries.push(OpdsEntry {
            id: format!("book-{}", i),
            title: format!("Book Title {}", i + 1),
            author: Some(format!("Author {}", i + 1)),
            summary: Some(format!("Description for book {}.", i + 1)),
            cover_url: None,
            download_url: Some(format!("https://example.com/book{}.epub", i)),
            format: Some("application/epub+zip".to_string()),
            size: Some(300000 + (i as u64 * 10000)),
        });
    }
    OpdsCatalog {
        title: "Large Catalog".to_string(),
        subtitle: Some("A catalog with many books".to_string()),
        entries,
        links: vec![],
    }
}
    OpdsCatalog {
        title: "Large Catalog".to_string(),
        subtitle: Some("A catalog with many books".to_string()),
        entries,
        links: vec![],
    }
}

