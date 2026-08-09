@echo off
setlocal
cd /d "%~dp0"

echo Cursor Appliance — portable sandbox
echo.

REM Prefer Windows Sandbox VM when available; otherwise folder-contained mode.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\windows\Start-WindowsSandbox.ps1" %*
if errorlevel 1 (
  echo.
  echo Falling back to portable folder sandbox...
  powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\windows\Start-PortableMode.ps1" -InitGitWorkspace
  if errorlevel 1 (
    echo Failed to start portable sandbox.
    pause
    exit /b 1
  )
)
exit /b 0
