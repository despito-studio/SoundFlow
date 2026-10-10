# Native Windows build

SoundFlow 3.2 uses Rust/Slint and Windows native audio. No browser engine is built or launched. The normal renderer uses OpenGL in the application process; the software renderer can be selected with `SLINT_BACKEND=winit-software` on unsupported graphics hardware.

## Toolchain

- Rust 1.92 or newer, x86_64-pc-windows-msvc.
- Visual Studio Build Tools with the C++ desktop compiler.
- Windows SDK. `scripts/setup-windows-sdk.ps1` downloads pinned official Microsoft NuGet packages, verifies their SHA-256 hashes and extracts them inside ignored `output/toolchain`.
- Node.js for building the media adapter; it is bundled for end users.
- NSIS for the optional installer.

`scripts/build-windows.ps1` scopes compiler/SDK variables to its process and defaults to one build job on machines with limited RAM. The MSVC release uses a static C runtime.

```powershell
npm.cmd ci --prefix media-provider
node scripts/build-provider.mjs
.\scripts\build-windows.ps1 -Action Build
.\scripts\build-windows.ps1 -Action Test
npm.cmd test --prefix media-provider
.\scripts\package-windows.ps1
```

`-SkipInstaller` or `-SkipZip` skip the corresponding package. Build outputs live under `target`, `output` and `dist`, separate from source.

## Verification

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-windows.ps1 -Executable .\dist\windows-x64\SoundFlow.exe
```

Run verification in Windows PowerShell 5.1 so the Windows UI Automation assemblies are available. Verification uses a temporary library. It exercises the packaged music worker, local file approvals, playlist/like writes, the Plus gate and full native WAV decoding. It renders the actual Slint UI at several sizes, then checks ordinary startup, window/EXE icons, accessibility, child processes and duplicate instance activation. Results are written under `output/native`.

YouTube download tests must consume the complete AAC file. A successful HTTP prefix alone does not establish a valid download. SoundCloud playback selects a directly supported MP3/AAC stream. Spotify uses metadata and user-selected counterparts.

## Application identity

The EXE, window, tray, shortcuts, installer and media runtime carry SoundFlow artwork. A Windows mutex prevents multiple native windows; repeated launches activate the existing one. The bundled media runtime is a single child attached to a Windows Job object, so closing the application also closes the worker. Windows Task Manager controls its own visual grouping; the application does not launch renderer or GPU child processes.

## Data

Roaming user data stays in `%APPDATA%\app.soundflow.desktop`. The uninstaller removes installed program files and shortcuts, leaving user libraries and downloaded music intact. Local library files are separate per account and API tokens use the Windows credential vault.
