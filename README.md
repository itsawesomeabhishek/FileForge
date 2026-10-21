# FileForge (Windows)

A local, offline file converter for Windows. Hold **Shift** while
dragging files from Explorer or the desktop and drop them on a radial
wheel of target formats. Nothing is uploaded; originals are never
modified or overwritten.

This is the Windows port of the macOS app. It is built with Tauri 2,
React, and Rust, and bundles FFmpeg for audio, video, and some image
conversions.

## Use

- **Shift + drag** files from Explorer or the desktop, then drop on a
  wedge of the wheel to convert.
- **Ctrl + Alt + O** (or tray menu, Convert Files...) picks files and
  opens the wheel at the cursor. Arrow keys and Enter select, Esc cancels.
- The tray menu opens Settings (output folder, JPEG quality, FFmpeg
  path, excluded apps) and the log.

While the wheel is enabled, Shift+click in Explorer selects only the
clicked file instead of a range. It can be turned off in Settings.

## Supported conversions

- Images: jpg, png, webp, tiff, bmp, gif, svg (input), avif, heic
  (input), and image to pdf
- Audio: mp3, m4a, wav, flac, ogg, opus, aiff, wma
- Video: mp4, mov, mkv, webm, avi, wmv, gif, and video to mp3
- Subtitles: srt, vtt, txt
- Text to pdf

## Develop

Requires Node 22, Rust (MSVC), and WebView2.

```powershell
npm install
powershell scripts/fetch-ffmpeg.ps1   # downloads the FFmpeg sidecars once
npm run tauri dev
npm run tauri build                   # NSIS installer
cd src-tauri; cargo test
```

Project notes live in `context/` (architecture, UI, standards, and the
progress tracker).

## License note

The bundled FFmpeg is the GPL "essentials" build from gyan.dev. Review
its licence before redistributing the installer.
