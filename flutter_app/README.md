# SoundFlow Flutter

Windows frontend for the Rust audio core. Build from the repository root:

```powershell
./scripts/build-flutter-windows.ps1
./scripts/package-windows.ps1
```

The runnable bundle is `output/flutter-preview`; distributable ZIP and Setup.exe
are written to `dist`. Keep the executable, DLLs, resources and data together.
See [Windows setup](../docs/WINDOWS.md) and [architecture](../docs/ARCHITECTURE.md).
