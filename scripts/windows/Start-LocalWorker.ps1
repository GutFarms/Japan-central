#Requires -Version 5.1
$ErrorActionPreference = "Stop"
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

$Name = if ($env:CURSOR_APPLIANCE_NAME) { $env:CURSOR_APPLIANCE_NAME } else { "cursor-appliance" }
$WorkerDir = if ($env:CURSOR_APPLIANCE_WORKER_DIR) { $env:CURSOR_APPLIANCE_WORKER_DIR } else { (Resolve-Path (Join-Path $Root "..")).Path }
if (-not (Test-Path (Join-Path $WorkerDir ".git"))) {
  # Packaged layout may keep worker_dir as the appliance folder itself
  $WorkerDir = $Root
}
$DataDir = if ($env:CURSOR_APPLIANCE_DATA_DIR) { $env:CURSOR_APPLIANCE_DATA_DIR } else { Join-Path $Root "data" }
$LocalAddr = if ($env:CURSOR_APPLIANCE_LOCAL_ADDR) { $env:CURSOR_APPLIANCE_LOCAL_ADDR } else { "127.0.0.1:8734" }

New-Item -ItemType Directory -Force -Path $DataDir | Out-Null

$Bin = @(
  (Join-Path $Root "cursor-local-worker.exe"),
  (Join-Path $Root "gui\target\release\cursor-local-worker.exe"),
  (Join-Path $Root "gui\target\debug\cursor-local-worker.exe")
) | Where-Object { Test-Path $_ } | Select-Object -First 1

if (-not $Bin) {
  throw "cursor-local-worker.exe not found. Run Setup.ps1 / download the Windows zip / build with cargo."
}

Write-Host "Starting local worker"
Write-Host "  name:   $Name"
Write-Host "  repo:   $WorkerDir"
Write-Host "  health: http://$LocalAddr/healthz"

& $Bin `
  --name $Name `
  --worker-dir $WorkerDir `
  --data-dir $DataDir `
  --listen $LocalAddr
