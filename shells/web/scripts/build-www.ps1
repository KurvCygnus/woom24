# Builds the self-contained static artifact into shells/web/www/pkg/
# (no CDN, no runtime fetches). Generated output; the repo carries script + sources.
# Twin script: build-www.sh (bash). Keep the two in lockstep when changing.
# Usage (from anywhere):
#   powershell -NoProfile -ExecutionPolicy Bypass -File shells/web/scripts/build-www.ps1
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..\..\..")

# The wasm-bindgen CLI and crate must stay in lockstep (pinned 0.2.121).
$RequiredWasmBindgen = "0.2.121"

# Crate side: the version recorded in Cargo.lock for the exact package
# (exact line match so "wasm-bindgen-macro" & friends never confuse the lookup).
$lockLines = Get-Content Cargo.lock
$crateVersion = $null
for ($i = 0; $i -lt $lockLines.Count; $i++) {
    if ($lockLines[$i] -eq 'name = "wasm-bindgen"') {
        $crateVersion = ($lockLines[$i + 1] -replace 'version = "(.+)"', '$1')
        break
    }
}
if ($crateVersion -ne $RequiredWasmBindgen) {
    [Console]::Error.WriteLine("error: wasm-bindgen crate in Cargo.lock is '$(if ($null -eq $crateVersion) { "missing" } else { $crateVersion })', expected $RequiredWasmBindgen")
    exit 1
}

# CLI side: present, then in lockstep.
if (-not (Get-Command wasm-bindgen -ErrorAction SilentlyContinue)) {
    [Console]::Error.WriteLine("error: wasm-bindgen CLI not found (expected $RequiredWasmBindgen)")
    [Console]::Error.WriteLine("       install it with: cargo install wasm-bindgen-cli --version $RequiredWasmBindgen")
    exit 1
}
$cliVersion = ((wasm-bindgen --version) -split " ")[1]
if ($cliVersion -ne $RequiredWasmBindgen) {
    [Console]::Error.WriteLine("error: wasm-bindgen CLI is '$cliVersion', expected $RequiredWasmBindgen")
    [Console]::Error.WriteLine("       install it with: cargo install wasm-bindgen-cli --version $RequiredWasmBindgen")
    exit 1
}

Write-Host "[1/3] cargo build (wasm32, release)"
cargo build -p room-shell-web --target wasm32-unknown-unknown --release
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "[2/3] wasm-bindgen --target web"
wasm-bindgen --target web --out-dir shells/web/www/pkg --no-typescript target/wasm32-unknown-unknown/release/room_shell_web.wasm
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "[3/3] artifact size"
Get-Item shells/web/www/pkg/room_shell_web_bg.wasm | Format-List Name, Length
Write-Host "Local preview: python -m http.server 8000 --directory shells/web/www"
[console]::beep(880, 250)
