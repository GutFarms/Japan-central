# Cursor Appliance on Windows

Download a portable build, unzip, and double-click to run. No installer required for the GUI + local worker.

## Download

1. Open the latest GitHub Release for this repo:  
   https://github.com/GutFarms/Japan-central/releases
2. Download **`cursor-appliance-windows-x64-v*.zip`**
3. Unzip to a folder you can write to (for example `Desktop\cursor-appliance`)

CI also uploads the same zip as a workflow artifact named `cursor-appliance-windows-x64` on every push that builds successfully.

## Run (GUI + local failover)

```text
1. Double-click Start-CursorAppliance.bat
2. Use Offline mode or Stop cloud → local worker takes over on 127.0.0.1:8734
```

Or from PowerShell:

```powershell
cd path\to\cursor-appliance-windows-x64
.\scripts\windows\Setup.ps1
.\Start-CursorAppliance.bat
.\scripts\windows\Status.ps1
```

## Optional: Cursor cloud (My Machines) worker

Cloud mode needs the Cursor Agent CLI and a git checkout of the repo:

```powershell
.\scripts\windows\Setup.ps1
# Edit .env and set CURSOR_API_KEY=... (https://cursor.com/dashboard/api)
# Point CURSOR_APPLIANCE_WORKER_DIR at your Japan-central clone
.\scripts\windows\Start-CloudWorker.ps1
```

Then select **cursor-appliance** on https://cursor.com/agents.

## What’s in the zip

| File | Purpose |
|---|---|
| `Start-CursorAppliance.bat` | One-click GUI launcher |
| `CursorAppliance.exe` | egui control panel |
| `cursor-local-worker.exe` | Lightweight local takeover worker |
| `scripts\windows\*.ps1` | Setup / cloud / local / status helpers |
| `.env.example` | Config template (copied to `.env` by Setup) |

## Build from source on Windows

Requirements: [Rust](https://rustup.rs/) stable, MSVC build tools (or mingw).

```powershell
cd Japan-central\cursor-appliance\gui
cargo build --release --bins
cd ..
$env:GUI_EXE="gui\target\release\cursor-appliance-gui.exe"
$env:LOCAL_EXE="gui\target\release\cursor-local-worker.exe"
bash .\scripts\package-windows.sh
# or on pure PowerShell CI: see .github/workflows/windows-release.yml
```

## Notes

- Windows SmartScreen may warn on unsigned exes — choose **More info → Run anyway** for builds you trust.
- Allowlist shell jobs on the local worker use `cmd /C` on Windows.
- Keep `.env` private (`CURSOR_API_KEY`).
