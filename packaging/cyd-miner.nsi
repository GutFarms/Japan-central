; CYD Miner Kit — Windows setup wizard (NSIS)
; Installs Companion app + firmware image + flash docs for any Windows PC.

!include "MUI2.nsh"
!include "FileFunc.nsh"

!define PRODUCT_NAME "Njörðr Seas' CYD miner"
!define PRODUCT_PUBLISHER "GutFarms"
!define PRODUCT_VERSION "0.8.50"
!define PRODUCT_WEB "https://github.com/GutFarms/Japan-central"

Name "${PRODUCT_NAME} ${PRODUCT_VERSION}"
OutFile "..\dist\CYD-Miner-Setup.exe"
Unicode true
RequestExecutionLevel admin
InstallDir "$PROGRAMFILES64\Njordr Seas' CYD miner"
InstallDirRegKey HKLM "Software\CYDMiner" "Install_Dir"
BrandingText "${PRODUCT_NAME} ${PRODUCT_VERSION}"
SetCompressor /SOLID lzma

!define MUI_ABORTWARNING
!define MUI_ICON "${NSISDIR}\Contrib\Graphics\Icons\modern-install.ico"
!define MUI_UNICON "${NSISDIR}\Contrib\Graphics\Icons\modern-uninstall.ico"
!define MUI_WELCOMEPAGE_TITLE "Njörðr Seas' CYD miner Setup"
!define MUI_WELCOMEPAGE_TEXT "Install everything needed to run the ESP32-2432S028 (CYD) USB Bitcoin SHA-256 miner on this Windows PC.$\r$\n$\r$\nIncludes:$\r$\n  • Njörðr Seas' CYD miner (pool + USB control)$\r$\n  • In-app Update board (push firmware over USB)$\r$\n  • Firmware image + espflash helper$\r$\n$\r$\nAfter setup: Connect USB → Update board (or flash once) → Start mining."
!define MUI_FINISHPAGE_RUN "$INSTDIR\cyd-companion.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Launch Njörðr Seas' CYD miner"
!define MUI_FINISHPAGE_SHOWREADME "$INSTDIR\START-HERE.txt"
!define MUI_FINISHPAGE_SHOWREADME_TEXT "Open START-HERE guide"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "LICENSE-windows.txt"
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Section "Njörðr Seas' CYD miner (required)" SecApp
  SectionIn RO
  SetOutPath $INSTDIR
  File "..\dist\cyd-miner-kit\cyd-companion.exe"
  File "..\dist\cyd-miner-kit\START-HERE.txt"
  File "..\dist\cyd-miner-kit\FLASH-WINDOWS.txt"
  File "..\dist\cyd-miner-kit\COMPANION.md"
  File "..\dist\cyd-miner-kit\Flash-Firmware.bat"
  File "..\dist\cyd-miner-kit\VERSION.txt"

  SetOutPath "$INSTDIR\Firmware"
  File "..\dist\cyd-miner-kit\Firmware\esp32-2432s028-sha256-miner-merged.bin"
  File "..\dist\cyd-miner-kit\Firmware\SHA256SUMS.txt"
  File "..\dist\cyd-miner-kit\Firmware\FLASH.md"
  File "..\dist\cyd-miner-kit\Firmware\VERSION.txt"

  SetOutPath "$INSTDIR\Tools"
  File "..\dist\cyd-miner-kit\Tools\espflash.exe"

  WriteRegStr HKLM "Software\CYDMiner" "Install_Dir" "$INSTDIR"
  WriteRegStr HKLM "Software\CYDMiner" "Version" "${PRODUCT_VERSION}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\CYDMiner" "DisplayName" "${PRODUCT_NAME} ${PRODUCT_VERSION}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\CYDMiner" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\CYDMiner" "DisplayVersion" "${PRODUCT_VERSION}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\CYDMiner" "Publisher" "${PRODUCT_PUBLISHER}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\CYDMiner" "URLInfoAbout" "${PRODUCT_WEB}"
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\CYDMiner" "NoModify" 1
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\CYDMiner" "NoRepair" 1
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\CYDMiner" "EstimatedSize" "$0"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
SectionEnd

