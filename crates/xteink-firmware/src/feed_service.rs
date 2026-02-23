//! Feed service for fetching and parsing OPDS catalogs.

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use embedded_svc::http::client::Client as HttpClient;
use embedded_svc::io::Read;
use esp_idf_svc::http::client::{Configuration as HttpConfiguration, EspHttpConnection};

use xteink_ui::{OpdsCatalog, OpdsEntry, OpdsLink};

#[derive(Debug)]
pub enum FeedError {
    Http(String),
    Parse(String),
    Network(String),
    Io(String),
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
        let bytes = self.http_get(url)?;
        self.parse_opds(&bytes)
    }

    pub fn download_book<F: FnMut(u64, u64)>(
        &mut self,
        url: &str,
        dest_path: &str,
        mut progress: F,
    ) -> Result<(), FeedError> {
        let bytes = self.http_get(url)?;
        progress(bytes.len() as u64, bytes.len() as u64);

        if let Some(parent) = std::path::Path::new(dest_path).parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| FeedError::Io(format!("Create dir failed: {:?}", e)))?;
        }

        std::fs::write(dest_path, &bytes)
            .map_err(|e| FeedError::Io(format!("Write failed: {:?}", e)))?;

        Ok(())
    }

    fn http_get(&mut self, url: &str) -> Result<Vec<u8>, FeedError> {
        let request = self
            .client
            .get(url, &[])
            .map_err(|e| FeedError::Http(format!("{:?}", e)))?;

        let mut response = request
            .submit()
            .map_err(|e| FeedError::Network(format!("{:?}", e)))?;

        let status = response.status();
        if status != 200 {
            return Err(FeedError::Http(format!("HTTP {}", status)));
        }

        let mut body = Vec::new();
        let mut buf = [0u8; 4096];
        loop {
            let read = response
                .read(&mut buf)
                .map_err(|e| FeedError::Io(format!("{:?}", e)))?;
            if read == 0 {
                break;
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
