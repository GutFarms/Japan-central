#Requires -Version 5.1
<#
.SYNOPSIS
  Run Cursor Appliance in portable sandbox mode (folder-contained, no Windows Sandbox VM).

  All data/profile/temp writes stay under the unzipped appliance folder.
#>
param(
  [switch]$AllowOnline,
  [switch]$InitGitWorkspace
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "PortableSandbox.ps1")

$Root = Get-ApplianceRoot
Initialize-PortableSandboxLayout -Root $Root -InitGitWorkspace:$InitGitWorkspace
Set-PortableSandboxEnvironment -Root $Root -OfflineDefault:( -not $AllowOnline )

# Merge .env without overriding portable path locks.
$envFile = Join-Path $Root ".env"
if (Test-Path $envFile) {
  Get-Content $envFile | ForEach-Object {
    $line = $_.Trim()
    if (-not $line -or $line.StartsWith("#") -or -not $line.Contains("=")) { return }
    $pair = $line.Split("=", 2)
    $key = $pair[0].Trim()
    $val = $pair[1].Trim().Trim('"')
    $locked = @(
      "CURSOR_APPLIANCE_PORTABLE",
      "CURSOR_APPLIANCE_DATA_DIR",
      "CURSOR_APPLIANCE_WORKER_DIR",
      "USERPROFILE", "HOME", "APPDATA", "LOCALAPPDATA", "TEMP", "TMP"
    )
    if ($locked -contains $key) { return }
    if ($key -eq "CURSOR_APPLIANCE_OFFLINE" -and -not $AllowOnline) { return }
    Set-Item -Path "Env:$key" -Value $val
  }
}

$gui = @(
  (Join-Path $Root "CursorAppliance.exe"),
  (Join-Path $Root "cursor-appliance-gui.exe"),
  (Join-Path $Root "gui\target\release\cursor-appliance-gui.exe"),
  (Join-Path $Root "gui\target\debug\cursor-appliance-gui.exe")
) | Where-Object { Test-Path $_ } | Select-Object -First 1

if (-not $gui) {
  throw "GUI exe not found under $Root"
}

Write-Host "Portable sandbox mode"
Write-Host "  root:      $Root"
Write-Host "  data:      $env:CURSOR_APPLIANCE_DATA_DIR"
Write-Host "  workspace: $env:CURSOR_APPLIANCE_WORKER_DIR"
Write-Host "  profile:   $env:USERPROFILE"
Write-Host "  offline:   $env:CURSOR_APPLIANCE_OFFLINE"
Write-Host "Launching $gui"

$proc = Start-Process -FilePath $gui -WorkingDirectory $Root -PassThru
Write-Host "PID $($proc.Id)"
