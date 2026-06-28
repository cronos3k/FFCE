# Serve the static web/ folder on the first free port in a fallback range.
# No external dependencies beyond Python 3 (used as a static file server).
$ErrorActionPreference = "Stop"
$web = Join-Path $PSScriptRoot "web"
$ports = @(8099, 8123, 8145, 8190, 8222, 8311, 8420)

function Test-PortFree([int]$p) {
  $listeners = [System.Net.NetworkInformation.IPGlobalProperties]::GetIPGlobalProperties().GetActiveTcpListeners()
  return -not ($listeners | Where-Object { $_.Port -eq $p })
}

$port = $null
foreach ($p in $ports) {
  if (Test-PortFree $p) { $port = $p; break }
}
if ($null -eq $port) { throw "No free port found in: $($ports -join ', ')" }

Write-Host "Serving $web at http://127.0.0.1:$port  (Ctrl+C to stop)"
Set-Location $web
python -m http.server $port --bind 127.0.0.1
