param(
    [Parameter(Mandatory)][string]$Payload,
    [Parameter(Mandatory)][string]$Version,
    [string]$TermsUrl,
    [switch]$RequireTermsUrl,
    [string]$OutputDirectory
)
$ErrorActionPreference = 'Stop'
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
. (Join-Path $PSScriptRoot 'build-windows.ps1') -Action Environment
$dist = if ($OutputDirectory) { [IO.Path]::GetFullPath($OutputDirectory) } else { Join-Path $projectRoot 'dist' }
$build = Join-Path $projectRoot 'output/installer-build'
New-Item -ItemType Directory -Path $build, $dist -Force | Out-Null
$notices = @('ANIMATEICONS-LICENSE.txt','ANIMATEICONS-THIRD-PARTY-NOTICES.txt','CONCENTRIC-LICENSE.txt','CONCENTRIC-NOTICE.txt') | ForEach-Object {
    Get-Content -LiteralPath (Join-Path $projectRoot "resources/licenses/$_") -Raw
}
$notices += Get-Content -LiteralPath (Join-Path $projectRoot 'resources/licenses/ONEST-OFL.txt') -Raw
$noticeFile = Join-Path $build 'installer-notices.txt'
[IO.File]::WriteAllText($noticeFile,($notices -join "`r`n`r`n"),[Text.UTF8Encoding]::new($false))
$iconNames = @('folder-open','download','refresh-cw','x','arrow-left','check','circle-check')
$icons = Join-Path $projectRoot 'output/installer-icons'
$catalog = Join-Path $projectRoot 'flutter_app/packages/animate_icons/assets/catalog.json'
$exporter = Join-Path $projectRoot 'flutter_app/tool/export_installer_icons_test.dart'
$flutter = Join-Path $projectRoot 'output/toolchain/flutter/bin/flutter.bat'
if (-not (Test-Path -LiteralPath $flutter)) { $flutter = (Get-Command flutter.bat -ErrorAction Stop).Source }
$designInputs = @('concentric.dart','design.dart','installer_design.dart','anicry_motion.dart') | ForEach-Object { Join-Path $projectRoot "flutter_app/lib/$_" }
$designInputs += Join-Path $projectRoot 'flutter_app/packages/animate_icons/lib/animate_icons.dart'
$iconInputsChanged = @($catalog,$exporter) + $designInputs | Where-Object {
    (Get-Item -LiteralPath $_).LastWriteTimeUtc -gt (Get-Item -LiteralPath (Join-Path $icons 'download.png') -ErrorAction SilentlyContinue).LastWriteTimeUtc
}
if ($iconInputsChanged -or -not (Test-Path -LiteralPath (Join-Path $icons 'installer_vectors.h')) -or -not (Test-Path -LiteralPath (Join-Path $icons 'installer_design.h')) -or ($iconNames | Where-Object { -not (Test-Path -LiteralPath (Join-Path $icons "$_.png")) })) {
    Push-Location (Join-Path $projectRoot 'flutter_app')
    try {
        & $flutter test --no-pub tool/export_installer_icons_test.dart
        if ($LASTEXITCODE -ne 0) { throw 'Shared AnimateIcons export failed.' }
    } finally { Pop-Location }
}
if (-not $TermsUrl) { $TermsUrl = (Get-Content -LiteralPath (Join-Path $projectRoot 'resources/installer/config.json') -Raw | ConvertFrom-Json).termsUrl }
if ($TermsUrl) {
    $terms = [Uri]$TermsUrl
    if (-not $terms.IsAbsoluteUri -or $terms.Scheme -ne 'https' -or $terms.UserInfo -or $TermsUrl -match '["\r\n]') { throw 'TermsUrl must be a public HTTPS website URL.' }
} elseif ($RequireTermsUrl) { throw 'Set termsUrl in resources/installer/config.json before publishing an installer.' }
else { Write-Host 'Terms website is not configured; the agreement step will be skipped.' }
$payload = (Resolve-Path -LiteralPath $Payload).Path
$payloadHash = (Get-FileHash -LiteralPath $payload -Algorithm SHA256).Hash.ToLowerInvariant()
$payloadSize = (Get-Item -LiteralPath $payload).Length
function Cpp-String([string]$Text) { return $Text.Replace('\', '\\').Replace('"', '\"') }
$config = @"
#pragma once
#define SF_VERSION L"$(Cpp-String $Version)"
#define SF_VERSION_UTF8 "$(Cpp-String $Version)"
#define SF_TERMS_URL L"$(Cpp-String $TermsUrl)"
#define SF_PACKAGE_SHA256 "$payloadHash"
#define SF_PACKAGE_SIZE ${payloadSize}ULL
"@
Set-Content -LiteralPath (Join-Path $build 'installer_config.h') -Value $config -Encoding utf8
$versionNumbers = ($Version.Split('.') + '0') -join ','
$identity = @"
#include <windows.h>
101 ICON "$(Cpp-String (Join-Path $projectRoot 'resources/icon.ico'))"
502 RCDATA "$(Cpp-String (Join-Path $projectRoot 'resources/icon.png'))"
503 RCDATA "$(Cpp-String $noticeFile)"
504 RCDATA "$(Cpp-String (Join-Path $projectRoot 'resources/fonts/Onest-Regular.ttf'))"
505 RCDATA "$(Cpp-String (Join-Path $projectRoot 'resources/fonts/Onest-SemiBold.ttf'))"
1 RT_MANIFEST "$(Cpp-String (Join-Path $projectRoot 'resources/installer/installer.manifest'))"
1 VERSIONINFO
FILEVERSION $versionNumbers
PRODUCTVERSION $versionNumbers
FILEOS VOS_NT_WINDOWS32
FILETYPE VFT_APP
BEGIN
 BLOCK "StringFileInfo"
 BEGIN
  BLOCK "040904b0"
  BEGIN
   VALUE "ProductName", "SoundFlow"
   VALUE "FileDescription", "SoundFlow Installer"
   VALUE "ProductVersion", "$Version"
   VALUE "FileVersion", "$Version"
   VALUE "CompanyName", "SoundFlow"
  END
 END
 BLOCK "VarFileInfo"
 BEGIN
  VALUE "Translation", 0x409, 1200
 END
END
"@
for ($index = 0; $index -lt $iconNames.Count; $index++) {
    $identity += "`r`n$($index + 601) RCDATA `"$(Cpp-String (Join-Path $icons ($iconNames[$index] + '.png')))`"`r`n"
}
$compiler = (Get-Command cl.exe).Source
$resourceCompiler = (Get-Command rc.exe).Source
$source = Join-Path $projectRoot 'resources/installer/setup.cpp'
$object = Join-Path $build 'setup.obj'
& $compiler /nologo /std:c++17 /utf-8 /EHsc /O2 /MT /W4 /DUNICODE /D_UNICODE /D_WIN32_WINNT=0x0A00 "/I$build" "/I$icons" /c $source "/Fo$object"
if ($LASTEXITCODE -ne 0) { throw 'Native installer compilation failed.' }
foreach ($kind in @('Setup')) {
    $resource = $identity
    if ($kind -eq 'Setup') { $resource += "`r`n501 RCDATA `"$(Cpp-String $payload)`"`r`n" }
    $rc = Join-Path $build "$kind.rc"
    $res = Join-Path $build "$kind.res"
    Set-Content -LiteralPath $rc -Value $resource -Encoding utf8
    & $resourceCompiler /nologo "/fo$res" $rc
    if ($LASTEXITCODE -ne 0) { throw "Installer resources failed: $kind" }
    $exe = Join-Path $dist "SoundFlow-$Version-$kind.exe"
    & link.exe /NOLOGO /SUBSYSTEM:WINDOWS /MACHINE:X64 /DYNAMICBASE /NXCOMPAT /MANIFEST:NO "/OUT:$exe" $object $res kernel32.lib user32.lib gdi32.lib gdiplus.lib d2d1.lib dwrite.lib dwmapi.lib shell32.lib ole32.lib uuid.lib advapi32.lib comdlg32.lib comctl32.lib winhttp.lib bcrypt.lib version.lib
    if ($LASTEXITCODE -ne 0) { throw "Installer link failed: $kind" }
}
$setup = Join-Path $dist "SoundFlow-$Version-Setup.exe"
Write-Host "Installer: $setup"
