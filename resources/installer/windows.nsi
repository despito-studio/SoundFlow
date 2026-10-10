Unicode true
!include "MUI2.nsh"
Name "SoundFlow"
OutFile "${OUTPUT}"
InstallDir "$LOCALAPPDATA\SoundFlow"
InstallDirRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\SoundFlow" "InstallLocation"
RequestExecutionLevel user
ManifestDPIAware true
SetCompressor /SOLID lzma
VIProductVersion "3.2.0.0"
VIAddVersionKey "ProductName" "SoundFlow"
VIAddVersionKey "FileDescription" "SoundFlow native music application installer"
VIAddVersionKey "FileVersion" "3.2.0"
VIAddVersionKey "ProductVersion" "3.2.0"
VIAddVersionKey "LegalCopyright" "SoundFlow"
!define MUI_ICON "${PROJECT}\resources\icon.ico"
!define MUI_UNICON "${PROJECT}\resources\icon.ico"
!define MUI_ABORTWARNING
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "Ukrainian"
!insertmacro MUI_LANGUAGE "English"

Section "SoundFlow" SEC_MAIN
    SetShellVarContext current
    SetOutPath "$INSTDIR"
    SetOverwrite on
    File /r "${STAGE}\*"
    Delete "$INSTDIR\soundflow-provider.exe"
    Delete "$INSTDIR\provider-build.json"
    WriteUninstaller "$INSTDIR\Uninstall.exe"
    CreateDirectory "$SMPROGRAMS\SoundFlow"
    CreateShortcut "$SMPROGRAMS\SoundFlow\SoundFlow.lnk" "$INSTDIR\SoundFlow.exe" "" "$INSTDIR\SoundFlow.exe" 0
    CreateShortcut "$DESKTOP\SoundFlow.lnk" "$INSTDIR\SoundFlow.exe" "" "$INSTDIR\SoundFlow.exe" 0
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\SoundFlow" "DisplayName" "SoundFlow"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\SoundFlow" "DisplayVersion" "3.2.0"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\SoundFlow" "Publisher" "SoundFlow"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\SoundFlow" "InstallLocation" "$INSTDIR"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\SoundFlow" "DisplayIcon" "$INSTDIR\SoundFlow.exe,0"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\SoundFlow" "UninstallString" '"$INSTDIR\Uninstall.exe"'
    WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\SoundFlow" "NoModify" 1
    WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\SoundFlow" "NoRepair" 1
SectionEnd

Section "Uninstall"
    SetShellVarContext current
    Delete "$INSTDIR\SoundFlow.exe"
    Delete "$INSTDIR\READ-ME.txt"
    Delete "$INSTDIR\Uninstall.exe"
    RMDir /r "$INSTDIR\resources"
    RMDir /r "$INSTDIR\media-provider"
    RMDir /r "$INSTDIR\licenses"
    RMDir "$INSTDIR"
    Delete "$DESKTOP\SoundFlow.lnk"
    Delete "$SMPROGRAMS\SoundFlow\SoundFlow.lnk"
    RMDir "$SMPROGRAMS\SoundFlow"
    DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\SoundFlow"
SectionEnd
