; Abode installer for Windows (NSIS).
;
; Build:  makensis /DVERSION=0.1.0 /DEXE=path\to\abode.exe /DOUTFILE=Abode-0.1.0-windows-setup.exe packaging\windows\abode.nsi
;
; Installs for the current user only (no admin prompt) into
; %LOCALAPPDATA%\Programs\Abode, adds Start menu and desktop shortcuts,
; opens .abode files, and registers an uninstaller in Settings > Apps.
;
; The in-app updater runs this with /S (silent): it waits for Abode to quit,
; installs over the old version and starts Abode again.

Unicode true
!include "MUI2.nsh"
!include "FileFunc.nsh"

!ifndef VERSION
  !define VERSION "0.0.0"
!endif
!ifndef EXE
  !define EXE "..\..\target\release\abode.exe"
!endif
!ifndef OUTFILE
  !define OUTFILE "Abode-${VERSION}-windows-setup.exe"
!endif

!define APPNAME "Abode"
!define UNINSTKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\Abode"

Name "${APPNAME}"
OutFile "${OUTFILE}"
InstallDir "$LOCALAPPDATA\Programs\Abode"
InstallDirRegKey HKCU "Software\Abode" "InstallDir"
RequestExecutionLevel user
SetCompressor /SOLID lzma
BrandingText "Abode ${VERSION}"

VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "Abode"
VIAddVersionKey "FileDescription" "Abode installer"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "LegalCopyright" "Abode"

!define MUI_ICON "..\..\crates\abode-app\assets\icon\icon.ico"
!define MUI_UNICON "..\..\crates\abode-app\assets\icon\icon.ico"
!define MUI_ABORTWARNING
!define MUI_WELCOMEPAGE_TITLE "Install Abode"
!define MUI_WELCOMEPAGE_TEXT "Abode is the owner's manual for your home: floor plans, appliances, paint, service history and reminders, all in one file.$\r$\n$\r$\nClick Install to continue."
!define MUI_FINISHPAGE_RUN "$INSTDIR\abode.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Open Abode now"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Section "Abode" SecMain
  ; When updating from inside Abode, give it a moment to quit.
  IfSilent 0 +2
    Sleep 2000

  SetOutPath "$INSTDIR"
  ; Retry a few times in case the old version is still closing.
  StrCpy $0 0
  retry:
    ClearErrors
    SetOverwrite try
    File "/oname=abode.exe" "${EXE}"
    IfErrors 0 copied
    IntOp $0 $0 + 1
    IntCmp $0 10 failed
    Sleep 1000
    Goto retry
  failed:
    SetOverwrite on
    File "/oname=abode.exe" "${EXE}"
  copied:
  SetOverwrite on

  WriteUninstaller "$INSTDIR\Uninstall Abode.exe"
  WriteRegStr HKCU "Software\Abode" "InstallDir" "$INSTDIR"

  ; Shortcuts
  CreateShortcut "$SMPROGRAMS\Abode.lnk" "$INSTDIR\abode.exe"
  IfSilent +2 0
    CreateShortcut "$DESKTOP\Abode.lnk" "$INSTDIR\abode.exe"

  ; Open .abode files with Abode
  WriteRegStr HKCU "Software\Classes\.abode" "" "Abode.House"
  WriteRegStr HKCU "Software\Classes\Abode.House" "" "Abode house file"
  WriteRegStr HKCU "Software\Classes\Abode.House\DefaultIcon" "" "$INSTDIR\abode.exe,0"
  WriteRegStr HKCU "Software\Classes\Abode.House\shell\open\command" "" '"$INSTDIR\abode.exe" "%1"'
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'

  ; Settings > Apps entry
  WriteRegStr HKCU "${UNINSTKEY}" "DisplayName" "Abode"
  WriteRegStr HKCU "${UNINSTKEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINSTKEY}" "Publisher" "Abode"
  WriteRegStr HKCU "${UNINSTKEY}" "DisplayIcon" "$INSTDIR\abode.exe"
  WriteRegStr HKCU "${UNINSTKEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINSTKEY}" "UninstallString" '"$INSTDIR\Uninstall Abode.exe"'
  WriteRegStr HKCU "${UNINSTKEY}" "QuietUninstallString" '"$INSTDIR\Uninstall Abode.exe" /S'
  WriteRegDWORD HKCU "${UNINSTKEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINSTKEY}" "NoRepair" 1
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD HKCU "${UNINSTKEY}" "EstimatedSize" "$0"

  ; After a silent update, start the new version.
  IfSilent 0 +2
    Exec '"$INSTDIR\abode.exe"'
SectionEnd

Section "Uninstall"
  Delete "$INSTDIR\abode.exe"
  Delete "$INSTDIR\Uninstall Abode.exe"
  RMDir "$INSTDIR"
  Delete "$SMPROGRAMS\Abode.lnk"
  Delete "$DESKTOP\Abode.lnk"
  DeleteRegKey HKCU "Software\Classes\Abode.House"
  ReadRegStr $0 HKCU "Software\Classes\.abode" ""
  StrCmp $0 "Abode.House" 0 +2
    DeleteRegKey HKCU "Software\Classes\.abode"
  DeleteRegKey HKCU "${UNINSTKEY}"
  DeleteRegKey HKCU "Software\Abode"
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
  ; Houses in Documents\Abode are left alone.
SectionEnd
