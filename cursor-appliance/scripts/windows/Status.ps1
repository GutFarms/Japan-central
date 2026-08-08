#Requires -Version 5.1
$ErrorActionPreference = "Continue"
$Root = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path

function Import-DotEnv([string]$Path) {
  if (-not (Test-Path $Path)) { return }
  Get-Content $Path | ForEach-Object {
    $line = $_.Trim()
    if (-not $line -or $line.StartsWith("#") -or -not $line.Contains("=")) { return }
    $pair = $line.Split("=", 2)
    Set-Item -Path "Env:$($pair[0].Trim())" -Value $pair[1].Trim().Trim('"')
  }
}

Import-DotEnv (Join-Path $Root ".env")
$Version = if (Test-Path (Join-Path $Root "VERSION")) { (Get-Content (Join-Path $Root "VERSION") -Raw).Trim() } else { "unknown" }
$Name = if ($env:CURSOR_APPLIANCE_NAME) { $env:CURSOR_APPLIANCE_NAME } else { "cursor-appliance" }
$Cloud = if ($env:CURSOR_APPLIANCE_MANAGEMENT_ADDR) { $env:CURSOR_APPLIANCE_MANAGEMENT_ADDR } else { "127.0.0.1:8733" }
$Local = if ($env:CURSOR_APPLIANCE_LOCAL_ADDR) { $env:CURSOR_APPLIANCE_LOCAL_ADDR } else { "127.0.0.1:8734" }

Write-Host "Cursor appliance v$Version"
Write-Host "  name: $Name"

try {
  $null = Invoke-WebRequest -Uri "http://$Cloud/healthz" -UseBasicParsing -TimeoutSec 2
  Write-Host "  cloud healthz: OK (http://$Cloud/healthz)"
} catch {
  Write-Host "  cloud healthz: unreachable (http://$Cloud/healthz)"
}

try {
  $null = Invoke-WebRequest -Uri "http://$Local/healthz" -UseBasicParsing -TimeoutSec 2
  Write-Host "  local healthz: OK (http://$Local/healthz)"
} catch {
  Write-Host "  local healthz: unreachable (http://$Local/healthz)"
}

$cloudProc = Get-Process -Name "agent" -ErrorAction SilentlyContinue
$localProc = Get-Process -Name "cursor-local-worker" -ErrorAction SilentlyContinue
$guiProc = Get-Process -Name "CursorAppliance","cursor-appliance-gui" -ErrorAction SilentlyContinue

Write-Host "  cloud process: $(if ($cloudProc) { 'running' } else { 'not running' })"
Write-Host "  local process: $(if ($localProc) { 'running' } else { 'not running' })"
Write-Host "  gui process:   $(if ($guiProc) { 'running' } else { 'not running' })"
