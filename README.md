# FileForge

Convert a file by dropping it on a wheel. FileForge sits in the Windows
system tray. Drag a file out of Explorer or off the desktop, then press
**Shift** to convert it or **Ctrl** for image tools. Drop the file on a
wedge and the new file is written next to the original.

Nothing is uploaded. The original is never changed, moved, or overwritten.
The wheel starts loading as soon as you press the mouse button, and a
card at the bottom of the screen shows each conversion until the new
file is ready. After a few minutes with nothing open, that window
closes so FileForge does not keep a browser engine running in the tray.

<!-- Demo video.
     Drop a short screen recording on this README in GitHub (it becomes a
     user-attachments link), or save it as docs/media/demo.mp4 and uncomment:

[Watch FileForge convert a file](docs/media/demo.mp4)
-->

## Install

1. Download
   [`FileForge_0.1.5_x64-setup.exe`](https://github.com/itsawesomeabhishek/FileForge/releases/latest)
   and run it.
2. You need Windows 10 or 11, 64-bit. WebView2 is already on current
   Windows 11.
3. Windows SmartScreen may say the publisher is unknown, because the
   installer is not code-signed yet. Choose **More info**, then **Run anyway**.

FileForge does not open a window. Look for its icon in the tray, in the
**^** overflow next to the clock. It also starts when you sign in to
Windows. Turn that off under **Settings…** if you do not want it.

<!-- Tray icon after install.
![FileForge in the system tray](docs/media/tray.png)
-->

To remove it, use **Settings > Apps > Installed apps**.

## Convert a file

1. Select one or more files in Explorer, or on the desktop.
2. Start dragging, then press **Shift**. You can also hold Shift before
   the drag. The wheel stays open until you drop, even if you let go of
   the key.
3. The wheel opens on the monitor under the cursor. Each wedge is one
   format every selected file can become.
4. Move the pointer onto a wedge. It highlights, and a short tick plays
   each time you move to a different wedge.
5. Release the mouse on that wedge.

`photo.png` dropped on **JPG** becomes `photo.jpg` in the same folder.
If that name already exists, the new file is `photo 2.jpg`, then
`photo 3.jpg`. The PNG you dragged is still there, unchanged.

<!-- The wheel open over an Explorer window, one wedge highlighted.
![Shift-drag opens the format wheel](docs/media/wheel.png)
-->

Release the mouse anywhere that is not a wedge, including the middle of
the wheel, and nothing is converted.

## File tools

Press **Ctrl** instead of Shift. The wheel follows the kind of file
you dragged, and every tool writes a new file beside the original.

- Images: Compress, Metadata, Add background.
- Video, including GIF: Compress, Mute, Snapshot, Metadata.
- Audio: Compress, Normalize, Mono, Metadata, Visualize (an MP4 of the waveform).

A drag that mixes images with video, or a PDF, has no tools yet.
Crop, trim, redact, speed, split, and bleep need their own window.
PDF, Word, Pages, slides, spreadsheets, and EPUB are not on the wheel
until FileForge can write those files.

The wheel only lists formats that work for **every** file in the drag.
A PNG and a WAV dragged together have no format in common, so the wheel
says there is no shared conversion. Drag them separately.

<!-- A mixed selection with the empty-wheel message.
![No shared conversion](docs/media/no-shared.png)
-->

A few files cannot be converted at all (folders, and types FileForge does
not read). Those are skipped, and the wheel says the selection includes
unsupported files.

## Pick files instead of dragging

Press **Ctrl+Alt+O**, or open the tray menu and choose **Convert Files…**.

A normal file dialog opens. After you choose files, the same wheel appears
at the cursor and waits for a choice:

| Key | Action |
|---|---|
| Left / Right, or Up / Down | Move between wedges |
| Enter | Convert to the highlighted format |
| Esc, or a click off a wedge | Cancel |

<!-- The wheel in picker mode, with the file dialog behind it.
![Picker wheel](docs/media/picker.png)
-->

## Where the new file goes

The tray menu's **Settings…** controls this. The default is **Next to the
original**. You can send every conversion to **Downloads** or the
**Desktop** instead.

**Show the file in Explorer when done** opens the folder and selects the
new file after each conversion.

<!-- Settings window.
![Settings](docs/media/settings.png)
-->

Other settings:

| Setting | What it does |
|---|---|
| Start FileForge when I sign in | Adds it to Windows startup |
| Open the wheel when I press Shift or Ctrl… | Turns the drag gesture on or off. Shift converts. Ctrl opens image tools |
| Ignore drags from these apps | One `.exe` name per line. Drags that start in those programs never open the wheel |
| JPEG quality | Used when the target is JPG. Default is high |
| FFmpeg | Shows whether the bundled converter is working. You can point this at your own `ffmpeg.exe` |

## What you can convert

The wheel never offers a conversion it cannot do, and it never offers the
format a file already is.

| From | To |
|---|---|
| Images: jpg, png, webp, tiff, bmp, gif, avif, heic, svg | jpg, png, webp, tiff, bmp, gif, avif, pdf. SVG and HEIC can be read, not written |
| Audio: mp3, m4a, wav, flac, ogg, opus, aiff, wma | any other audio format in that list |
| Video: mp4, mov, mkv, webm, avi, wmv | any other video format in that list, plus gif and mp3 |
| Subtitles: srt, vtt | srt, vtt, txt |
| Text: txt, md, html | txt, md, html, pdf. PDF input keeps the text layer |
| Spreadsheets: csv, tsv | csv, tsv, pdf |
| Archives: zip, tar, gzip | zip, tar, gzip (gzip is a .tar.gz) |

Audio, video, gif output, and HEIC or AVIF input use the FFmpeg build
that is inside the installer. You do not install FFmpeg yourself.

## The tray menu

| Item | Action |
|---|---|
| Convert Files… | Same as Ctrl+Alt+O |
| Settings… | Opens the settings window |
| Show Logs | Opens the log, if a conversion failed and you want the reason |
| Quit FileForge | Exits. The wheel will not appear again until you start it |

## Two Explorer shortcuts change

While the wheel is turned on, **Shift+click** in Explorer selects only the
file you clicked. Windows normally uses Shift+click to select a range.
**Ctrl+click** also selects only that file, instead of adding or removing
it from the selection. FileForge takes those clicks so a drag does not
change which files are selected before the wheel opens. Turn the wheel
off in Settings if you want the usual Explorer selection back.

## Develop

Windows 10 or 11, Node 22, Rust (MSVC), and WebView2.

```powershell
npm install
powershell scripts/fetch-ffmpeg.ps1   # once; downloads the FFmpeg sidecars
npm run tauri dev
npm run tauri build                   # builds the installer
cd src-tauri; cargo test
```

`context/` holds the architecture notes, UI rules, and the progress tracker.

## Contributing

Issues and pull requests are welcome. Read [CONTRIBUTING.md](CONTRIBUTING.md)
and the [Code of Conduct](CODE_OF_CONDUCT.md) first. Report security
problems privately, as described in [SECURITY.md](SECURITY.md).

## License

FileForge is released under the [GNU General Public License v3.0](LICENSE).

The installer includes the GPL "essentials" FFmpeg build from gyan.dev.
That build has its own licence. Read it before you redistribute the installer.
