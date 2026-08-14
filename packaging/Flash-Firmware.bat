@echo off
setlocal EnableExtensions
cd /d "%~dp0"

set "FW=%~dp0Firmware\esp32-2432s028-sha256-miner-merged.bin"
set "ESPFLASH=%~dp0Tools\espflash.exe"
if not exist "%FW%" (
  echo Firmware image not found:
  echo   %FW%
  echo Reinstall CYD Miner Setup, or copy the merged.bin into Firmware\
  pause
  exit /b 1
)

echo.
echo  CYD Miner — Flash Firmware
echo  ==========================
echo  Image: Firmware\esp32-2432s028-sha256-miner-merged.bin
echo  Offset: 0x0   Chip: ESP32   Mode: DIO   Size: 4MB   Freq: 40m
echo.
echo  Blank boards: first tries use ROM loader (--no-stub).
echo  Tip: if the port fails, hold BOOT, tap RESET, release BOOT.
echo  SAFETY: if erase runs but rewrite fails, the board may be blank —
echo          keep USB plugged in and retry until Flash OK.
echo.

set "COMPORT="
set /p COMPORT=Enter COM port number only (example 6 for COM6): 
if "%COMPORT%"=="" (
  echo No port entered.
  goto MANUAL
)

REM espflash exact-matches OS port list names (COM6 / COM10 — never \\.\COMx).
set "PORT=COM%COMPORT%"
echo Using port %PORT%

if exist "%ESPFLASH%" (
  echo.
  echo Trying: write-bin --no-stub @ 115200 (blank-board friendly)...
  REM stdin closed via ^<nul — espflash can hang after printing MAC if it waits on a prompt.
  "%ESPFLASH%" --skip-update-check write-bin -p %PORT% -B 115200 -c esp32 --non-interactive --no-stub --before default-reset --after hard-reset 0x0 "%FW%" <nul
  if %ERRORLEVEL%==0 goto DONE
  echo.
  echo Trying: write-bin @ 115200 (with stub)...
  "%ESPFLASH%" --skip-update-check write-bin -p %PORT% -B 115200 -c esp32 --non-interactive --before default-reset --after hard-reset 0x0 "%FW%" <nul
  if %ERRORLEVEL%==0 goto DONE
  echo.
  echo Trying: write-bin --no-stub @ 460800 ...
  "%ESPFLASH%" --skip-update-check write-bin -p %PORT% -B 460800 -c esp32 --non-interactive --no-stub --before default-reset --after hard-reset 0x0 "%FW%" <nul
  if %ERRORLEVEL%==0 goto DONE
  echo.
  echo Auto-reset write failed — hold BOOT, tap RESET, release BOOT, then press a key.
  pause >nul
  "%ESPFLASH%" --skip-update-check write-bin -p %PORT% -B 115200 -c esp32 --non-interactive --no-stub --before no-reset --after hard-reset 0x0 "%FW%" <nul
  if %ERRORLEVEL%==0 goto DONE
  echo.
  echo Trying no-reset with stub...
  pause >nul
  "%ESPFLASH%" --skip-update-check write-bin -p %PORT% -B 115200 -c esp32 --non-interactive --before no-reset --after hard-reset 0x0 "%FW%" <nul
  if %ERRORLEVEL%==0 goto DONE
  echo.
  echo espflash write failed. Trying erase + write...
  "%ESPFLASH%" --skip-update-check erase-flash -p %PORT% -B 115200 -c esp32 --non-interactive --after hard-reset <nul
  echo.
  echo SAFETY: erase finished — rewriting now. Keep USB connected.
  echo Hold BOOT, tap RESET, release BOOT, then press a key.
  pause >nul
  timeout /t 2 /nobreak >nul
  "%ESPFLASH%" --skip-update-check write-bin -p %PORT% -B 115200 -c esp32 --non-interactive --no-stub --before no-reset --after hard-reset 0x0 "%FW%" <nul
  if %ERRORLEVEL%==0 goto DONE
  echo Rewrite with no-stub failed — trying default-reset no-stub...
  "%ESPFLASH%" --skip-update-check write-bin -p %PORT% -B 115200 -c esp32 --non-interactive --no-stub --before default-reset --after hard-reset 0x0 "%FW%" <nul
  if %ERRORLEVEL%==0 goto DONE
  echo espflash failed — will try Python esptool if available.
)

where py >nul 2>nul
if %ERRORLEVEL%==0 (
  echo.
  echo Trying: py -3 -m esptool --no-stub ...
  py -3 -m esptool version >nul 2>nul
  if errorlevel 1 (
    echo esptool module missing — installing with pip...
    py -3 -m pip install --upgrade --disable-pip-version-check esptool
  )
  echo Hold BOOT, tap RESET, release BOOT if connect stalls, then press a key.
  pause >nul
  py -3 -m esptool --chip esp32 --port %PORT% --baud 115200 --before no_reset --no-stub write_flash -z --flash_mode dio --flash_freq 40m --flash_size 4MB 0x0 "%FW%"
  if %ERRORLEVEL%==0 goto DONE
  echo.
  echo esptool failed. Install with:
  echo   py -3 -m pip install esptool
  echo then run this script again.
  goto MANUAL
)

where python >nul 2>nul
if %ERRORLEVEL%==0 (
  echo.
  echo Trying: python -m esptool --no-stub ...
  python -m esptool version >nul 2>nul
  if errorlevel 1 (
    python -m pip install --upgrade --disable-pip-version-check esptool
  )
  echo Hold BOOT, tap RESET, release BOOT if connect stalls, then press a key.
  pause >nul
  python -m esptool --chip esp32 --port %PORT% --baud 115200 --before no_reset --no-stub write_flash -z --flash_mode dio --flash_freq 40m --flash_size 4MB 0x0 "%FW%"
  if %ERRORLEVEL%==0 goto DONE
)

:MANUAL
echo.
echo Opening flash guide + browser flasher...
start "" "%~dp0FLASH-WINDOWS.txt"
start "" "https://espressif.github.io/esptool-js/"
start "" "%~dp0Firmware"
echo.
echo In the browser flasher, select the merged.bin from the Firmware folder @ 0x0.
pause
exit /b 1

:DONE
echo.
echo Flash OK. Unplug/replug without holding BOOT, then open Njörðr Seas' CYD miner.
pause
exit /b 0
