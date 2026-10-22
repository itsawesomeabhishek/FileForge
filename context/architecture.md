# Architecture Context

## Stack

| Layer      | Technology                        | Role                                   |
| ---------- | --------------------------------- | -------------------------------------- |
| Shell      | Tauri 2                           | Window, tray, packaging, IPC           |
| Backend    | Rust (edition 2024)               | Formats, engines, job queue, settings  |
| UI         | React 19 + TypeScript + Vite      | Presentation only                      |
| Media      | FFmpeg/ffprobe sidecar            | Audio, video, and fallback image work  |
| Images     | Rust `image` crate, `resvg`       | Native image conversion                |
| Plugins    | dialog, opener, autostart         | File picker, Explorer reveal, startup  |

## System Boundaries

- `src-tauri/src/formats.rs` - file format enum, categories,
  aliases, detection from path.
- `src-tauri/src/matrix.rs` - which target formats each source
  offers; output file naming and unique-name helpers.
- `src-tauri/src/ffmpeg.rs` - locates the sidecar, health check,
  duration probe, encoder list (cached for the session), streaming
  progress runner.
- `src-tauri/src/engines/` - one module per engine plus a
  registry that picks the first engine supporting a pair.
- `src-tauri/src/jobs.rs` - serial job queue, status, progress
  events, cancellation.
- `src-tauri/src/settings.rs` - persisted settings.
- `src-tauri/src/tray.rs` - tray icon and menu.
- `src-tauri/src/dragmon.rs` - low-level mouse hook plus a
  controller thread; decides when a modifier-drag from Explorer or
  the desktop should open the wheel. Shift opens convert. Ctrl
  opens image, video, audio, and PDF tools, which write a new file
  and leave the original. Crop opens its own window first. Once
  open, the wheel stays until the mouse button is released.
- `src-tauri/src/wheel.rs` - the full-monitor overlay window
  and its show/hide/mode events. The window is parked on the
  monitor at full size while hidden, including at mouse-down,
  so a drop's physical coordinates match the wheel. The page
  converts those pixels with the webview scale. After the wheel
  closes, the same window shrinks into a progress card until
  the conversion finishes, then returns to full size on the next
  press. It is destroyed after a few idle minutes. The settings
  window is created the first time Settings is opened, and
  closing it destroys it. The process stays running because a
  window close does not exit the tray app.
- `src-tauri/src/cropwin.rs` - the crop editor. It shows one
  image or video frame; the page reports the kept rectangle and
  Rust writes a new file. The original is not changed. The window
  is created from an async command so WebView2 does not deadlock.
- `src-tauri/src/picker.rs` - menu/hotkey entry: file dialog, then
  the wheel in picker mode; global shortcut registration.
- `src/` - React UI. Calls Tauri commands, listens to events.
  Holds no conversion logic.

## Storage Model

- **Settings JSON** in the app config directory: output
  location, reveal flag, JPEG quality, FFmpeg path, compression
  preset.
- **Log file** in the app data directory for diagnostics.
- **Media files** are only ever read from the user's chosen
  paths and written as new files in the chosen output folder.
  No database.

## Auth and Access Model

- No accounts, no network access for conversions.
- Tauri capabilities grant only the plugins the UI needs.
- File access happens in Rust on explicit user-selected paths.

## Invariants

1. An original file is never modified, moved, or deleted.
2. Output is written to a hidden temp file in the destination
   directory and renamed on success; an existing file is never
   overwritten (`name 2.ext`).
3. Failed or cancelled jobs remove their temp file.
4. Jobs run one at a time.
5. A conversion is only offered if an engine supports it.
6. Conversion logic lives in Rust, never in the React layer.
7. The wheel never moves or deletes originals: it reports copy
   semantics for drops and only reads dragged paths.
8. The drag hook only forwards events; it never blocks, and
   decisions run on the controller thread.
9. Drag sources are limited to Explorer and the desktop, minus the
   user's excluded executables, latched at mouse-down.
