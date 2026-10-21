# Publishes the already-built installer as the GitHub release for a tag.
# Usage: powershell scripts/publish-release.ps1 v0.1.0
param([Parameter(Mandatory = $true)][string]$Tag)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$installer = Get-ChildItem -Path $root -Recurse -Filter "FileForge_*_x64-setup.exe" |
    Where-Object { $_.FullName -match "bundle\\nsis\\" } |
    Select-Object -First 1
if (-not $installer) { throw "Installer not found. Run 'npm run tauri build' first." }

gh release view $Tag 2>$null
if ($LASTEXITCODE -ne 0) {
    gh release create $Tag $installer.FullName --title "FileForge $Tag" --generate-notes
} else {
    gh release upload $Tag $installer.FullName --clobber
}
Write-Output "Published $($installer.Name) to $Tag"
