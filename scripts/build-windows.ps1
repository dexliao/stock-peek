# Builds a Windows NSIS installer with Tauri.
# Requires: Node.js, pnpm, Rust stable 1.90+, MSVC Build Tools, WebView2 runtime.

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

if (-not (Get-Command pnpm -ErrorAction SilentlyContinue)) {
  throw "pnpm was not found. Install pnpm 9+ first."
}
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
  throw "cargo was not found. Install Rust stable 1.90+ (MSVC toolchain) first."
}

pnpm install

if ($LASTEXITCODE -ne 0) {
  throw "pnpm install failed with exit code $LASTEXITCODE"
}

pnpm tauri build

if ($LASTEXITCODE -ne 0) {
  throw "Tauri build failed with exit code $LASTEXITCODE"
}

$bundleDir = Join-Path $root "src-tauri\target\release\bundle\nsis"
$installers = @(Get-ChildItem -Path $bundleDir -Filter "*-setup.exe" -ErrorAction SilentlyContinue)

if ($installers.Count -eq 0) {
  throw "Build finished but no NSIS installer was found under $bundleDir"
}

$installers | ForEach-Object {
  Write-Host "Installer: $($_.FullName)"
}
