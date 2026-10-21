# Progress Tracker

Update this file after every meaningful implementation
change.

## Current Phase

- Phase 1 (core conversion): complete.
- Wheel Phase 1 (drag-to-wheel convert + picker): complete.

## Current Goal

Match the macOS app's interaction model on Windows: hold a
modifier while dragging files and choose a conversion from a
radial wheel, with the same shortcuts and tool set. Delivered in
phases; only the convert wheel exists so far.

## Completed

### Core conversion (earlier)

- Environment verified: Node 22, Rust 1.99 MSVC, VS Build
  Tools 2022, WebView2, Tauri 2.12.
- Context files filled in.
- Tauri config (`com.abhishek.fileforge`, NSIS target, FFmpeg
  `externalBin`), plugins, capabilities, `.gitignore`.
- FFmpeg/ffprobe sidecars fetched by `scripts/fetch-ffmpeg.ps1`
  into `src-tauri/binaries/` (gyan.dev essentials build, GPL).
- `formats.rs`, `matrix.rs`, `ffmpeg.rs`, engines (audio, video,
  subtitle, image, document), `jobs.rs` queue, `settings.rs`,
  `diag.rs`, `commands.rs`, `tray.rs`, single-instance,
  close-to-tray, `--minimized` login launch.
- React UI: drop zone and picker, file list, target chips, job
  list, Settings.
- Installer built: `FileForge_0.1.0_x64-setup.exe` (56.7 MiB).

### Wheel Phase 1 (this change)

- `dragmon.rs`: `WH_MOUSE_LL` hook forwards button down/up to a
  controller thread. On press it checks the window under the
  cursor is Explorer or the desktop (`CabinetWClass`, `Progman`,
  `WorkerW`) and the process is not in the excluded-apps list
  (latched at mouse-down). While the button is down it polls
  cursor and modifiers at ~60 Hz; after 24 px of movement with
  Shift held it opens the wheel. Releasing Shift mid-drag hides
  it; releasing the button hides it after a 350 ms grace.
- Shift-press fix: Explorer applies Shift+click as a range selection
  at mouse-down, so Shift-dragging a file selected every file
  between it and the previous selection. The mouse hook now
  swallows a Shift press on Explorer/the desktop and replays it
  with Shift released (`press_without_shift`, tagged with
  `MARKER`); a keyboard hook tracks the physical Shift state
  (`SHIFT_KEYS`) so the wheel still sees Shift held. Verified: with
  five files, Shift-press on the fourth selects 1 file (5 with the
  wheel disabled), the drop still converts, Shift is not left
  stuck. Side effect: plain Shift+click range-select in Explorer
  is unavailable while the wheel is enabled.
- Modifier mapping (`mode_for`): Shift = convert, Alt+Shift =
  tools, Ctrl+Shift = file actions. Only convert is enabled
  (`mode_enabled`) until the other wheels have content.
- `wheel.rs`: one pre-created transparent, always-on-top,
  non-activating overlay window. It covers the monitor under the
  cursor so a drop anywhere lands on it (cancel) instead of on
  the app below, like the macOS full-screen panel. Wheel centre is
  clamped to keep the 170 px disc on screen.
- `picker.rs`: global hotkey Ctrl+Alt+O and tray "Convert Files…"
  open a file dialog, then the wheel at the cursor in picker mode.
- React `WheelApp.tsx` + `radial.ts` + `wheel.css`: SVG port of
  `RadialGeometry`/`WedgeShape` (inner 64, outer 150, gap 4, hit
  region to outer+24, wedge 0 at the top, clockwise), centre
  pill, empty-state messages copied from `RadialController`,
  accent highlight, spring-in animation.
  - Drag mode: Tauri `onDragDropEvent` enter/over/drop drive
    highlight and drop; dropping off a wedge cancels. The wheel is
    drawn only after a drag-enter with file paths arrives.
  - Picker mode: arrows step, Enter selects, Esc or a click off a
    wedge cancels, mouse hover highlights.
- Settings: `wheelEnabled` toggle and `excludedApps` list
  (normalised to lower-case exe names).
- Tray: "Open FileForge", "Convert Files…" (Ctrl+Alt+O), Settings,
  Show Logs, Quit.
- Removed the Convert window (drop zone, file list, target chips,
  job list) and its dead CSS and API wrappers. The main window is
  now Settings only, hidden at startup, opened from the tray;
  tray left-click opens the menu. Rust job commands remain for the
  upcoming progress pill.
- Dev helpers: `scripts/sim-drag.ps1` performs a real Explorer
  drag with modifiers held (refuses to press the mouse unless an
  Explorer window is under the file); `scripts/cdp-eval.mjs` gained
  `CDP_TARGET=wheel`, `CDP_WHEN=<expr>` polling and `--file`.
- Verification:
  - `cargo test`: 70 pass (adds mode mapping, shell classes,
    exclusion, drag threshold, wheel clamping, settings).
  - Real end to end with `sim-drag.ps1`: Shift-drag `sample.png`
    in Explorer, wheel appears with JPG/WEBP/TIFF/AVIF/BMP/PDF,
    drop on the top wedge creates `sample.jpg`, the original is
    still in place; dropping on the centre cancels with no job.
  - Excluded app `explorer.exe` suppresses the wheel.
  - Ctrl+Alt+O opens the file dialog; choosing a file opens the
    focused picker wheel; real Right, Right, Enter keys convert to
    WEBP.
  - Hit testing checked for all six wedges of a 6-item wheel.
