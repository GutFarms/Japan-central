#Requires -Version 5.1
<#
.SYNOPSIS
  Shared helpers for portable / sandboxed Cursor Appliance launches.
#>

function Get-ApplianceRoot {
  return (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
}

function Initialize-PortableSandboxLayout {
  param(
    [Parameter(Mandatory = $true)][string]$Root,
    [switch]$InitGitWorkspace
  )

  $paths = @(
    "data",
    "data\local-queue\incoming",
    "data\local-queue\done",
    "data\local-queue\failed",
    "sandbox-home",
    "sandbox-home\AppData\Roaming",
    "sandbox-home\AppData\Local",
    "sandbox-workspace",
    "sandbox-tmp"
  )
  foreach ($rel in $paths) {
    New-Item -ItemType Directory -Force -Path (Join-Path $Root $rel) | Out-Null
  }

  $marker = Join-Path $Root "sandbox-workspace\README.txt"
  if (-not (Test-Path $marker)) {
    @"
Portable sandbox workspace
==========================
This folder is the contained worker directory for portable/sandbox mode.
Cloud My Machines needs a git remote here (or point CURSOR_APPLIANCE_WORKER_DIR
at a real checkout outside the sandbox).
"@ | Set-Content -Path $marker -Encoding UTF8
  }

  if ($InitGitWorkspace) {
    $gitDir = Join-Path $Root "sandbox-workspace\.git"
    if (-not (Test-Path $gitDir)) {
      $git = Get-Command git -ErrorAction SilentlyContinue
      if ($git) {
        Push-Location (Join-Path $Root "sandbox-workspace")
        try {
          git init -q
          git config user.email "sandbox@localhost"
          git config user.name "Cursor Appliance Sandbox"
          if (-not (Test-Path ".gitignore")) {
            Set-Content -Path ".gitignore" -Value "`n"
          }
          git add README.txt .gitignore 2>$null
          git commit -qm "sandbox workspace" 2>$null
        } finally {
          Pop-Location
        }
      }
    }
  }
}

function Set-PortableSandboxEnvironment {
  param(
    [Parameter(Mandatory = $true)][string]$Root,
    [switch]$OfflineDefault
  )

  $env:CURSOR_APPLIANCE_PORTABLE = "1"
  $env:CURSOR_APPLIANCE_DATA_DIR = Join-Path $Root "data"
  $env:CURSOR_APPLIANCE_WORKER_DIR = Join-Path $Root "sandbox-workspace"
  $env:CURSOR_APPLIANCE_LOCAL_ADDR = if ($env:CURSOR_APPLIANCE_LOCAL_ADDR) { $env:CURSOR_APPLIANCE_LOCAL_ADDR } else { "127.0.0.1:8734" }
  $env:CURSOR_APPLIANCE_MANAGEMENT_ADDR = if ($env:CURSOR_APPLIANCE_MANAGEMENT_ADDR) { $env:CURSOR_APPLIANCE_MANAGEMENT_ADDR } else { "127.0.0.1:8733" }

  if ($OfflineDefault -and -not $env:CURSOR_APPLIANCE_OFFLINE) {
    $env:CURSOR_APPLIANCE_OFFLINE = "1"
  }

  # Keep profile / temp writes inside the portable folder.
  $home = Join-Path $Root "sandbox-home"
  $env:USERPROFILE = $home
  $env:HOME = $home
  $env:HOMEDRIVE = (Split-Path -Qualifier $home)
  $env:HOMEPATH = ($home.Substring($env:HOMEDRIVE.Length))
  $env:APPDATA = Join-Path $home "AppData\Roaming"
  $env:LOCALAPPDATA = Join-Path $home "AppData\Local"
  $env:TEMP = Join-Path $Root "sandbox-tmp"
  $env:TMP = $env:TEMP
}

function Test-WindowsSandboxAvailable {
  $wsb = Join-Path $env:WINDIR "System32\WindowsSandbox.exe"
  if (-not (Test-Path $wsb)) { return $false }
  try {
    $feature = Get-WindowsOptionalFeature -Online -FeatureName "Containers-DisposableClientVM" -ErrorAction SilentlyContinue
    if ($null -eq $feature) { return (Test-Path $wsb) }
    return $feature.State -eq "Enabled"
  } catch {
    return (Test-Path $wsb)
  }
}
