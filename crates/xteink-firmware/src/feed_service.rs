//! Feed service for fetching and parsing OPDS catalogs.

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use einked_ereader::{get_reader_url, FeedEntryData, FeedType, OpdsCatalog, OpdsEntry, OpdsLink};
use embedded_svc::http::client::Client as HttpClient;
use embedded_svc::http::Headers;
use esp_idf_svc::http::client::{Configuration as HttpConfiguration, EspHttpConnection};

/// Maximum size for OPDS/RSS feed XML response (256 KB)
const MAX_FEED_BYTES: usize = 256 * 1024;

/// Maximum number of entries to parse from a feed
const MAX_ENTRIES: usize = 200;

#[derive(Debug)]
pub enum FeedError {
    Http(String),
    Parse(String),
    Network(String),
    Io(String),
    ResponseTooLarge(usize),
}

pub struct FeedService {
    client: HttpClient<EspHttpConnection>,
}

impl FeedService {
    pub fn new() -> Result<Self, FeedError> {
        let config = HttpConfiguration {
            use_global_ca_store: true,
            crt_bundle_attach: Some(esp_idf_svc::sys::esp_crt_bundle_attach),
            ..Default::default()
        };
        let conn =
            EspHttpConnection::new(&config).map_err(|e| FeedError::Http(format!("{:?}", e)))?;
        let client = HttpClient::wrap(conn);
        Ok(Self { client })
    }

    pub fn fetch_catalog(&mut self, url: &str) -> Result<OpdsCatalog, FeedError> {
        let bytes = self.http_get_feed(url)?;
        self.parse_opds(&bytes)
    }

    pub fn fetch_entries(
        &mut self,
        url: &str,
        feed_type: FeedType,
    ) -> Result<Vec<FeedEntryData>, FeedError> {
        match feed_type {
            FeedType::Opds => {
                let catalog = self.fetch_catalog(url)?;
                let mut entries = Vec::new();
                for entry in catalog.entries.iter().take(64) {
                    entries.push(FeedEntryData {
                        title: entry.title.clone(),
                        url: entry
                            .download_url
                            .clone()
                            .or_else(|| entry.cover_url.clone()),
                        summary: entry.summary.clone(),
                    });
                }
                Ok(entries)
            }
            FeedType::Rss => {
                let bytes = self.http_get_feed(url)?;
                let feed = feed_rs::parser::parse(&bytes[..])
                    .map_err(|e| FeedError::Parse(format!("{:?}", e)))?;
                let mut entries = Vec::new();
                for entry in feed.entries.iter().take(64) {
                    entries.push(FeedEntryData {
                        title: entry
                            .title
                            .as_ref()
                            .map(|t| t.content.clone())
                            .unwrap_or_default(),
                        url: entry.links.first().map(|link| link.href.clone()),
                        summary: entry.summary.as_ref().map(|s| s.content.clone()),
                    });
                }
                Ok(entries)
            }
        }
    }

    pub fn fetch_article_text(&mut self, url: &str) -> Result<String, FeedError> {
        let reader_url = get_reader_url(url);
        let bytes = self.http_get_feed(&reader_url)?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    pub fn download_book<F: FnMut(u64, u64)>(
        &mut self,
        url: &str,
        dest_path: &str,
        mut progress: F,
    ) -> Result<(), FeedError> {
        if let Some(parent) = std::path::Path::new(dest_path).parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| FeedError::Io(format!("Create dir failed: {:?}", e)))?;
        }

        let request = self
            .client
            .get(url)
            .map_err(|e| FeedError::Http(format!("{:?}", e)))?;

        let mut response = request
            .submit()
            .map_err(|e| FeedError::Network(format!("{:?}", e)))?;

        let status = response.status();
        if status != 200 {
            return Err(FeedError::Http(format!("HTTP {}", status)));
        }

