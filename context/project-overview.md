# FileForge (Windows)

## Overview

FileForge is a local, offline file converter and file-tool
app. This repository is the Windows port of the macOS
SwiftUI app at `C:\Users\Abhishek\exploit`. It lives in the
system tray, lets the user drag files onto a radial wheel (or pick them with a hotkey), and converts
them between image, audio, video, subtitle, and text formats
using a bundled FFmpeg and native Rust codecs. Nothing is
uploaded; every operation runs on the user's machine.

## Goals

1. Reproduce the macOS app's conversion behavior exactly
   (same format list, same conversion matrix, same output
   naming, same codec settings).
2. Never overwrite or modify an original file.
3. Work out of the box on Windows 10/11 with no manual
   FFmpeg install.

## Core User Flow

1. User launches FileForge; it sits in the system tray.
2. User drags files from Explorer or the desktop and presses
   Shift to convert or Ctrl for image tools (or presses
   Ctrl+Alt+O / uses the tray menu to pick files). There is no
   main window; Settings opens from the tray.
3. A radial wheel lists the targets common to all the files.
4. Dropping on a wedge starts the conversion.
5. Jobs run one at a time; the output appears next to the
   original and can be shown in Explorer.

## Features

### Conversion (Phase 1)

- Image: jpg, png, webp, tiff, bmp, svg (input), gif, plus
  heic/avif when FFmpeg supports them; image to pdf.
- Audio: mp3, m4a, wav, flac, ogg, opus, aiff, wma.
- Video: mp4, mov, mkv, webm, avi, wmv, gif; video to mp3.
- Subtitles: srt, vtt, and subtitle to txt; txt to srt/vtt.
- Text: txt, markdown, and html, including pdf output. PDF
  input keeps the text layer.
- Spreadsheets: csv and tsv, including pdf output.
- Archives: zip, tar, and gzip (written as .tar.gz).

### App shell

- System tray icon and menu (Convert Files, Settings, Show
  Logs, Quit).
- Settings: output location, reveal in Explorer, JPEG
  quality, FFmpeg path override, launch at login.
- Job queue with progress, cancel, Open, Show in Explorer.

## Scope

### In Scope

- Phase 1: everything listed under Features above.

### In progress, from the Tangerine set

Shift opens convert. Ctrl opens tools (Tangerine uses Command
for that wheel). The wheel stays open until the drop. A mixed
selection has no tools. File actions stay a later wheel.

Working on Ctrl, when every file is the same kind:

- Images: Compress, Metadata, Add background. Crop opens an
  editor where the kept area can be dragged or typed in pixels.
- Video and GIF: Compress, Mute, Snapshot, Metadata, Speed
  (always twice as fast), Split (two files at the midpoint).
  Crop opens the same editor and applies that area to every frame.
- Audio: Compress, Normalize, Channels (mixes to mono), Metadata,
  Visualize.
- PDF: Compress, Metadata, Split (one file per page).

Still needs the user to mark the file, so it is not on the wheel:
redact, edit, annotate, trim, and bleep. PDF organize needs a page
order.

PDF text and markdown are on the Shift wheel. PDF pages as
images, DOCX, Pages, PPTX, XLSX, ODS, EPUB, and RAR are not,
because FileForge cannot write those files yet. HEIC can be
read, not written. Pages, AirDrop, and Float are Mac-only.

### Out of Scope

- Phase 1 excludes the editing tools that are not listed as
  working above (photo edit, redact, resize, rotate, collage,
  PDF merge/organize, video trim/join/frames, audio trim/bleep).
  They are later phases, one feature unit each. Crop, video
  speed and split, and PDF compress, metadata, and split are
  on the Ctrl wheel. Audio normalize, mono, and waveform video
  are on the Ctrl wheel.
- RAR input. ZIP, TAR, and GZIP convert to each other.
- docx output and pdf to image/docx/txt (deferred, hidden
  rather than offered and failing).
- The macOS Shift-drag radial menu and notch pill, which have
  no Windows equivalent.

## Success Criteria

1. wav to mp3, mp4 to gif, srt to vtt, and png to jpg all
   produce valid output files from the running app.
2. Unsupported conversions are never offered in the UI.
3. `cargo test` and `npm run build` pass.
4. `npm run tauri build` produces a Windows installer.
