use alloc::format;
use alloc::string::{String, ToString};
use core::fmt;
use std::fs;
use std::io::Write as IoWrite;
use std::io::{Read as StdRead, Seek as StdSeek, SeekFrom};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};

use esp_idf_svc::http::server::{Configuration, EspHttpConnection, EspHttpServer, Request};
use esp_idf_svc::http::Method;
use esp_idf_svc::io::{EspIOError, Read, Write};
#[cfg(any(esp_idf_comp_mdns_enabled, esp_idf_comp_espressif__mdns_enabled))]
use esp_idf_svc::mdns::EspMdns;
use esp_idf_svc::sys::{self, EspError};

const SERVER_STACK_SIZE: usize = 10 * 1024;
const MAX_UPLOAD_BYTES: usize = 32 * 1024 * 1024;
const EVENT_QUEUE_DEPTH: usize = 8;
const IO_CHUNK_BYTES_MIN: usize = 1024;
const IO_CHUNK_BYTES_MAX: usize = 8192;
const SD_ROOT: &str = "/sd";
const DEFAULT_UPLOAD_DIR: &str = "/books";
const API_VERSION: &str = "v1";
const TRANSFER_MDNS_HOSTNAME: &str = "xteink-x4";
const TRANSFER_MDNS_INSTANCE: &str = "Xteink X4 Transfer";
const TRANSFER_MDNS_HTTP_SERVICE: &str = "_http";
const TRANSFER_MDNS_XTEINK_SERVICE: &str = "_xteink";
const TRANSFER_MDNS_PROTO: &str = "_tcp";
const TRANSFER_MDNS_PORT: u16 = 80;
const TRANSFER_MDNS_HOST_LABEL: &str = "xteink-x4.local";
const MULTIPART_TEMP_PATH: &str = "/sd/.tmp/upload.multipart";
const MULTIPART_HEADER_SCAN_MAX_BYTES: usize = 8 * 1024;
const MULTIPART_HEADER_SCAN_CHUNK_BYTES_MAX: usize = 1024;
const UPLOAD_CORS_RESPONSE_HEADERS: &[(&str, &str)] = &[
    ("Access-Control-Allow-Origin", "*"),
    ("Access-Control-Allow-Methods", "POST, PUT, OPTIONS"),
    (
        "Access-Control-Allow-Headers",
        "Content-Type, Content-Length, X-Filename, X-Upload-Path",
    ),
];
const UPLOAD_CORS_PREFLIGHT_HEADERS: &[(&str, &str)] = &[
    ("Access-Control-Allow-Origin", "*"),
    ("Access-Control-Allow-Methods", "POST, PUT, OPTIONS"),
    (
        "Access-Control-Allow-Headers",
        "Content-Type, Content-Length, X-Filename, X-Upload-Path",
    ),
    ("Access-Control-Max-Age", "600"),
];

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
    _mdns: Option<TransferMdns>,
    event_rx: Receiver<UploadEvent>,
}

