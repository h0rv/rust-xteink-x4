use alloc::format;
use alloc::string::{String, ToString};
use core::fmt;
use std::fs;
use std::io::Write as IoWrite;
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};

use esp_idf_svc::http::server::{Configuration, EspHttpServer};
use esp_idf_svc::http::{Headers, Method};
use esp_idf_svc::io::{EspIOError, Read, Write};

const SERVER_STACK_SIZE: usize = 10 * 1024;
const MAX_UPLOAD_BYTES: usize = 32 * 1024 * 1024;
const EVENT_QUEUE_DEPTH: usize = 8;
const IO_CHUNK_BYTES: usize = 4096;
const SD_ROOT: &str = "/sd";
const DEFAULT_UPLOAD_DIR: &str = "/books";
const API_VERSION: &str = "v1";

#[derive(Debug, Clone)]
pub struct UploadEvent {
    pub path: String,
    pub received_bytes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PollError {
    QueueDisconnected,
}

impl fmt::Display for PollError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PollError::QueueDisconnected => write!(f, "web upload event queue disconnected"),
        }
    }
}

pub struct WebUploadServer {
    _server: EspHttpServer<'static>,
    event_rx: Receiver<UploadEvent>,
}

impl WebUploadServer {
    pub fn start() -> Result<Self, EspIOError> {
        let mut server = EspHttpServer::new(&Configuration {
            stack_size: SERVER_STACK_SIZE,
            max_uri_handlers: 12,
            ..Default::default()
        })?;
        let (event_tx, event_rx) = mpsc::sync_channel(EVENT_QUEUE_DEPTH);

        server.fn_handler::<(), _>("/", Method::Get, |req| {
            let mut resp = req.into_ok_response().map_err(|_| ())?;
            let _ = resp.write_all(b"xteink web upload server");
            Ok(())
        })?;

        server.fn_handler::<(), _>("/upload/health", Method::Get, |req| {
            let mut resp = req.into_ok_response().map_err(|_| ())?;
            let _ = resp.write_all(b"OK");
            Ok(())
        })?;

        server.fn_handler::<(), _>("/api/status", Method::Get, |req| {
            let mut resp = req.into_ok_response().map_err(|_| ())?;
            let body = format!(
                "{{\"ok\":true,\"api\":\"{}\",\"upload\":{{\"maxBytes\":{},\"defaultDir\":\"{}\"}}}}",
                API_VERSION, MAX_UPLOAD_BYTES, DEFAULT_UPLOAD_DIR
            );
            let _ = resp.write_all(body.as_bytes());
            Ok(())
        })?;

        server.fn_handler::<(), _>("/api/files", Method::Get, |req| {
            let uri = req.uri().to_string();
            let dir = parse_query_param(&uri, "path")
                .and_then(|value| sanitize_virtual_path(&value))
                .unwrap_or_else(|| DEFAULT_UPLOAD_DIR.to_string());
            let host_dir = virtual_to_host_path(&dir);
            let mut out = String::from("[");
            if let Ok(read_dir) = fs::read_dir(&host_dir) {
                let mut first = true;
                for entry in read_dir.flatten() {
                    let Some(name) = entry.file_name().to_str().map(|s| s.to_string()) else {
                        continue;
                    };
                    if name.starts_with('.') {
                        continue;
                    }
                    let Ok(meta) = entry.metadata() else {
                        continue;
                    };
                    if !first {
                        out.push(',');
                    }
                    first = false;
                    let escaped_name = escape_json(&name);
                    let is_dir = meta.is_dir();
                    let size = if is_dir { 0 } else { meta.len() };
                    let is_epub = name.to_ascii_lowercase().ends_with(".epub");
                    out.push_str(&format!(
                        "{{\"name\":\"{}\",\"size\":{},\"isDirectory\":{},\"isEpub\":{}}}",
                        escaped_name,
                        size,
                        if is_dir { "true" } else { "false" },
                        if is_epub { "true" } else { "false" }
                    ));
                }
            }
            out.push(']');
            let mut resp = req.into_ok_response().map_err(|_| ())?;
            let _ = resp.write_all(out.as_bytes());
            Ok(())
        })?;

        let upload_tx = event_tx.clone();
        server.fn_handler::<(), _>("/upload", Method::Post, move |req| {
            handle_upload(req, &upload_tx)
        })?;
        let upload_tx = event_tx.clone();
        server.fn_handler::<(), _>("/upload", Method::Put, move |req| {
            handle_upload(req, &upload_tx)
        })?;

        log::info!("[WEB] upload server started on port 80");
        Ok(Self {
            _server: server,
            event_rx,
        })
    }