- Renamed the project to **FileForge** (was "exploit" / `rust_tool`):
  - Product name, window titles, tray text, installer name
    (`FileForge_0.1.0_x64-setup.exe`), and the app identifier
    (`com.abhishek.fileforge`).
  - Crate `fileforge`, library `fileforge_lib`, npm package
    `fileforge`, error enum `ForgeError`, temp prefix
    `.fileforge-`, log file `fileforge.log`.
  - The identifier decides the config/log folders, so settings
    saved by an older build under `com.abhishek.exploit` are not
    carried over.
  - References to the macOS source app `exploit` are unchanged.
  - `cargo test` (70) and `npm run build` pass.
- Added community files: `LICENSE` (GPL-3.0, chosen because the
  bundled FFmpeg build is GPL), `CODE_OF_CONDUCT.md` (Contributor
  Covenant 2.1), `CONTRIBUTING.md`, `SECURITY.md`; license metadata
  in `Cargo.toml` and `package.json`; README Contributing and
  License sections.

## In Progress

- Nothing.

## Next Up

- Wheel Phase 2: file-actions wheel (Reveal, Copy Files, Copy
  Paths, Duplicate, SHA-256), progress pill and card with the
  citrus companion, four-tab Settings (modifier display, excluded
  apps UI polish), hotkeys for tools and file actions.
- Wheel Phase 3: simple tool windows (compress, metadata, resize,
  rotate, create PDF, PDF merge/split, audio trim/normalize).
- Wheel Phase 4: editor tools (crop, photo edit, background frame,
  video trim/crop/speed/join/frames/split, audio visualizer).
- Wheel Phase 5: redact, collage, organize PDF, bleep audio,
  update checker.
- Make the modifiers configurable in Settings; add the optional
  "any app" drag source.

## Open Questions

- HEIC output is hidden: the image crate and the bundled FFmpeg
  cannot encode it. HEIC input is attempted through FFmpeg and may
  fail on builds without HEIF support.
- docx, pdf-to-image/txt/docx, and txt-to-jpg/png are deferred and
  not offered.
- The bundled FFmpeg is the GPL "essentials" build (libx264).
  Review licensing before distributing the installer.
- A settings value once showed `jpegQuality` 1.0 during testing
  without a clear writer; first-run defaults and every update path
  re-tested as 0.9. Watch for recurrence.
- Windows has no global drag pasteboard, so a Shift rubber-band
  selection in Explorer also opens the (invisible) overlay until
  the button is released; nothing is drawn because no drag-enter
  arrives. Shift+rubber-band therefore cannot extend a selection
  while the wheel is enabled.
- There is no visible progress for wheel conversions yet (the
  Explorer reveal setting is the only signal); the progress pill is
  the first Phase 2 item.
- Ctrl+Alt+O may clash with another app's hotkey; a failed
  registration is logged and the tray entry still works.
- Mixed-DPI multi-monitor placement of the overlay is untested.

## Architecture Decisions

- Rust owns all file work; React is presentation only.
  Keeps behavior testable and mirrors the Swift engine split.
- FFmpeg is bundled as a Tauri sidecar so the app works out
  of the box; Settings can override the path like the Mac app.
  Resolution order: configured path, next to the exe (installed
  and `tauri dev`), `src-tauri/binaries` (tests), then `PATH`.
- The conversion matrix is the macOS list filtered by
  `engines::supports`, then at runtime by `engines::is_available`
  (FFmpeg present, libaom for AVIF), so the UI never offers a
  conversion that cannot work.
- The image engine module is named `images` to avoid clashing with
  the `image` crate.
- The wheel overlay window must be created visible (off screen)
  and hidden right after. WebView2 only creates the child windows
  that receive OLE drops once shown, and wry registers its drop
  targets on those children at creation; a window built hidden
  never receives drag-enter.
- The wheel returns copy semantics (wry reports DROPEFFECT_COPY),
  so a Shift-drag onto it never moves the originals.
- The wheel window is full-monitor, not wheel-sized, so stray drops
  are swallowed. It is transparent and non-focusable; picker mode
  temporarily makes it focusable to receive keys.
- The same bundle serves both windows. `main.tsx` picks the view by
  window label and imports each view lazily so the overlay never
  loads `App.css` (which would paint an opaque background).

## Session Notes

- Reference source: `C:\Users\Abhishek\exploit\NativeApp\Sources`.
- Plan: `.cursor/plans/windows_exploit_port_3c428464.plan.md`.
- Run: `powershell scripts/fetch-ffmpeg.ps1` once, then
  `npm run tauri dev`. To drive the live webview from scripts, set
  `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222`
  and use `node scripts/cdp-eval.mjs` (`CDP_TARGET=wheel` for the
  overlay; its dev trace is `window.__wheelTrace`).
- Cursor's built-in browser auto-opens `localhost:1420`; it has no
  Tauri IPC, so errors there are expected and unrelated.
