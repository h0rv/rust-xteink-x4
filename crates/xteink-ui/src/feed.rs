//! OPDS (Open Publication Distribution System) and RSS feed data structures.
//!
//! Used for browsing and downloading ebooks from online catalogs (OPDS)
//! and reading articles from RSS/Atom feeds.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

/// Type of feed source
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedType {
    /// OPDS catalog - downloadable ebooks
    Opds,
    /// RSS/Atom feed - articles to read
    Rss,
}

/// A feed source (OPDS catalog or RSS feed)
#[derive(Debug, Clone)]
pub struct FeedSource {
    pub name: String,
    pub url: String,
    pub feed_type: FeedType,
}

impl FeedSource {
    pub const fn new(_name: &'static str, _url: &'static str, _feed_type: FeedType) -> Self {
        Self {
            name: String::new(),
            url: String::new(),
            feed_type: FeedType::Opds,
        }
    }
}

/// An OPDS catalog (parsed from Atom feed)
#[derive(Debug, Clone)]
pub struct OpdsCatalog {
    pub title: String,
    pub subtitle: Option<String>,
    pub entries: Vec<OpdsEntry>,
    /// Links to sub-catalogs (navigation)
    pub links: Vec<OpdsLink>,
}

/// A book entry in an OPDS catalog
#[derive(Debug, Clone)]
pub struct OpdsEntry {
    pub id: String,
    pub title: String,
    pub author: Option<String>,
    pub summary: Option<String>,
    /// Link to cover image
    pub cover_url: Option<String>,
    /// Link to download the book
    pub download_url: Option<String>,
    /// File format (e.g., "application/epub+zip")
    pub format: Option<String>,
    /// File size in bytes (if known)
    pub size: Option<u64>,
}

/// A navigation link in an OPDS catalog
#[derive(Debug, Clone)]
pub struct OpdsLink {
    pub href: String,
    pub rel: String,
    pub title: Option<String>,
}

/// Preloaded OPDS sources (ebook catalogs)
pub const PRELOADED_OPDS_SOURCES: &[(&str, &str)] = &[
    ("Project Gutenberg", "https://m.gutenberg.org/ebooks.opds/"),
    ("Standard Ebooks", "https://standardebooks.org/feeds/opds"),
    (
        "Feedbooks (Public Domain)",
        "https://catalog.feedbooks.com/catalog/public_domain.atom",
    ),
];

/// Preloaded RSS/Atom sources (article feeds)
pub const PRELOADED_RSS_SOURCES: &[(&str, &str)] = &[
    ("Hacker News", "https://news.ycombinator.com/rss"),
    ("Hacker News (Front Page)", "https://hnrss.org/frontpage"),
    ("Longform", "https://longform.org/rss/"),
];

/// All preloaded sources (OPDS + RSS)
pub fn all_preloaded_sources() -> Vec<(&'static str, &'static str, FeedType)> {
    let mut sources = Vec::new();
    for (name, url) in PRELOADED_OPDS_SOURCES {
        sources.push((*name, *url, FeedType::Opds));
    }
    for (name, url) in PRELOADED_RSS_SOURCES {
        sources.push((*name, *url, FeedType::Rss));
    }
    sources
}

/// Jina.ai Reader API endpoint for content extraction
/// Usage: https://r.jina.ai/{url} returns clean text/markdown
pub const JINA_READER_BASE: &str = "https://r.jina.ai/";

/// Extract article content using Jina.ai Reader
pub fn get_reader_url(article_url: &str) -> String {
    let mut result = String::with_capacity(JINA_READER_BASE.len() + article_url.len());
    result.push_str(JINA_READER_BASE);
    result.push_str(article_url);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    #[test]
    fn test_feed_source_creation() {
        let source = FeedSource {
            name: "Test".to_string(),
            url: "https://example.com/opds".to_string(),
            feed_type: FeedType::Opds,
        };
        assert_eq!(source.name, "Test");
        assert_eq!(source.url, "https://example.com/opds");
        assert_eq!(source.feed_type, FeedType::Opds);
    }

    #[test]
    fn test_feed_type() {
        let opds = FeedType::Opds;
        let rss = FeedType::Rss;
        assert_ne!(opds, rss);
    }

    #[test]
    fn test_opds_catalog_creation() {
        let catalog = OpdsCatalog {
            title: "My Catalog".to_string(),
            subtitle: Some("A test catalog".to_string()),
            entries: Vec::new(),
            links: Vec::new(),
        };
        assert_eq!(catalog.title, "My Catalog");
        assert!(catalog.subtitle.is_some());
    }

    #[test]
    fn test_opds_entry_creation() {
        let entry = OpdsEntry {
            id: "book-123".to_string(),
            title: "Test Book".to_string(),
            author: Some("Author Name".to_string()),
            summary: Some("A test book description.".to_string()),
            cover_url: Some("https://example.com/cover.jpg".to_string()),
            download_url: Some("https://example.com/book.epub".to_string()),
            format: Some("application/epub+zip".to_string()),
            size: Some(1024000),
        };

        assert_eq!(entry.title, "Test Book");
        assert_eq!(entry.author.as_ref().unwrap(), "Author Name");
        assert!(entry.download_url.is_some());
    }

    #[test]
    fn test_opds_link_creation() {
        let link = OpdsLink {
            href: "https://example.com/next".to_string(),
            rel: "next".to_string(),
            title: Some("Next Page".to_string()),
        };
        assert_eq!(link.rel, "next");
    }

    #[test]
    fn test_preloaded_opds_sources_not_empty() {
        assert!(!PRELOADED_OPDS_SOURCES.is_empty());

        let gutenberg = PRELOADED_OPDS_SOURCES
            .iter()
            .find(|(name, _)| *name == "Project Gutenberg");
        assert!(gutenberg.is_some());
    }

    #[test]
    fn test_preloaded_rss_sources_not_empty() {
        assert!(!PRELOADED_RSS_SOURCES.is_empty());

        let hn = PRELOADED_RSS_SOURCES
            .iter()
            .find(|(name, _)| *name == "Hacker News");
        assert!(hn.is_some());
    }

    #[test]
    fn test_all_preloaded_sources() {
        let all = all_preloaded_sources();
        assert!(!all.is_empty());

        let opds_count = all.iter().filter(|(_, _, t)| *t == FeedType::Opds).count();
        let rss_count = all.iter().filter(|(_, _, t)| *t == FeedType::Rss).count();

        assert_eq!(opds_count, PRELOADED_OPDS_SOURCES.len());
        assert_eq!(rss_count, PRELOADED_RSS_SOURCES.len());
    }

    #[test]
    fn test_preloaded_opds_urls_valid() {
        for (name, url) in PRELOADED_OPDS_SOURCES {
            assert!(
                url.starts_with("https://"),
                "Source '{}' should use HTTPS: {}",
                name,
                url
            );
        }
    }

    #[test]
    fn test_preloaded_rss_urls_valid() {
        for (name, url) in PRELOADED_RSS_SOURCES {
            assert!(
                url.starts_with("https://"),
                "Source '{}' should use HTTPS: {}",
                name,
                url
            );
        }
    }

    #[test]
    fn test_jina_reader_url() {
        let article = "https://example.com/article";
        let reader_url = get_reader_url(article);
        assert_eq!(reader_url, "https://r.jina.ai/https://example.com/article");
    }

    #[test]
    fn test_jina_reader_base() {
        assert!(JINA_READER_BASE.starts_with("https://"));
        assert!(JINA_READER_BASE.ends_with("/"));
    }
}
