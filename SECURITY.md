# Security Policy

## Supported Versions

Security fixes are provided for the latest release.

| Version | Supported |
| ------- | --------- |
| 0.1.x   | Yes       |
| < 0.1.0 | No        |

## Reporting a Vulnerability

Please do **not** open a public issue for a security problem.

Email **abhisheq117@gmail.com** with:

1. A description of the problem and its impact.
2. Steps to reproduce, or a sample file or proof of concept.
3. Your Windows version and the FileForge version.
4. Any fix or mitigation you suggest.

### What to expect

- An acknowledgement within 72 hours.
- The report will be reviewed and reproduced privately, and a fix prepared.
- After a fix is released you will be credited, unless you prefer to stay anonymous.

## Scope

FileForge runs entirely on your machine and does not upload files or open network connections for conversion. Examples of issues in scope:

- Reading, writing, or overwriting files outside what the user chose, including path traversal and symlink tricks.
- Command or argument injection when FileForge runs FFmpeg.
- Crashes or code execution triggered by a crafted media, SVG, subtitle, or text file.
- The global mouse hook or drag overlay exposing or capturing input beyond its purpose.
- Tauri command handlers accepting unvalidated input from the UI.

Vulnerabilities in FFmpeg itself should be reported to the [FFmpeg project](https://ffmpeg.org/security.html). Tell us too if the bundled version needs updating.

## Design notes

- Originals are never modified. Output goes to a hidden temp file in the destination folder and is renamed into place.
- Processes are started with argument arrays, never through a shell.
- Paths received from the UI are validated before use.
