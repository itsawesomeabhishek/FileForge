# Code Standards

## General

- Keep modules small and single-purpose.
- Fix root causes; do not layer workarounds.
- Port behavior from the macOS source faithfully; if a
  behavior is changed on Windows, record why in
  `progress-tracker.md`.
- Do not mix conversion logic with UI code.

## Rust

- No `unwrap()` or `expect()` in non-test code paths that can
  fail from user input; return `Result<_, ExploitError>`.
- One error enum (`ExploitError`) with user-readable messages.
- Never spawn processes through a shell; pass argument arrays.
- Validate every path received from the UI before use.
- Unit-test pure logic (formats, matrix, naming, subtitles).
- `cargo test` must pass before moving to the next unit.

## TypeScript

- Strict mode is required.
- Avoid `any`; use explicit interfaces that mirror Rust
  command payloads.
- Treat all values coming from `invoke` and events as
  untrusted and typed at the boundary in `src/api.ts`.

## React

- Function components with hooks only.
- Components render state; side effects live in hooks or
  `src/api.ts`.

## Styling

- Use the CSS custom properties from `ui-context.md`; no
  hardcoded hex values outside the token definitions.
- Follow the border radius scale in `ui-context.md`.

## Data and Storage

- Settings are small JSON; media is never copied into app
  storage.
- Temp output files are hidden (`.exploit-<uuid>.<ext>`) and
  live in the destination directory so the final rename is
  atomic.

## File Organization

- `src-tauri/src/` - Rust backend modules.
- `src-tauri/src/engines/` - conversion engines.
- `src-tauri/binaries/` - FFmpeg sidecars (git-ignored).
- `src/` - React UI.
- `src/components/` - UI components.
- `scripts/` - setup scripts such as the FFmpeg fetcher.
- `context/` - project specs.