    pub fn poll(&mut self) -> Result<Option<UploadEvent>, PollError> {
        match self.event_rx.try_recv() {
            Ok(event) => Ok(Some(event)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(PollError::QueueDisconnected),
        }
    }

    pub fn stop(self) {
        drop(self);
        log::info!("[WEB] upload server stopped");
    }
}

fn handle_upload(
    mut req: esp_idf_svc::http::server::Request<&mut esp_idf_svc::http::server::EspHttpConnection>,
    upload_tx: &SyncSender<UploadEvent>,
) -> Result<(), ()> {
    let Some(content_len) = req.content_len() else {
        if let Ok(mut resp) = req.into_status_response(411) {
            let _ = resp.write_all(b"{\"ok\":false,\"error\":\"Content-Length required\"}");
        }
        return Ok(());
    };
    let content_len = content_len as usize;
    if content_len == 0 {
        if let Ok(mut resp) = req.into_status_response(400) {
            let _ = resp.write_all(b"{\"ok\":false,\"error\":\"Empty body\"}");
        }
        return Ok(());
    }
    if content_len > MAX_UPLOAD_BYTES {
        if let Ok(mut resp) = req.into_status_response(413) {
            let _ = resp.write_all(b"{\"ok\":false,\"error\":\"Payload too large\"}");
        }
        return Ok(());
    }

    let uri = req.uri().to_string();
    let filename = req
        .header("X-Filename")
        .map(ToString::to_string)
        .or_else(|| parse_query_param(&uri, "filename"))
        .and_then(|value| sanitize_filename(&value))
        .unwrap_or_else(|| "upload.epub".to_string());
    let requested_dir = req
        .header("X-Upload-Path")
        .map(ToString::to_string)
        .or_else(|| parse_query_param(&uri, "path"))
        .and_then(|value| sanitize_virtual_path(&value))
        .unwrap_or_else(|| DEFAULT_UPLOAD_DIR.to_string());

    let mut virtual_target = requested_dir;
    if !virtual_target.ends_with('/') {
        virtual_target.push('/');
    }
    virtual_target.push_str(&filename);
    let host_target = virtual_to_host_path(&virtual_target);

    if let Some(parent) = std::path::Path::new(&host_target).parent() {
        if let Err(err) = fs::create_dir_all(parent) {
            log::warn!("[WEB] unable to create upload dir: {}", err);
            if let Ok(mut resp) = req.into_status_response(500) {
                let _ = resp.write_all(b"{\"ok\":false,\"error\":\"Unable to create directory\"}");
            }
            return Ok(());
        }
    }

    let mut out = match fs::File::create(&host_target) {
        Ok(file) => file,
        Err(err) => {
            log::warn!("[WEB] unable to create upload file: {}", err);
            if let Ok(mut resp) = req.into_status_response(500) {
                let _ = resp.write_all(b"{\"ok\":false,\"error\":\"Unable to create file\"}");
            }
            return Ok(());
        }
    };

    let mut remaining = content_len;
    let mut read_buf = [0u8; IO_CHUNK_BYTES];
    while remaining > 0 {
        let want = remaining.min(read_buf.len());
        if let Err(err) = req.read_exact(&mut read_buf[..want]) {
            let _ = fs::remove_file(&host_target);
            log::warn!("[WEB] upload read failed: {:?}", err);
            if let Ok(mut resp) = req.into_status_response(400) {
                let _ = resp.write_all(b"{\"ok\":false,\"error\":\"Bad upload body\"}");
            }
            return Ok(());
        }
        if let Err(err) = out.write_all(&read_buf[..want]) {
            let _ = fs::remove_file(&host_target);
            log::warn!("[WEB] upload write failed: {}", err);
            if let Ok(mut resp) = req.into_status_response(500) {
                let _ = resp.write_all(b"{\"ok\":false,\"error\":\"Write failed\"}");
            }
            return Ok(());
        }
        remaining -= want;
    }

    if let Err(err) = enqueue_event(
        upload_tx,
        UploadEvent {
            path: virtual_target.clone(),
            received_bytes: content_len,
        },
    ) {
        log::warn!("[WEB] upload event queueing failed: {}", err);
    }

    let escaped = escape_json(&virtual_target);
    let mut resp = match req.into_status_response(201) {
        Ok(resp) => resp,
        Err(_) => return Ok(()),
    };
    let body = format!(
        "{{\"ok\":true,\"path\":\"{}\",\"bytes\":{}}}",
        escaped, content_len
    );
    let _ = resp.write_all(body.as_bytes());
    Ok(())
}

fn enqueue_event(tx: &SyncSender<UploadEvent>, event: UploadEvent) -> Result<(), &'static str> {
    match tx.try_send(event) {
        Ok(()) => Ok(()),
        Err(TrySendError::Full(_)) => Err("queue full"),
        Err(TrySendError::Disconnected(_)) => Err("queue disconnected"),
    }
}

fn sanitize_virtual_path(input: &str) -> Option<String> {
    let mut out = String::new();
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Some(DEFAULT_UPLOAD_DIR.to_string());
    }
    for part in trimmed.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            return None;
        }
        if !part.bytes().all(
            |b| matches!(b, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b' '),
        ) {
            return None;
        }
        out.push('/');
        out.push_str(part);
    }
    if out.is_empty() {
        out.push('/');
    }
    Some(out)
}

fn sanitize_filename(input: &str) -> Option<String> {
    let name = input.trim();
    if name.is_empty() || name == "." || name == ".." {
        return None;
    }
    if name.contains('/') || name.contains('\\') {
        return None;
    }
    if !name
        .bytes()
        .all(|b| matches!(b, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b' '))
    {
        return None;
    }
    Some(name.to_string())
}

fn virtual_to_host_path(virtual_path: &str) -> String {
    let mut out = String::from(SD_ROOT);
    if virtual_path.starts_with('/') {
        out.push_str(virtual_path);
    } else {
        out.push('/');
        out.push_str(virtual_path);
    }
    out
}

fn parse_query_param(uri: &str, key: &str) -> Option<String> {
    let (_, query) = uri.split_once('?')?;
    for pair in query.split('&') {
        let (k, v) = pair.split_once('=')?;
        if k == key {
            return Some(percent_decode(v));
        }
    }
    None
}

fn percent_decode(value: &str) -> String {
    let mut out = String::new();
    let bytes = value.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let h1 = (bytes[i + 1] as char).to_digit(16);
            let h2 = (bytes[i + 2] as char).to_digit(16);
            if let (Some(a), Some(b)) = (h1, h2) {
                out.push((a * 16 + b) as u8 as char);
                i += 3;
                continue;
            }
        }
        if bytes[i] == b'+' {
            out.push(' ');
        } else {
            out.push(bytes[i] as char);
        }
        i += 1;
    }
    out
}

fn escape_json(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push('?'),
            c => out.push(c),
        }
    }
    out
}
