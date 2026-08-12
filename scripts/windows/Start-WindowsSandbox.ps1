#Requires -Version 5.1
<#
.SYNOPSIS
  Launch the portable Cursor Appliance inside Windows Sandbox (.wsb).

  Requires Windows Pro/Enterprise with Windows Sandbox enabled.
  Falls back to Start-PortableMode.ps1 when Sandbox is unavailable (unless -RequireSandbox).
#>
param(
  [switch]$Offline,
  [switch]$RequireSandbox,
  [switch]$NoNetworking
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "PortableSandbox.ps1")

$Root = Get-ApplianceRoot
Initialize-PortableSandboxLayout -Root $Root -InitGitWorkspace

if (-not (Test-WindowsSandboxAvailable)) {
  if ($RequireSandbox) {
    throw @"
Windows Sandbox is not available.
Enable it: Settings → Apps → Optional features → More Windows features → Windows Sandbox
(Or: Enable-WindowsOptionalFeature -Online -FeatureName Containers-DisposableClientVM)
"@
  }
  Write-Warning "Windows Sandbox not available — using portable folder sandbox instead."
  $args = @()
  if (-not $Offline) { $args += "-AllowOnline" }
  & (Join-Path $PSScriptRoot "Start-PortableMode.ps1") @args -InitGitWorkspace
  return
}

$sandboxFolder = "C:\Users\WDAGUtilityAccount\Desktop\CursorAppliance"
$bootstrap = Join-Path $sandboxFolder "scripts\windows\Bootstrap-Sandbox.cmd"
$networking = if ($NoNetworking -or $Offline) { "Disable" } else { "Enable" }

# Escape for XML
$hostXml = [System.Security.SecurityElement]::Escape($Root)

$wsb = @"
<Configuration>
  <MappedFolders>
    <MappedFolder>
      <HostFolder>$hostXml</HostFolder>
      <SandboxFolder>$sandboxFolder</SandboxFolder>
      <ReadOnly>false</ReadOnly>
    </MappedFolder>
  </MappedFolders>
  <LogonCommand>
    <Command>cmd.exe /c "$bootstrap"</Command>
  </LogonCommand>
  <Networking>$networking</Networking>
  <ClipboardRedirection>Enable</ClipboardRedirection>
  <ProtectedClient>Enable</ProtectedClient>
  <MemoryInMB>4096</MemoryInMB>
</Configuration>
"@

$wsbPath = Join-Path $env:TEMP "cursor-appliance-portable-sandbox.wsb"
Set-Content -Path $wsbPath -Value $wsb -Encoding UTF8

Write-Host "Starting Windows Sandbox"
Write-Host "  host folder:    $Root"
Write-Host "  sandbox folder: $sandboxFolder"
Write-Host "  networking:     $networking"
Write-Host "  config:         $wsbPath"

Start-Process -FilePath $wsbPath
