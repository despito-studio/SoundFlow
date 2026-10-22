# Windows build

The current frontend is Flutter 3.47.7 / Dart 3.13.5. The Rust audio core builds as soundflow_lib.dll, without a separate Rust UI executable.

The checkout-local build uses:

- Flutter at output/toolchain/flutter.
- Rust MSVC 1.92 or newer.
- Microsoft C++ toolset and Windows SDK prepared by setup-msvc.ps1 and setup-windows-sdk.ps1.
- CMake and Ninja under output/toolchain/python-build.

Run scripts/build-flutter-windows.ps1. The script builds the Rust library, resolves Flutter packages, generates Flutter Windows configuration, then builds the standard Windows runner using Ninja and the local Microsoft compiler. The Flutter tool does not discover this portable toolset as a registered Visual Studio installation; its generated configuration is consumed by the native CMake build.

The output is output/flutter-preview/SoundFlow.exe with its required DLLs and data directories. Keep the whole folder together. -SkipCore reuses the current Rust DLL while iterating on the frontend.

The application has a native Windows caption and resizable main window. Initial bounds fit the monitor work area. Rust handles the single-instance mutex, tray, media keys and Ctrl+Alt+M shortcuts. The Windows runner owns the Island window and forwards its commands to the main Flutter engine.

User data remains in %APPDATA%\app.soundflow.desktop. --data-dir selects an isolated profile; --resources selects a bundle root; --offline prevents cloud work. --export-design <png> exports one frame directly from the Flutter render tree for design review. It does not inject desktop input.

The former UI verifier and previous sources are archived under ignored output/previous-slint-source-20261021. Its historical reports do not establish Flutter behavior. No automated UI tests were added or run for this frontend migration.

Flutter package notices are generated into data/flutter_assets/NOTICES.Z and accessible from About. Existing icon and media-runtime notices stay under resources/licenses. Segoe UI is read from Windows and is not redistributed as an app asset.
