# FileForge

Convert a file by dropping it on a wheel. FileForge sits in the Windows
system tray. Hold **Shift**, drag a file out of Explorer or off the
desktop, and a wheel of formats appears under the cursor. Drop the file
on a wedge and the new file is written next to the original.

Nothing is uploaded. The original is never changed, moved, or overwritten.
The wheel and the settings window are created when you open them and
closed when you are done, so FileForge does not keep a browser engine
running in the tray. The first drag after the app has been idle waits
a moment while that window starts.

<!-- Demo video.
     Drop a short screen recording on this README in GitHub (it becomes a
     user-attachments link), or save it as docs/media/demo.mp4 and uncomment:

[Watch FileForge convert a file](docs/media/demo.mp4)
-->

## Install

1. Download
   [`FileForge_0.1.1_x64-setup.exe`](https://github.com/itsawesomeabhishek/FileForge/releases/latest)
   and run it.
2. You need Windows 10 or 11, 64-bit. WebView2 is already on current
   Windows 11.
3. Windows SmartScreen may say the publisher is unknown, because the
   installer is not code-signed yet. Choose **More info**, then **Run anyway**.

FileForge does not open a window. Look for its icon in the tray, in the
**^** overflow next to the clock.

<!-- Tray icon after install.
![FileForge in the system tray](docs/media/tray.png)
-->

To remove it, use **Settings > Apps > Installed apps**.

## Convert a file

1. Select one or more files in Explorer, or on the desktop.
2. Hold **Shift**, then drag. Keep Shift held.
3. The wheel opens on the monitor under the cursor. Each wedge is one
   format every selected file can become.
4. Move the pointer onto a wedge. It highlights.
5. Release the mouse on that wedge.

`photo.png` dropped on **JPG** becomes `photo.jpg` in the same folder.
If that name already exists, the new file is `photo 2.jpg`, then
`photo 3.jpg`. The PNG you dragged is still there, unchanged.

<!-- The wheel open over an Explorer window, one wedge highlighted.
![Shift-drag opens the format wheel](docs/media/wheel.png)
-->

Release the mouse anywhere that is not a wedge, including the middle of
the wheel, and nothing is converted.

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
| Open the wheel when I hold Shift… | Turns the drag gesture on or off |
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
| Text: txt | pdf |

Audio, video, gif output, and HEIC or AVIF input use the FFmpeg build
that is inside the installer. You do not install FFmpeg yourself.

## The tray menu

| Item | Action |
|---|---|
| Convert Files… | Same as Ctrl+Alt+O |
| Settings… | Opens the settings window |
| Show Logs | Opens the log, if a conversion failed and you want the reason |
| Quit FileForge | Exits. The wheel will not appear again until you start it |

## One Shift shortcut changes

While the wheel is turned on, **Shift+click** in Explorer selects only the
file you clicked. Windows normally uses Shift+click to select a range.
FileForge takes that click so a Shift-drag does not select everything
between two files. Turn the wheel off in Settings if you want range
selection back. Ctrl+click still adds and removes individual files.

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
