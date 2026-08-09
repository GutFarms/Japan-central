@echo off
setlocal
cd /d "%~dp0"

if not exist "%~dp0data" mkdir "%~dp0data"

if exist "%~dp0CursorAppliance.exe" (
  start "" "%~dp0CursorAppliance.exe"
  exit /b 0
)

if exist "%~dp0cursor-appliance-gui.exe" (
  start "" "%~dp0cursor-appliance-gui.exe"
  exit /b 0
)

if exist "%~dp0scripts\windows\Start-Gui.ps1" (
  powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\windows\Start-Gui.ps1"
  exit /b %ERRORLEVEL%
)

echo CursorAppliance.exe not found.
echo Download the Windows release zip or build from source.
pause
exit /b 1