        let total_size = response.content_len().unwrap_or(0);
        let mut file = std::fs::File::create(dest_path)
            .map_err(|e| FeedError::Io(format!("Create file failed: {:?}", e)))?;
        let mut downloaded: u64 = 0;
        let mut buf = [0u8; 4096];

        loop {
            let read = response
                .read(&mut buf)
                .map_err(|e| FeedError::Io(format!("{:?}", e)))?;
            if read == 0 {
                break;
            }
            std::io::Write::write_all(&mut file, &buf[..read])
                .map_err(|e| FeedError::Io(format!("Write failed: {:?}", e)))?;
            downloaded += read as u64;
            progress(downloaded, total_size.max(downloaded));
        }

        Ok(())
    }

    fn http_get_feed(&mut self, url: &str) -> Result<Vec<u8>, FeedError> {
        let request = self
            .client
            .get(url)
            .map_err(|e| FeedError::Http(format!("{:?}", e)))?;

        let mut response = request
            .submit()
            .map_err(|e| FeedError::Network(format!("{:?}", e)))?;

        let status = response.status();
        if status != 200 {
            return Err(FeedError::Http(format!("HTTP {}", status)));
        }

        let content_length = response.content_len().unwrap_or(0) as usize;
        if content_length > MAX_FEED_BYTES {
            return Err(FeedError::ResponseTooLarge(content_length));
        }

        let estimated_size = content_length.min(MAX_FEED_BYTES).max(1024);
        let mut body = Vec::with_capacity(estimated_size);
        let mut buf = [0u8; 4096];

        loop {
            let read = response
                .read(&mut buf)
                .map_err(|e| FeedError::Io(format!("{:?}", e)))?;
            if read == 0 {
                break;
            }
            if body.len() + read > MAX_FEED_BYTES {
                return Err(FeedError::ResponseTooLarge(body.len() + read));
            }
            body.extend_from_slice(&buf[..read]);
        }

        Ok(body)
    }

    fn parse_opds(&self, bytes: &[u8]) -> Result<OpdsCatalog, FeedError> {
        let feed =
            feed_rs::parser::parse(bytes).map_err(|e| FeedError::Parse(format!("{:?}", e)))?;

        let title = feed
            .title
            .as_ref()
            .map(|t| t.content.clone())
            .unwrap_or_default();

        let entries: Vec<OpdsEntry> = feed
            .entries
            .iter()
            .take(MAX_ENTRIES)
            .map(|entry| {
                let (download_url, format, size) = entry
                    .links
                    .iter()
                    .filter_map(|link| {
                        let rel = link.rel.as_deref().unwrap_or("");
                        if rel.contains("acquisition") {
                            Some((
                                Some(link.href.clone()),
                                link.media_type.clone(),
                                link.length,
                            ))
                        } else {
                            None
                        }
                    })
                    .next()
                    .unwrap_or((None, None, None));

                let cover_url = entry
                    .links
                    .iter()
                    .filter_map(|link| {
                        let rel = link.rel.as_deref().unwrap_or("");
                        if rel.contains("cover") || rel.contains("thumbnail") {
                            Some(link.href.clone())
                        } else {
                            None
                        }
                    })
                    .next();

                OpdsEntry {
                    id: entry.id.clone(),
                    title: entry
                        .title
                        .as_ref()
                        .map(|t| t.content.clone())
                        .unwrap_or_default(),
                    author: entry.authors.first().map(|a| a.name.clone()),
                    summary: entry.summary.as_ref().map(|s| s.content.clone()),
                    cover_url,
                    download_url,
                    format,
                    size,
                }
            })
            .collect();

        let links: Vec<OpdsLink> = feed
            .links
            .iter()
            .map(|link| OpdsLink {
                href: link.href.clone(),
                rel: link.rel.clone().unwrap_or_default(),
                title: link.title.clone(),
            })
            .collect();

        Ok(OpdsCatalog {
            title,
            subtitle: feed.description.as_ref().map(|d| d.content.clone()),
            entries,
            links,
        })
    }
}
