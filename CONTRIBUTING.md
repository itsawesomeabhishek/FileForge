# Contributing to FileForge

Thanks for helping improve FileForge, a local, offline file converter for Windows. Bug reports, fixes, new conversions, and documentation improvements are all welcome.

By taking part you agree to follow the [Code of Conduct](CODE_OF_CONDUCT.md).

## Reporting bugs and requesting features

- Search [existing issues](https://github.com/itsawesomeabhishek/FileForge/issues) first.
- For a bug, include your Windows version, the FileForge version, the file type you were converting, the steps to reproduce, and the relevant lines from the log (tray menu, **Show Logs**).
- **Security problems must not be filed as public issues.** See [SECURITY.md](SECURITY.md).

## Development setup

Requirements: Windows 10 or 11, Node 22, Rust (MSVC toolchain), and WebView2.

```powershell
git clone https://github.com/itsawesomeabhishek/FileForge.git
cd FileForge
npm install
powershell scripts/fetch-ffmpeg.ps1   # downloads the FFmpeg sidecars once
npm run tauri dev
```

## Before you open a pull request

Run both checks and make sure they pass:

```powershell
cd src-tauri; cargo test; cd ..
npm run build
```

If you changed the wheel, drag detection, or anything the user sees, also try it in `npm run tauri dev`.

## Project layout

| Path | Purpose |
|---|---|
| `src-tauri/src/` | Rust backend: engines, job queue, settings, tray, wheel, drag monitor |
| `src-tauri/src/engines/` | Conversion engines (image, audio, video, subtitle, text) |
| `src/` | React UI (Settings window and the wheel overlay) |
| `scripts/` | Setup and dev helper scripts |
| `context/` | Architecture, UI conventions, code standards, progress tracker |

Rust owns all file work. React only renders state. Read `context/architecture.md` and `context/code-standards.md` before larger changes.

## Coding standards

The full list is in [`context/code-standards.md`](context/code-standards.md). The essentials:

- **Rust:** no `unwrap()` or `expect()` on paths that can fail from user input; return `Result<_, ForgeError>` with a readable message. Never spawn processes through a shell. Validate every path that comes from the UI.
- **Tests:** unit-test pure logic (formats, conversion matrix, naming, subtitles). `cargo test` must pass.
- **TypeScript:** strict mode, no `any`, and typed boundaries in `src/api.ts`.
- **Safety invariant:** originals are never modified or overwritten. Conversions write a hidden temp file and rename it.
- **Styling:** use the CSS custom properties from `context/ui-context.md`.

## Commits and pull requests

1. Fork the repo and create a branch such as `fix/wheel-position` or `feat/new-format`.
2. Keep each pull request focused on one change.
3. Use clear, imperative commit messages, for example `Fix wheel placement on the second monitor`.
4. Describe what changed and why, and how you tested it. Add screenshots for UI changes.
5. Update `context/progress-tracker.md` when your change affects project status, and the other `context/` files if it changes architecture or standards.

## Licensing of contributions

FileForge is licensed under the [GNU General Public License v3.0](LICENSE). By submitting a contribution you agree that it is licensed under the same terms.

The installer bundles an FFmpeg build licensed under the GPL. Do not add dependencies whose licenses are incompatible with GPL-3.0.
