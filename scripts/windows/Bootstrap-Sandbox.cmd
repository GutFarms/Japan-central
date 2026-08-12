@echo off
setlocal
REM Runs inside Windows Sandbox after mapped-folder logon.
set "ROOT=%~dp0..\.."
cd /d "%ROOT%"

powershell -NoProfile -ExecutionPolicy Bypass -File "%ROOT%\scripts\windows\Start-PortableMode.ps1" -InitGitWorkspace
if errorlevel 1 (
  echo Portable sandbox bootstrap failed.
  pause
)
