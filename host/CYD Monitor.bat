@echo off
setlocal
cd /d "%~dp0"

if not exist ".venv\Scripts\python.exe" (
  echo Setting up CYD Monitor...
  py -3 -m venv .venv 2>nul || python -m venv .venv
  call ".venv\Scripts\activate.bat"
  python -m pip install --upgrade pip
  python -m pip install -r requirements.txt
) else (
  call ".venv\Scripts\activate.bat"
)

python desktop_app.py
if errorlevel 1 pause
endlocal
