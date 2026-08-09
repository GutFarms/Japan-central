#Requires -Version 5.1
<#
.SYNOPSIS
  Bootstrap the Cursor Appliance on Windows.
#>
$ErrorActionPreference = "Stop"
$Root = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path

Write-Host "==> Cursor appliance Windows setup"
Write-Host "    root: $Root"

$envExample = Join-Path $Root ".env.example"
$envFile = Join-Path $Root ".env"
if (-not (Test-Path $envFile) -and (Test-Path $envExample)) {
  Copy-Item $envExample $envFile
  Write-Host "==> Created .env — set CURSOR_API_KEY if you use the cloud worker"
}

$data = Join-Path $Root "data"
New-Item -ItemType Directory -Force -Path $data | Out-Null
@(
  "local-queue\incoming",
  "local-queue\done",
  "local-queue\failed"
) | ForEach-Object {
  New-Item -ItemType Directory -Force -Path (Join-Path $data $_) | Out-Null
}

$agent = Get-Command agent -ErrorAction SilentlyContinue
if (-not $agent) {
  Write-Host "==> Installing Cursor Agent CLI"
  try {
    irm 'https://cursor.com/install?win32=true' | iex
  } catch {
    Write-Warning "Could not auto-install agent CLI: $_"
    Write-Host "Install manually, then re-run Setup.ps1"
  }
} else {
  Write-Host "    agent: $($agent.Source)"
}

$gui = @("CursorAppliance.exe", "cursor-appliance-gui.exe") |
  ForEach-Object { Join-Path $Root $_ } |
  Where-Object { Test-Path $_ } |
  Select-Object -First 1
$local = Join-Path $Root "cursor-local-worker.exe"

Write-Host ""
Write-Host "Done. Next:"
if ($gui) {
  Write-Host "  1) Double-click Start-CursorAppliance.bat"
  Write-Host "     or: & '$gui'"
} else {
  Write-Host "  1) From a source checkout: cd gui; cargo build --release --bins"
}
Write-Host "  2) Optional cloud worker: .\scripts\windows\Start-CloudWorker.ps1"
Write-Host "  3) Open https://cursor.com/agents and select cursor-appliance"
if (Test-Path $local) {
  Write-Host "  Local worker ready: $local"
}
