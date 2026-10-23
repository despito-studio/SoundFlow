; Private extraction backend. The public entry point is the native frontend.
Unicode true
!include "MUI2.nsh"
!include "FileFunc.nsh"
Name "SoundFlow"
OutFile "${OUTPUT}"
InstallDir "$LOCALAPPDATA\SoundFlow"
RequestExecutionLevel user
ManifestDPIAware true
SilentInstall silent
AutoCloseWindow true
SetCompressor /SOLID lzma
VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "SoundFlow"
VIAddVersionKey "FileDescription" "SoundFlow installation payload"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "LegalCopyright" "SoundFlow"
!define MUI_ICON "${PROJECT}\resources\icon.ico"
!define MUI_UNICON "${PROJECT}\resources\icon.ico"
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "Ukrainian"
!insertmacro MUI_LANGUAGE "English"

Function .onInit
    ${GetParameters} $0
    ClearErrors
    ${GetOptions} $0 "/EXTRACT" $1
    IfErrors 0 +3
        SetErrorLevel 10
        Quit
    ${GetFileName} "$INSTDIR" $0
    StrCpy $1 $0 17
    StrCmp $1 ".SoundFlow-stage-" +3
        SetErrorLevel 11
        Quit
FunctionEnd

Section "Extract"
    ; Caller supplies a fresh staging directory. Never register or launch from staging.
    FindFirst $0 $1 "$INSTDIR\*"
    empty_check:
        StrCmp $1 "" empty_done
        StrCmp $1 "." empty_next
        StrCmp $1 ".." empty_next
        FindClose $0
        SetErrorLevel 12
        Quit
    empty_next:
        FindNext $0 $1
        Goto empty_check
    empty_done:
    FindClose $0
    SetOutPath "$INSTDIR"
    SetOverwrite off
    ClearErrors
    File /r "${STAGE}\*"
    IfErrors 0 +3
        SetErrorLevel 13
        Quit
    WriteUninstaller "$INSTDIR\Uninstall.exe"
    IfErrors 0 +3
        SetErrorLevel 14
        Quit
    SetErrorLevel 0
SectionEnd

Section "Uninstall"
    SetShellVarContext current
    Delete "$INSTDIR\SoundFlow.exe"
    Delete "$INSTDIR\flutter_windows.dll"
    Delete "$INSTDIR\soundflow_lib.dll"
    Delete "$INSTDIR\READ-ME.txt"
    Delete "$INSTDIR\soundflow-install.json"
    Delete "$INSTDIR\Uninstall.exe"
    RMDir /r "$INSTDIR\resources"
    RMDir /r "$INSTDIR\media-provider"
    RMDir /r "$INSTDIR\licenses"
    RMDir /r "$INSTDIR\data"
    RMDir "$INSTDIR"
    ; A superseded/custom installation must not unregister a different installation.
    ReadRegStr $0 HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\SoundFlow" "InstallLocation"
    StrCmp $0 "$INSTDIR" 0 done
    Delete "$DESKTOP\SoundFlow.lnk"
    Delete "$SMPROGRAMS\SoundFlow\SoundFlow.lnk"
    RMDir "$SMPROGRAMS\SoundFlow"
    DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\SoundFlow"
    done:
SectionEnd
