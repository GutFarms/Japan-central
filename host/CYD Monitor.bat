@echo off
setlocal
cd /d "%~dp0"

if not exist ".venv\Scripts\pythonw.exe" (
  echo Setting up CYD Monitor...
  py -3 -m venv .venv 2>nul || python -m venv .venv
  call ".venv\Scripts\activate.bat"
  python -m pip install --upgrade pip
  python -m pip install -r requirements.txt
)

REM pythonw hides the console/terminal window
start "" ".venv\Scripts\pythonw.exe" desktop_app.py
endlocal
