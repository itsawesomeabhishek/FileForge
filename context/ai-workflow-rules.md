# AI Workflow Rules

## Approach

Build incrementally and spec-first. The macOS source in
`C:\Users\Abhishek\exploit\NativeApp\Sources` is the
behavioral reference; the context files define what and how
for the Windows port. Do not invent behavior that is not in
the reference or these specs.

## Scoping Rules

- Work on one feature unit at a time.
- Prefer small, verifiable increments over large speculative
  changes.
- Do not combine unrelated system boundaries in a single step.

## When to Split Work

Split an implementation step if it combines:

- Backend engine work and UI work.
- More than one engine family (for example FFmpeg-driven and
  pure-Rust).
- Behavior not clearly defined in the context files or the
  macOS reference.

If a change cannot be verified end to end quickly, the scope
is too broad; split it.

## Handling Missing Requirements

- Do not invent product behavior not defined in the context
  files or the macOS reference.
- If a requirement is ambiguous, resolve it in the relevant
  context file before implementing.
- If a requirement is missing, add it as an open question in
  `progress-tracker.md` before continuing.

## Protected Files

Do not modify unless explicitly instructed:

- `C:\Users\Abhishek\exploit\**` (the macOS reference source).
- `Skills/**` and `.mcp.json` in this repository.
- The plan file in `.cursor/plans`.

## Keeping Docs in Sync

Update the relevant context file whenever implementation
changes system architecture, storage decisions, standards, or
feature scope.

## Before Moving to the Next Unit

1. The current unit works end to end within its scope.
2. No invariant defined in `architecture.md` was violated.
3. `progress-tracker.md` reflects the completed work.
4. `cargo test` and `npm run build` pass.