impl WebUploadServer {
    pub fn start() -> Result<Self, EspIOError> {
        ensure_tcpip_ready()?;
        let mut server = EspHttpServer::new(&Configuration {
            stack_size: SERVER_STACK_SIZE,
            max_uri_handlers: 16,
            ..Default::default()
        })?;
        let (event_tx, event_rx) = mpsc::sync_channel(EVENT_QUEUE_DEPTH);

        server.fn_handler::<(), _>("/", Method::Get, |req| {
            let mut resp = req.into_ok_response().map_err(|_| ())?;
            let _ = resp.write_all(
                br#"<!doctype html>
<html lang="en">
<head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Xteink Transfer</title>
<style>
body{font-family:system-ui,-apple-system,sans-serif;background:#f4f4f4;color:#111;margin:0;padding:24px}
.card{max-width:920px;margin:0 auto;background:#fff;border:1px solid #ddd;border-radius:10px;padding:16px}
h1{margin:0 0 8px 0}.muted{color:#555;font-size:14px}
input,button{font-size:16px;padding:10px}
button{cursor:pointer}
pre{background:#f7f7f7;padding:10px;border:1px solid #eee;overflow:auto}
table{width:100%;border-collapse:collapse;margin-top:12px}
th,td{border-bottom:1px solid #eee;padding:8px;text-align:left;font-size:14px}
.row{display:flex;gap:8px;align-items:center;flex-wrap:wrap}
.path{font-family:ui-monospace,Menlo,Consolas,monospace;background:#f7f7f7;border:1px solid #eee;padding:6px 8px;border-radius:6px}
</style></head>
<body><div class="card">
<h1>Xteink File Transfer</h1>
<p class="muted">Upload, list, download, and delete files on device storage.</p>
<div class="row">
  <label>Path</label>
  <input id="path" value="/books" />
  <button id="refresh" type="button">Refresh</button>
</div>
<form id="upf">
<input id="file" type="file" required>
<button type="submit">Upload</button>
</form>
<table>
<thead><tr><th>Name</th><th>Size</th><th>Type</th><th>Actions</th></tr></thead>
<tbody id="files"></tbody>
</table>
<pre id="out">Ready.</pre>
</div>
<script>
const form=document.getElementById('upf');const out=document.getElementById('out');
const filesBody=document.getElementById('files');
const pathEl=document.getElementById('path');
const fmt=(n)=>{if(!n)return '0 B';const u=['B','KB','MB','GB'];let i=0;let v=n;while(v>=1024&&i<u.length-1){v/=1024;i++;}return v.toFixed(i?1:0)+' '+u[i];}
async function loadFiles(){
  const path=pathEl.value||'/books';
  try{
    const r=await fetch('/api/files?path='+encodeURIComponent(path));
    const items=await r.json();
    filesBody.innerHTML='';
    for(const it of items){
      const tr=document.createElement('tr');
      const name=it.name||'';
      const t=it.isDirectory?'dir':(it.isEpub?'epub':'file');
      const downloadPath=path.replace(/\/$/,'')+'/'+name;
      tr.innerHTML='<td>'+name+'</td><td>'+fmt(it.size||0)+'</td><td>'+t+'</td><td></td>';
      const td=tr.children[3];
      if(it.isDirectory){
        const b=document.createElement('button');
        b.textContent='Open';
        b.onclick=()=>{pathEl.value=downloadPath;loadFiles();};
        td.appendChild(b);
      }else{
        const a=document.createElement('a');
        a.textContent='Download';
        a.href='/api/download?path='+encodeURIComponent(downloadPath);
        a.style.marginRight='8px';
        td.appendChild(a);
        const del=document.createElement('button');
        del.textContent='Delete';
        del.onclick=async()=>{if(!confirm('Delete '+name+'?'))return;
          const rr=await fetch('/api/delete?path='+encodeURIComponent(downloadPath),{method:'POST'});
          out.textContent='Delete '+name+': HTTP '+rr.status+' '+await rr.text();
          await loadFiles();
        };
        td.appendChild(del);
      }
      filesBody.appendChild(tr);
    }
    out.textContent='Loaded '+items.length+' item(s) from '+path;
  }catch(err){
    out.textContent='List failed: '+err;
  }
}
document.getElementById('refresh').addEventListener('click',loadFiles);
form.addEventListener('submit', async(e)=>{e.preventDefault();const f=document.getElementById('file').files[0];if(!f){return;}
const path=pathEl.value||'/books';
out.textContent='Uploading '+f.name+' ...';
try{const r=await fetch('/upload?filename='+encodeURIComponent(f.name)+'&path='+encodeURIComponent(path),{method:'POST',headers:{'Content-Length':String(f.size)},body:f});
const t=await r.text();out.textContent='HTTP '+r.status+'\\n'+t;await loadFiles();}catch(err){out.textContent='Upload failed: '+err;}});
loadFiles();
</script>
</body></html>"#,
            );
            Ok(())
        })?;
        server.fn_handler::<(), _>("/files", Method::Get, |req| {
            let mut resp = req.into_ok_response().map_err(|_| ())?;
            let _ = resp.write_all(
                br#"<!doctype html><html><head><meta http-equiv="refresh" content="0; url=/" /></head><body>Redirecting...</body></html>"#,
            );
            Ok(())
        })?;

        server.fn_handler::<(), _>("/upload/health", Method::Get, |req| {
            let mut resp = req.into_ok_response().map_err(|_| ())?;
            let _ = resp.write_all(b"OK");
            Ok(())
        })?;

        server.fn_handler::<(), _>("/api/status", Method::Get, |req| {
            let host = req
                .header("Host")
                .map(ToString::to_string)
                .unwrap_or_else(|| "192.168.71.1".to_string());
            let ip = host
                .split(':')
                .next()
                .filter(|value| !value.is_empty())
                .unwrap_or("192.168.71.1");
            let free_heap = unsafe { sys::esp_get_free_heap_size() };
            let largest_8bit = largest_8bit_block_bytes();
            let chunk_bytes = runtime_io_chunk_bytes();
            let uptime = (unsafe { sys::esp_timer_get_time() } / 1_000_000) as u64;
            let body = format!(
                "{{\"version\":\"{}\",\"ip\":\"{}\",\"mode\":\"AP\",\"rssi\":0,\"freeHeap\":{},\"largest8bitBlock\":{},\"uptime\":{},\"ok\":true,\"api\":\"{}\",\"upload\":{{\"maxBytes\":{},\"defaultDir\":\"{}\",\"chunkBytes\":{}}}}}",
                env!("CARGO_PKG_VERSION"),
                escape_json(ip),
                free_heap,
                largest_8bit,
                uptime,
                API_VERSION,
                MAX_UPLOAD_BYTES,
                DEFAULT_UPLOAD_DIR,
                chunk_bytes
            );
            let mut resp = req.into_ok_response().map_err(|_| ())?;
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
        server.fn_handler::<(), _>("/api/download", Method::Get, |req| {
            let uri = req.uri().to_string();
            let Some(path) =
                parse_query_param(&uri, "path").and_then(|v| sanitize_virtual_path(&v))
            else {
                if let Ok(mut resp) = req.into_status_response(400) {
                    let _ = resp.write_all(b"{\"ok\":false,\"error\":\"Invalid path\"}");
                }
                return Ok(());
            };
            let host_path = virtual_to_host_path(&path);
            let mut file = match fs::File::open(&host_path) {
                Ok(file) => file,
                Err(_) => {
                    if let Ok(mut resp) = req.into_status_response(404) {
                        let _ = resp.write_all(b"{\"ok\":false,\"error\":\"Not found\"}");
                    }
                    return Ok(());
                }
            };
            let mut resp = match req.into_ok_response() {
                Ok(resp) => resp,
                Err(_) => return Ok(()),
            };
            let mut buf = vec![0u8; runtime_io_chunk_bytes()];
            loop {
                let read = match std::io::Read::read(&mut file, &mut buf) {
                    Ok(read) => read,
                    Err(_) => {
                        let _ = resp.write_all(b"");
                        break;
                    }
                };
                if read == 0 {
                    break;
                }
                if resp.write_all(&buf[..read]).is_err() {
                    break;
                }
            }
            Ok(())
        })?;
        server.fn_handler::<(), _>("/download", Method::Get, |req| {
            let uri = req.uri().to_string();
            let Some(path) =
                parse_query_param(&uri, "path").and_then(|v| sanitize_virtual_path(&v))
            else {
                if let Ok(mut resp) = req.into_status_response(400) {
                    let _ = resp.write_all(b"{\"ok\":false,\"error\":\"Invalid path\"}");
                }
                return Ok(());
            };
            let host_path = virtual_to_host_path(&path);
            let mut file = match fs::File::open(&host_path) {
                Ok(file) => file,
                Err(_) => {
                    if let Ok(mut resp) = req.into_status_response(404) {
                        let _ = resp.write_all(b"{\"ok\":false,\"error\":\"Not found\"}");
                    }
                    return Ok(());
                }
            };
            let mut resp = match req.into_ok_response() {
                Ok(resp) => resp,
                Err(_) => return Ok(()),
            };
            let mut buf = vec![0u8; runtime_io_chunk_bytes()];
            loop {
                let read = match StdRead::read(&mut file, &mut buf) {
                    Ok(read) => read,
                    Err(_) => break,
                };
                if read == 0 {
                    break;
                }
                if resp.write_all(&buf[..read]).is_err() {
                    break;
                }
            }
            Ok(())
        })?;
        server.fn_handler::<(), _>("/api/delete", Method::Post, |req| {
            let uri = req.uri().to_string();
            let Some(path) =
                parse_query_param(&uri, "path").and_then(|v| sanitize_virtual_path(&v))
            else {
                if let Ok(mut resp) = req.into_status_response(400) {
                    let _ = resp.write_all(b"{\"ok\":false,\"error\":\"Invalid path\"}");
                }
                return Ok(());
            };
            let host_path = virtual_to_host_path(&path);
            let result = match fs::metadata(&host_path) {
                Ok(meta) if meta.is_dir() => fs::remove_dir_all(&host_path),
                Ok(_) => fs::remove_file(&host_path),
                Err(err) => Err(err),
            };
            match result {
                Ok(()) => {
                    let mut resp = req.into_ok_response().map_err(|_| ())?;
                    let escaped = escape_json(&path);
                    let body = format!("{{\"ok\":true,\"deleted\":\"{}\"}}", escaped);
                    let _ = resp.write_all(body.as_bytes());
                }
                Err(_) => {
                    if let Ok(mut resp) = req.into_status_response(404) {
                        let _ = resp.write_all(b"{\"ok\":false,\"error\":\"Delete failed\"}");
                    }
                }
            }
            Ok(())
        })?;
        server.fn_handler::<(), _>("/delete", Method::Post, |req| {
            let uri = req.uri().to_string();
            let mut path = parse_query_param(&uri, "path");
            let mut item_type = parse_query_param(&uri, "type");
            let maybe_content_len = req
                .header("Content-Length")
                .and_then(|value| value.trim().parse::<usize>().ok());
            if path.is_none() && maybe_content_len.is_some() {
                let content_len = maybe_content_len.unwrap_or(0);
                if content_len == 0 || content_len > 4096 {
                    // fall through to query-based deletion
                } else {
                    let mut body = vec![0u8; content_len];
                    let mut req = req;
                    if req.read_exact(&mut body).is_ok() {
                        if let Ok(body_str) = String::from_utf8(body) {
                            if path.is_none() {
                                path = parse_form_param(&body_str, "path");
                            }
                            if item_type.is_none() {
                                item_type = parse_form_param(&body_str, "type");
                            }
                        }
                    }
                    let Some(path) = path.and_then(|value| sanitize_virtual_path(&value)) else {
                        if let Ok(mut resp) = req.into_status_response(400) {
                            let _ = resp.write_all(b"{\"ok\":false,\"error\":\"Invalid path\"}");
                        }
                        return Ok(());
                    };
                    let host_path = virtual_to_host_path(&path);
                    let result = match fs::metadata(&host_path) {
                        Ok(meta) if meta.is_dir() || item_type.as_deref() == Some("folder") => {
                            fs::remove_dir_all(&host_path)
                        }
                        Ok(_) => fs::remove_file(&host_path),
                        Err(err) => Err(err),
                    };
                    match result {
                        Ok(()) => {
                            let mut resp = req.into_ok_response().map_err(|_| ())?;
                            let _ = resp.write_all(b"Deleted successfully");
                        }
                        Err(_) => {
                            if let Ok(mut resp) = req.into_status_response(404) {
                                let _ = resp.write_all(b"Item not found");
                            }
                        }
                    }
                    return Ok(());
                }
            }
            let Some(path) = path.and_then(|value| sanitize_virtual_path(&value)) else {
                if let Ok(mut resp) = req.into_status_response(400) {
                    let _ = resp.write_all(b"{\"ok\":false,\"error\":\"Invalid path\"}");
                }
                return Ok(());
            };
            let host_path = virtual_to_host_path(&path);
            let result = match fs::metadata(&host_path) {
                Ok(meta) if meta.is_dir() || item_type.as_deref() == Some("folder") => {
                    fs::remove_dir_all(&host_path)
                }
                Ok(_) => fs::remove_file(&host_path),
                Err(err) => Err(err),
            };
            match result {
                Ok(()) => {
                    let mut resp = req.into_ok_response().map_err(|_| ())?;
                    let _ = resp.write_all(b"Deleted successfully");
                }
                Err(_) => {
                    if let Ok(mut resp) = req.into_status_response(404) {
                        let _ = resp.write_all(b"Item not found");
                    }
                }
            }
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
        server.fn_handler::<(), _>("/upload", Method::Options, |req| handle_upload_options(req))?;

        log::info!("[WEB] upload server started on port 80");
        let mdns = start_transfer_mdns();
        Ok(Self {
            _server: server,
            _mdns: mdns,
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

#[cfg(any(esp_idf_comp_mdns_enabled, esp_idf_comp_espressif__mdns_enabled))]
struct TransferMdns {
    inner: EspMdns,
}

#[cfg(any(esp_idf_comp_mdns_enabled, esp_idf_comp_espressif__mdns_enabled))]
impl TransferMdns {
    fn start() -> Result<Self, EspError> {
        let mut mdns = EspMdns::take()?;
        mdns.set_hostname(TRANSFER_MDNS_HOSTNAME)?;
        mdns.set_instance_name(TRANSFER_MDNS_INSTANCE)?;

        let txt = [
            ("path", "/"),
            ("api", API_VERSION),
            ("upload", "/upload"),
            ("books", DEFAULT_UPLOAD_DIR),
        ];
        mdns.add_service(
            Some(TRANSFER_MDNS_INSTANCE),
            TRANSFER_MDNS_HTTP_SERVICE,
            TRANSFER_MDNS_PROTO,
            TRANSFER_MDNS_PORT,
            &txt,
        )?;
        if let Err(err) = mdns.add_service(
            Some(TRANSFER_MDNS_INSTANCE),
            TRANSFER_MDNS_XTEINK_SERVICE,
            TRANSFER_MDNS_PROTO,
            TRANSFER_MDNS_PORT,
            &txt,
        ) {
            log::warn!("[WEB] mDNS custom service registration failed: {}", err);
        }

        Ok(Self { inner: mdns })
    }
}

#[cfg(any(esp_idf_comp_mdns_enabled, esp_idf_comp_espressif__mdns_enabled))]
impl Drop for TransferMdns {
    fn drop(&mut self) {
        let _ = self
            .inner
            .remove_service(TRANSFER_MDNS_HTTP_SERVICE, TRANSFER_MDNS_PROTO);
        let _ = self
            .inner
            .remove_service(TRANSFER_MDNS_XTEINK_SERVICE, TRANSFER_MDNS_PROTO);
        log::info!(
            "[WEB] mDNS advertising stopped ({})",
            TRANSFER_MDNS_HOST_LABEL
        );
    }
}

#[cfg(not(any(esp_idf_comp_mdns_enabled, esp_idf_comp_espressif__mdns_enabled)))]
struct TransferMdns;

fn start_transfer_mdns() -> Option<TransferMdns> {
    #[cfg(any(esp_idf_comp_mdns_enabled, esp_idf_comp_espressif__mdns_enabled))]
    {
        match TransferMdns::start() {
            Ok(mdns) => {
                log::info!(
                    "[WEB] mDNS advertising active at http://{}/",
                    TRANSFER_MDNS_HOST_LABEL
                );
                Some(mdns)
            }
            Err(err) => {
                log::warn!("[WEB] mDNS start failed: {}", err);
                None
            }
        }
    }
    #[cfg(not(any(esp_idf_comp_mdns_enabled, esp_idf_comp_espressif__mdns_enabled)))]
    {
        log::info!("[WEB] mDNS component disabled in ESP-IDF config");
        None
    }
}

fn ensure_tcpip_ready() -> Result<(), EspIOError> {
    unsafe {
        let netif_err = sys::esp_netif_init();
        if netif_err != sys::ESP_OK && netif_err != sys::ESP_ERR_INVALID_STATE {
            return Err(EspError::from(netif_err)
                .unwrap_or(EspError::from_infallible::<{ sys::ESP_FAIL }>())
                .into());
        }
        let event_err = sys::esp_event_loop_create_default();
        if event_err != sys::ESP_OK && event_err != sys::ESP_ERR_INVALID_STATE {
            return Err(EspError::from(event_err)
                .unwrap_or(EspError::from_infallible::<{ sys::ESP_FAIL }>())
                .into());
        }
    }
    Ok(())
}

fn handle_upload(
    req: Request<&mut EspHttpConnection>,
    upload_tx: &SyncSender<UploadEvent>,
) -> Result<(), ()> {
    let Some(content_len) = req
        .header("Content-Length")
        .and_then(|value| value.trim().parse::<usize>().ok())
    else {
        write_upload_response(
            req,
            411,
            b"{\"ok\":false,\"error\":\"Content-Length required\"}",
        );
        return Ok(());
    };
    if content_len == 0 {
        write_upload_response(req, 400, b"{\"ok\":false,\"error\":\"Empty body\"}");
        return Ok(());
    }
    if content_len > MAX_UPLOAD_BYTES {
        write_upload_response(req, 413, b"{\"ok\":false,\"error\":\"Payload too large\"}");
        return Ok(());
    }

    let uri = req.uri().to_string();
    let filename = req
        .header("X-Filename")
        .map(ToString::to_string)
        .or_else(|| parse_query_param(&uri, "filename"))
        .and_then(|value| sanitize_relative_upload_path(&value))
        .unwrap_or_else(|| "upload.epub".to_string());
    let requested_dir = req
        .header("X-Upload-Path")
        .map(ToString::to_string)
        .or_else(|| parse_query_param(&uri, "path"))
        .and_then(|value| sanitize_virtual_path(&value))
        .unwrap_or_else(|| DEFAULT_UPLOAD_DIR.to_string());

    let content_type = req
        .header("Content-Type")
        .map(ToString::to_string)
        .unwrap_or_default();
    if let Some(boundary) = parse_multipart_boundary(&content_type) {
        return handle_multipart_upload(
            req,
            upload_tx,
            content_len,
            &requested_dir,
            &filename,
            &boundary,
        );
    }
    handle_raw_upload(req, upload_tx, content_len, &requested_dir, &filename)
}

fn handle_raw_upload(
    mut req: Request<&mut EspHttpConnection>,
    upload_tx: &SyncSender<UploadEvent>,
    content_len: usize,
    requested_dir: &str,
    filename: &str,
) -> Result<(), ()> {
    let virtual_target = join_virtual_path(requested_dir, filename);
    let host_target = virtual_to_host_path(&virtual_target);

    if let Some(parent) = std::path::Path::new(&host_target).parent() {
        if let Err(err) = fs::create_dir_all(parent) {
            log::warn!("[WEB] unable to create upload dir: {}", err);
            write_upload_response(
                req,
                500,
                b"{\"ok\":false,\"error\":\"Unable to create directory\"}",
            );
            return Ok(());
        }
    }

    let mut out = match fs::File::create(&host_target) {
        Ok(file) => file,
        Err(err) => {
            log::warn!("[WEB] unable to create upload file: {}", err);
            write_upload_response(
                req,
                500,
                b"{\"ok\":false,\"error\":\"Unable to create file\"}",
            );
            return Ok(());
        }
    };

    let mut remaining = content_len;
    let mut read_buf = vec![0u8; runtime_io_chunk_bytes()];
    while remaining > 0 {
        let want = remaining.min(read_buf.len());
        if let Err(err) = req.read_exact(&mut read_buf[..want]) {
            let _ = fs::remove_file(&host_target);
            log::warn!("[WEB] upload read failed: {:?}", err);
            write_upload_response(req, 400, b"{\"ok\":false,\"error\":\"Bad upload body\"}");
            return Ok(());
        }
        if let Err(err) = out.write_all(&read_buf[..want]) {
            let _ = fs::remove_file(&host_target);
            log::warn!("[WEB] upload write failed: {}", err);
            write_upload_response(req, 500, b"{\"ok\":false,\"error\":\"Write failed\"}");
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
    let body = format!(
        "{{\"ok\":true,\"path\":\"{}\",\"bytes\":{}}}",
        escaped, content_len
    );
    write_upload_response(req, 201, body.as_bytes());
    Ok(())
}

fn handle_multipart_upload(
    mut req: Request<&mut EspHttpConnection>,
    upload_tx: &SyncSender<UploadEvent>,
    content_len: usize,
    requested_dir: &str,
    fallback_filename: &str,
    boundary: &str,
) -> Result<(), ()> {
    if let Some(parent) = std::path::Path::new(MULTIPART_TEMP_PATH).parent() {
        if let Err(err) = fs::create_dir_all(parent) {
            log::warn!("[WEB] unable to create multipart temp dir: {}", err);
            write_upload_response(
                req,
                500,
                b"{\"ok\":false,\"error\":\"Unable to create temp dir\"}",
            );
            return Ok(());
        }
    }
    let mut temp = match fs::File::create(MULTIPART_TEMP_PATH) {
        Ok(file) => file,
        Err(err) => {
            log::warn!("[WEB] unable to create multipart temp file: {}", err);
            write_upload_response(
                req,
                500,
                b"{\"ok\":false,\"error\":\"Unable to create temp file\"}",
            );
            return Ok(());
        }
    };
    let mut remaining = content_len;
    let mut read_buf = vec![0u8; runtime_io_chunk_bytes()];
    while remaining > 0 {
        let want = remaining.min(read_buf.len());
        if req.read_exact(&mut read_buf[..want]).is_err() {
            let _ = fs::remove_file(MULTIPART_TEMP_PATH);
            write_upload_response(req, 400, b"{\"ok\":false,\"error\":\"Bad upload body\"}");
            return Ok(());
        }
        if temp.write_all(&read_buf[..want]).is_err() {
            let _ = fs::remove_file(MULTIPART_TEMP_PATH);
            write_upload_response(req, 500, b"{\"ok\":false,\"error\":\"Write failed\"}");
            return Ok(());
        }
        remaining -= want;
    }
    drop(temp);

    match extract_multipart_file(
        MULTIPART_TEMP_PATH,
        boundary,
        requested_dir,
        fallback_filename,
    ) {
        Ok((virtual_target, bytes)) => {
            let _ = fs::remove_file(MULTIPART_TEMP_PATH);
            if let Err(err) = enqueue_event(
                upload_tx,
                UploadEvent {
                    path: virtual_target.clone(),
                    received_bytes: bytes,
                },
            ) {
                log::warn!("[WEB] upload event queueing failed: {}", err);
            }
            let escaped = escape_json(&virtual_target);
            let body = format!(
                "{{\"ok\":true,\"path\":\"{}\",\"bytes\":{}}}",
                escaped, bytes
            );
            write_upload_response(req, 201, body.as_bytes());
            Ok(())
        }
        Err(err) => {
            let _ = fs::remove_file(MULTIPART_TEMP_PATH);
            log::warn!("[WEB] multipart extract failed: {}", err);
            write_upload_response(
                req,
                400,
                b"{\"ok\":false,\"error\":\"Invalid multipart payload\"}",
            );
            Ok(())
        }
    }
}

fn handle_upload_options(req: Request<&mut EspHttpConnection>) -> Result<(), ()> {
    let mut resp = req
        .into_response(204, Some("No Content"), UPLOAD_CORS_PREFLIGHT_HEADERS)
        .map_err(|_| ())?;
    let _ = resp.write_all(b"");
    Ok(())
}

fn write_upload_response(req: Request<&mut EspHttpConnection>, status: u16, body: &[u8]) {
    if let Ok(mut resp) = req.into_response(status, None, UPLOAD_CORS_RESPONSE_HEADERS) {
        let _ = resp.write_all(body);
    }
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

fn sanitize_relative_upload_path(input: &str) -> Option<String> {
    let mut out = String::new();
    let normalized = input.trim().trim_matches('"').replace('\\', "/");
    if normalized.is_empty() {
        return None;
    }
    for part in normalized.split('/') {
        let segment = part.trim();
        if segment.is_empty() || segment == "." {
            continue;
        }
        if segment == ".." {
            return None;
        }
        if !segment.bytes().all(
            |b| matches!(b, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b' '),
        ) {
            return None;
        }
        if !out.is_empty() {
            out.push('/');
        }
        out.push_str(segment);
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn join_virtual_path(base_dir: &str, relative_path: &str) -> String {
    let mut out = base_dir.to_string();
    if !out.ends_with('/') {
        out.push('/');
    }
    out.push_str(relative_path.trim_start_matches('/'));
    out
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

fn parse_form_param(body: &str, key: &str) -> Option<String> {
    for pair in body.split('&') {
        let (k, v) = pair.split_once('=')?;
        if k == key {
            return Some(percent_decode(v));
        }
    }
    None
}

fn parse_multipart_boundary(content_type: &str) -> Option<String> {
    let lower = content_type.to_ascii_lowercase();
    if !lower.starts_with("multipart/form-data") {
        return None;
    }
    for part in content_type.split(';') {
        let trimmed = part.trim();
        if let Some(boundary) = trimmed.strip_prefix("boundary=") {
            let unquoted = boundary.trim_matches('"');
            if !unquoted.is_empty() {
                return Some(unquoted.to_string());
            }
        }
    }
    None
}

fn extract_multipart_file(
    temp_path: &str,
    boundary: &str,
    requested_dir: &str,
    fallback_filename: &str,
) -> Result<(String, usize), String> {
    let mut temp = fs::File::open(temp_path).map_err(|e| e.to_string())?;
    temp.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    let mut header: Vec<u8> = Vec::with_capacity(2048);
    let mut scan_buf = vec![
        0u8;
        runtime_io_chunk_bytes()
            .min(MULTIPART_HEADER_SCAN_CHUNK_BYTES_MAX)
            .max(256)
    ];
    let headers_end_idx = loop {
        let n = temp.read(&mut scan_buf).map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("missing multipart headers".to_string());
        }
        if header.len() + n > MULTIPART_HEADER_SCAN_MAX_BYTES {
            return Err("multipart headers too large".to_string());
        }
        header.extend_from_slice(&scan_buf[..n]);
        if let Some(idx) = find_subslice(&header, b"\r\n\r\n") {
            break idx;
        }
    };
    let header_str = core::str::from_utf8(&header).map_err(|_| "multipart header decode")?;
    let boundary_line = format!("--{}", boundary);
    let start = header_str.find(&boundary_line).ok_or("missing boundary")?;
    let headers_start = header_str[start..]
        .find("\r\n")
        .map(|v| start + v + 2)
        .ok_or("bad boundary line")?;
    let headers_end = headers_end_idx;
    let header_block = &header_str[headers_start..headers_end];
    let mut filename = None;
    for line in header_block.lines() {
        let lower = line.to_ascii_lowercase();
        if lower.starts_with("content-disposition:") {
            if let Some(idx) = line.find("filename=\"") {
                let rem = &line[idx + "filename=\"".len()..];
                if let Some(end) = rem.find('"') {
                    filename = sanitize_relative_upload_path(&rem[..end]);
                }
            }
        }
    }
    let filename = filename
        .or_else(|| sanitize_relative_upload_path(fallback_filename))
        .unwrap_or_else(|| "upload.epub".to_string());

    let data_start = (headers_end + 4) as u64;
    let marker = format!("\r\n--{}", boundary);
    let data_end = find_marker_in_file(&mut temp, data_start, marker.as_bytes())?
        .ok_or("missing multipart terminator")?;
    if data_end <= data_start {
        return Err("empty multipart data".to_string());
    }
    let data_len = (data_end - data_start) as usize;

    let virtual_target = join_virtual_path(requested_dir, &filename);
    let host_target = virtual_to_host_path(&virtual_target);
    if let Some(parent) = std::path::Path::new(&host_target).parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut out = fs::File::create(&host_target).map_err(|e| e.to_string())?;
    temp.seek(SeekFrom::Start(data_start))
        .map_err(|e| e.to_string())?;
    let mut remaining = data_len;
    let mut buf = vec![0u8; runtime_io_chunk_bytes()];
    while remaining > 0 {
        let want = remaining.min(buf.len());
        temp.read_exact(&mut buf[..want])
            .map_err(|e| e.to_string())?;
        out.write_all(&buf[..want]).map_err(|e| e.to_string())?;
        remaining -= want;
    }
    Ok((virtual_target, data_len))
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    (0..=(haystack.len() - needle.len())).find(|&idx| &haystack[idx..idx + needle.len()] == needle)
}

fn find_marker_in_file(
    file: &mut fs::File,
    start: u64,
    marker: &[u8],
) -> Result<Option<u64>, String> {
    if marker.is_empty() {
        return Ok(None);
    }
    file.seek(SeekFrom::Start(start))
        .map_err(|e| e.to_string())?;
    let mut pos = start;
    let mut carry: Vec<u8> = Vec::new();
    let mut buf = vec![0u8; runtime_io_chunk_bytes()];
    loop {
        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            return Ok(None);
        }
        let mut combined = Vec::with_capacity(carry.len() + n);
        combined.extend_from_slice(&carry);
        combined.extend_from_slice(&buf[..n]);
        if combined.len() >= marker.len() {
            for idx in 0..=(combined.len() - marker.len()) {
                if &combined[idx..idx + marker.len()] == marker {
                    let abs = pos.saturating_sub(carry.len() as u64) + idx as u64;
                    return Ok(Some(abs));
                }
            }
        }
        let keep = marker.len().saturating_sub(1).min(combined.len());
        carry.clear();
        carry.extend_from_slice(&combined[combined.len() - keep..]);
        pos += n as u64;
    }
}

fn largest_8bit_block_bytes() -> usize {
    unsafe { sys::heap_caps_get_largest_free_block(sys::MALLOC_CAP_8BIT as u32) as usize }
}

fn runtime_io_chunk_bytes() -> usize {
    let largest = largest_8bit_block_bytes();
    let candidate = largest.saturating_sub(8 * 1024) / 8;
    candidate.clamp(IO_CHUNK_BYTES_MIN, IO_CHUNK_BYTES_MAX)
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
