# Build the FFCE Rust core to WebAssembly and stage the web assets.
# All build artifacts stay inside this project folder (no user/global dirs).
$ErrorActionPreference = "Stop"
$root = $PSScriptRoot
$crate = Join-Path $root "crate"
$web = Join-Path $root "web"

# Keep cargo's target dir inside the project.
$env:CARGO_TARGET_DIR = Join-Path $crate "target"

Write-Host "==> Running native tests (perft)..."
Push-Location $crate
cargo test --release
if ($LASTEXITCODE -ne 0) { Pop-Location; throw "tests failed" }

Write-Host "==> Building wasm32-unknown-unknown release..."
cargo build --release --target wasm32-unknown-unknown
if ($LASTEXITCODE -ne 0) { Pop-Location; throw "wasm build failed" }
Pop-Location

$wasm = Join-Path $crate "target\wasm32-unknown-unknown\release\ffce_engine.wasm"
Copy-Item $wasm (Join-Path $web "ffce_engine.wasm") -Force
Write-Host "==> Copied ffce_engine.wasm to web/"

# Generate the base64-inlined wasm so the page also runs from file://.
$bytes = [System.IO.File]::ReadAllBytes($wasm)
$b64 = [System.Convert]::ToBase64String($bytes)
$inline = "globalThis.FFCE_WASM_BASE64 = `"$b64`";"
Set-Content -Path (Join-Path $web "wasm_inline.js") -Value $inline -Encoding ascii
Write-Host "==> Generated web/wasm_inline.js ($($bytes.Length) bytes)"

Write-Host "Done. Serve the 'web' folder (see serve.ps1) or open web/index.html."
