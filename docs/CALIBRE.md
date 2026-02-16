# Calibre Integration Reuse Plan

## Why this doc

This repository already has the beginnings of wireless upload support (`crates/xteink-firmware/src/web_upload.rs`) and we want a clear path to Calibre-compatible transfers without overbuilding.

## Integration paths (highest value first)

1. Calibre Device Plugin compatibility (desktop "Send to device")
- Target UX: user opens `File Transfer` mode on device, Calibre desktop discovers device or connects by IP, sends EPUBs wirelessly.
- Device side needs a stable HTTP upload contract and simple status endpoint.
- This gives the best user experience for existing Calibre users.

2. Calibre Content Server / OPDS pull (device downloads)
- Device can browse/download from Calibre OPDS feed over Wi-Fi.
- Useful as fallback when plugin compatibility is incomplete.
- Requires OPDS/browser work in UI + authenticated HTTP client path.

3. Generic web upload form (already useful)
- Browser uploads via `POST /upload`.
- Works immediately for non-Calibre users and as test harness for plugin path.

## Reusable assets from `crosspoint-reader`

The `crosspoint-reader` codebase in this workspace already has production-tested patterns we can mirror:

- Web server route surface and lifecycle
  - `crosspoint-reader/src/network/CrossPointWebServer.cpp`
- Multipart upload pipeline with bounded buffering and watchdog resets
  - `crosspoint-reader/src/network/CrossPointWebServer.cpp:522`
- Calibre-focused activity UX and instructions
  - `crosspoint-reader/src/activities/network/CalibreConnectActivity.cpp`
- User docs for file transfer flow
  - `crosspoint-reader/USER_GUIDE.md:78`
  - `crosspoint-reader/docs/webserver-endpoints.md`

## Reusable assets already in this repo

- Firmware-side upload scaffold (Rust, esp-idf)
  - `crates/xteink-firmware/src/web_upload.rs`
  - Current endpoints: `GET /upload/health`, `POST /upload` (queued event model)
- Main loop hooks for upload server start/poll/stop
  - `crates/xteink-firmware/src/main.rs`

## Recommended protocol contract (v1)

Keep this minimal first and Calibre-friendly:

- `GET /api/status`
  - Returns firmware name/version, free space, and upload capability flags.
- `POST /upload?path=/books/...`
  - Accept `multipart/form-data` and raw body uploads.
  - Overwrite existing file atomically.
  - Return structured JSON (`ok`, `path`, `bytes`, `error`).
- `GET /api/files?path=/books`
  - List files/folders for plugin-side browse.

Optional (v2):
- WebSocket binary upload endpoint for large transfers.
- UDP discovery beacon for zero-config desktop discovery.

## Calibre desktop compatibility notes

- Calibre plugin integration is the fastest route to native "Send to device" UX.
- We should implement the device HTTP contract first, then adapt a plugin shim if needed.
- If we reuse plugin code/resources from other projects, verify license compatibility before copying.

## Implementation checklist

1. Stabilize web upload server behavior
- [ ] Enforce upload size limits and SD write streaming.
- [ ] Add file extension policy (`.epub`, optional `.txt`, `.md`).
- [ ] Add robust error mapping (400/409/413/500).

2. Add Calibre-facing endpoints
- [ ] `GET /api/status`
- [ ] `GET /api/files`
- [ ] JSON responses with fixed schema and version field.

3. Add UI entrypoint
- [ ] `File Transfer` activity in UI.
- [ ] Show IP + short instructions: "In Calibre: Send to device".

4. Sync library after upload
- [ ] Invalidate/rescan library cache after successful writes.
- [ ] Ensure metadata/cover cache refresh for overwritten EPUBs.

5. Desktop validation
- [ ] Test with Calibre desktop + plugin.
- [ ] Test fallback via curl/browser upload.

## Suggested near-term decision

- Proceed with HTTP `v1` contract first (no WebSocket required).
- Keep server mode session-based (starts in File Transfer screen, stops on exit).
- Add plugin/discovery only after v1 is stable on device.
