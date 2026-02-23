//! Integration tests for `FeedBrowserActivity`.

use einked_ereader::{BrowserState, FeedBrowserActivity, OpdsCatalog, OpdsEntry};

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
                size: Some(500_000),
            },
            OpdsEntry {
                id: "2".to_string(),
                title: "Frankenstein".to_string(),
                author: Some("Mary Shelley".to_string()),
                summary: Some("A Gothic novel.".to_string()),
                cover_url: None,
                download_url: Some("https://example.com/frankenstein.epub".to_string()),
                format: Some("application/epub+zip".to_string()),
                size: Some(400_000),
            },
        ],
        links: vec![],
    }
}

#[test]
fn starts_on_source_list() {
    let activity = FeedBrowserActivity::new();
    assert!(matches!(activity.state(), BrowserState::SourceList));
}

#[test]
fn setting_catalog_transitions_to_browsing_state() {
    let mut activity = FeedBrowserActivity::new();
    activity.set_catalog(mock_catalog());
    assert!(matches!(activity.state(), BrowserState::BrowsingCatalog));
}

#[test]
fn error_and_download_state_helpers_work() {
    let mut activity = FeedBrowserActivity::new();
    activity.set_error("network".to_string());
    assert!(matches!(activity.state(), BrowserState::Error(_)));

    activity.set_download_progress(1.4);
    assert!(matches!(activity.state(), BrowserState::Downloading(1.0)));
}
