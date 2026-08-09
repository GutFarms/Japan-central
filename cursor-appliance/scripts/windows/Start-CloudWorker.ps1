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

if ($env:CURSOR_APPLIANCE_OFFLINE -match '^(1|true|yes|on)$') {
  throw "CURSOR_APPLIANCE_OFFLINE is set — cloud worker disabled. Use Start-LocalWorker.ps1 or turn offline mode off."
}

$agent = Get-Command agent -ErrorAction SilentlyContinue
if (-not $agent) {
  throw "Cursor Agent CLI (agent) not found. Run .\scripts\windows\Setup.ps1 first."
}

$Name = if ($env:CURSOR_APPLIANCE_NAME) { $env:CURSOR_APPLIANCE_NAME } else { "cursor-appliance" }
$WorkerDir = if ($env:CURSOR_APPLIANCE_WORKER_DIR) { $env:CURSOR_APPLIANCE_WORKER_DIR } else { (Resolve-Path (Join-Path $Root "..")).Path }
$Mgmt = if ($env:CURSOR_APPLIANCE_MANAGEMENT_ADDR) { $env:CURSOR_APPLIANCE_MANAGEMENT_ADDR } else { "127.0.0.1:8733" }
$DataDir = if ($env:CURSOR_APPLIANCE_DATA_DIR) { $env:CURSOR_APPLIANCE_DATA_DIR } else { Join-Path $Root "data" }
$Idle = if ($env:CURSOR_APPLIANCE_IDLE_RELEASE_TIMEOUT) { $env:CURSOR_APPLIANCE_IDLE_RELEASE_TIMEOUT } else { "0" }

if (-not (Test-Path (Join-Path $WorkerDir ".git"))) {
  throw "Worker dir is not a git checkout: $WorkerDir"
}

New-Item -ItemType Directory -Force -Path $DataDir | Out-Null

$workerArgs = @(
  "worker",
  "--name", $Name,
  "--worker-dir", $WorkerDir,
  "--management-addr", $Mgmt,
  "--data-dir", $DataDir,
  "--idle-release-timeout", $Idle
)

$agentArgs = @()
if ($env:CURSOR_API_KEY) {
  $agentArgs += @("--api-key", $env:CURSOR_API_KEY)
} else {
  Write-Host "info: no CURSOR_API_KEY; using local agent login session"
}

$agentArgs += $workerArgs
if ($env:CURSOR_APPLIANCE_DEBUG -eq "1") {
  $agentArgs += @("--debug")
}
$agentArgs += @("start")
if ($env:CURSOR_APPLIANCE_DEBUG -eq "1") {
  $agentArgs += @("--verbose")
}

Write-Host "Starting Cursor cloud worker"
Write-Host "  name:   $Name"
Write-Host "  repo:   $WorkerDir"
Write-Host "  health: http://$Mgmt/healthz"

& agent @agentArgs