Section "Start Menu shortcuts" SecMenu
  CreateDirectory "$SMPROGRAMS\Njordr Seas' CYD miner"
  CreateShortCut "$SMPROGRAMS\Njordr Seas' CYD miner\Njörðr Seas' CYD miner.lnk" "$INSTDIR\cyd-companion.exe"
  CreateShortCut "$SMPROGRAMS\Njordr Seas' CYD miner\Flash Firmware.lnk" "$INSTDIR\Flash-Firmware.bat"
  CreateShortCut "$SMPROGRAMS\Njordr Seas' CYD miner\START-HERE.lnk" "$INSTDIR\START-HERE.txt"
  CreateShortCut "$SMPROGRAMS\Njordr Seas' CYD miner\Firmware Folder.lnk" "$INSTDIR\Firmware"
  CreateShortCut "$SMPROGRAMS\Njordr Seas' CYD miner\Uninstall.lnk" "$INSTDIR\Uninstall.exe"
SectionEnd

Section "Desktop shortcuts" SecDesktop
  CreateShortCut "$DESKTOP\Njörðr Seas' CYD miner.lnk" "$INSTDIR\cyd-companion.exe"
  CreateShortCut "$DESKTOP\CYD Flash Firmware.lnk" "$INSTDIR\Flash-Firmware.bat"
SectionEnd

LangString DESC_SecApp ${LANG_ENGLISH} "Companion app, firmware image, and docs (required)."
LangString DESC_SecMenu ${LANG_ENGLISH} "Start Menu entries for Companion, flash helper, and docs."
LangString DESC_SecDesktop ${LANG_ENGLISH} "Desktop shortcuts for Companion and flash helper."
!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
  !insertmacro MUI_DESCRIPTION_TEXT ${SecApp} $(DESC_SecApp)
  !insertmacro MUI_DESCRIPTION_TEXT ${SecMenu} $(DESC_SecMenu)
  !insertmacro MUI_DESCRIPTION_TEXT ${SecDesktop} $(DESC_SecDesktop)
!insertmacro MUI_FUNCTION_DESCRIPTION_END

Section "Uninstall"
  Delete "$INSTDIR\cyd-companion.exe"
  Delete "$INSTDIR\START-HERE.txt"
  Delete "$INSTDIR\FLASH-WINDOWS.txt"
  Delete "$INSTDIR\COMPANION.md"
  Delete "$INSTDIR\Flash-Firmware.bat"
  Delete "$INSTDIR\VERSION.txt"
  Delete "$INSTDIR\Uninstall.exe"
  Delete "$INSTDIR\Firmware\esp32-2432s028-sha256-miner-merged.bin"
  Delete "$INSTDIR\Firmware\SHA256SUMS.txt"
  Delete "$INSTDIR\Firmware\FLASH.md"
  Delete "$INSTDIR\Firmware\VERSION.txt"
  RMDir "$INSTDIR\Firmware"
  Delete "$INSTDIR\Tools\espflash.exe"
  RMDir "$INSTDIR\Tools"
  RMDir "$INSTDIR"

  Delete "$SMPROGRAMS\Njordr Seas' CYD miner\Njörðr Seas' CYD miner.lnk"
  Delete "$SMPROGRAMS\Njordr Seas' CYD miner\Flash Firmware.lnk"
  Delete "$SMPROGRAMS\Njordr Seas' CYD miner\START-HERE.lnk"
  Delete "$SMPROGRAMS\Njordr Seas' CYD miner\Firmware Folder.lnk"
  Delete "$SMPROGRAMS\Njordr Seas' CYD miner\Uninstall.lnk"
  RMDir "$SMPROGRAMS\Njordr Seas' CYD miner"
  Delete "$DESKTOP\Njörðr Seas' CYD miner.lnk"
  Delete "$DESKTOP\CYD Flash Firmware.lnk"

  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\CYDMiner"
  DeleteRegKey HKLM "Software\CYDMiner"
SectionEnd
