# Downloads the Windows FFmpeg "essentials" build (gyan.dev) and installs
# ffmpeg/ffprobe as Tauri sidecars in src-tauri/binaries.
# Note: this build is GPL (it includes libx264), required for mp4/mov/mkv output.
$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
$dest = Join-Path $root "src-tauri\binaries"
$triple = "x86_64-pc-windows-msvc"
$ffmpegOut = Join-Path $dest "ffmpeg-$triple.exe"
$ffprobeOut = Join-Path $dest "ffprobe-$triple.exe"

if ((Test-Path $ffmpegOut) -and (Test-Path $ffprobeOut)) {
    Write-Output "FFmpeg sidecars already present in $dest"
    exit 0
}

New-Item -ItemType Directory -Force -Path $dest | Out-Null
$tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("exploit-ffmpeg-" + [guid]::NewGuid())
New-Item -ItemType Directory -Force -Path $tmp | Out-Null

try {
    $zip = Join-Path $tmp "ffmpeg.zip"
    $url = "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip"
    Write-Output "Downloading $url"
    Invoke-WebRequest -Uri $url -OutFile $zip -UseBasicParsing
    Expand-Archive -Path $zip -DestinationPath $tmp -Force

    $ffmpeg = Get-ChildItem -Path $tmp -Recurse -Filter "ffmpeg.exe" | Select-Object -First 1
    $ffprobe = Get-ChildItem -Path $tmp -Recurse -Filter "ffprobe.exe" | Select-Object -First 1
    if (-not $ffmpeg -or -not $ffprobe) { throw "ffmpeg.exe or ffprobe.exe not found in archive" }

    Copy-Item $ffmpeg.FullName $ffmpegOut -Force
    Copy-Item $ffprobe.FullName $ffprobeOut -Force
    Write-Output "Installed $ffmpegOut"
    Write-Output "Installed $ffprobeOut"
}
finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}
