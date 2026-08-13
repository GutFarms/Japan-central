@echo off
setlocal EnableExtensions
cd /d "%~dp0"

set "FW=%~dp0Firmware\esp32-2432s028-sha256-miner-merged.bin"
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
echo  Tip: if the port fails, hold BOOT, tap RESET, release BOOT.
echo.

set "COMPORT="
set /p COMPORT=Enter COM port number only (example 6 for COM6): 
if "%COMPORT%"=="" (
  echo No port entered.
  goto MANUAL
)

where py >nul 2>nul
if %ERRORLEVEL%==0 (
  echo.
  echo Trying: py -3 -m esptool ...
  py -3 -m esptool --chip esp32 --port COM%COMPORT% --baud 460800 write_flash -z --flash_mode dio --flash_freq 40m --flash_size 4MB 0x0 "%FW%"
  if %ERRORLEVEL%==0 goto DONE
  echo.
  echo esptool missing or failed. Install with:
  echo   py -3 -m pip install esptool
  echo then run this script again.
  goto MANUAL
)

where python >nul 2>nul
if %ERRORLEVEL%==0 (
  echo.
  echo Trying: python -m esptool ...
  python -m esptool --chip esp32 --port COM%COMPORT% --baud 460800 write_flash -z --flash_mode dio --flash_freq 40m --flash_size 4MB 0x0 "%FW%"
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
echo Flash OK. Unplug/replug without holding BOOT, then open Njörðr seas CYD miner.
pause
exit /b 0
