# SoundFlow Flutter

Windows frontend for the Rust audio core. Build from the repository root:

```powershell
./scripts/build-flutter-windows.ps1
./scripts/package-windows.ps1
```

The development bundle is `output/flutter-preview`; the installer and managed
portable ZIP are written to `dist/release/<version>`. Keep each bundle intact.
See [Windows setup](../docs/WINDOWS.md) and [architecture](../docs/ARCHITECTURE.md).

For contribution guidelines, see [CONTRIBUTING.md](../CONTRIBUTING.md).
