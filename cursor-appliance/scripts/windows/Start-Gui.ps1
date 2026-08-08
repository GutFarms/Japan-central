#Requires -Version 5.1
$ErrorActionPreference = "Stop"
$Root = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path

$gui = @(
  (Join-Path $Root "CursorAppliance.exe"),
  (Join-Path $Root "cursor-appliance-gui.exe"),
  (Join-Path $Root "gui\target\release\cursor-appliance-gui.exe"),
  (Join-Path $Root "gui\target\debug\cursor-appliance-gui.exe")
) | Where-Object { Test-Path $_ } | Select-Object -First 1

if (-not $gui) {
  throw "GUI exe not found. Download the Windows zip or build: cd gui; cargo build --release --bins"
}

Write-Host "Launching $gui"
Start-Process -FilePath $gui -WorkingDirectory $Root
