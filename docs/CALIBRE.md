# Calibre + Desktop Transfer Guide

This firmware exposes an HTTP transfer server while File Transfer mode is active.

## Discovery

When transfer mode starts:
- HTTP server: `http://<device-ip>/` (port 80)
- mDNS hostname: `http://xteink-x4.local/`
- mDNS services:
1. `_http._tcp` on port `80`
2. `_xteink._tcp` on port `80`

When transfer mode stops, HTTP and mDNS advertising are both stopped.

## Calibre Desktop Usage

### Plugin/manual URL setup

Use one of these base URLs in desktop tooling:
1. `http://xteink-x4.local/` (preferred if `.local` resolves on your OS)
2. `http://<device-ip>/` (always valid fallback)

Calibre-side endpoint contract:
1. `GET /api/status`
2. `GET /api/files?path=/books`
3. `POST /upload?path=/books&filename=<name>.epub`

### Manual upload (no plugin)

Browser:
1. Open `http://xteink-x4.local/` or `http://<device-ip>/`
2. Choose a file and upload to `/books` (default)

Command line:

```bash
curl -i -X POST \
  "http://xteink-x4.local/upload?path=/books&filename=Example.epub" \
  -H "Content-Type: application/epub+zip" \
  --data-binary @Example.epub
```

Raw upload also works with `PUT /upload` using the same query parameters.

## CORS / Preflight

`/upload` supports:
1. `OPTIONS /upload` preflight
2. `Access-Control-Allow-Origin: *`
3. `Access-Control-Allow-Methods: POST, PUT, OPTIONS`

This allows browser-based desktop tools to upload from a different origin.

## Network-drive style compatibility

Current firmware does not expose SMB/WebDAV, so it is not mountable as a native network drive.

Use HTTP paths instead:
1. Upload target directory: `/books` (via `path=/books`)
2. Nested upload path: `/books/<subdir>`
3. Direct download: `GET /api/download?path=/books/<file>.epub`

If a desktop file manager supports custom HTTP upload actions, point uploads to:
`/upload?path=/books&filename=<file-name>`.
