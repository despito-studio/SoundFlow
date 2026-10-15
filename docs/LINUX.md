# Native Linux build

SoundFlow 3.2 uses Rust/Slint and native Linux audio (PipeWire, PulseAudio or ALSA via Rodio). No browser engine or Electron/WebView is built or launched. The normal renderer uses OpenGL/FemtoVG via Wayland or X11; software rendering can be selected with `SLINT_BACKEND=winit-software` if needed.

## Toolchain

- Rust stable (1.92 or newer), `x86_64-unknown-linux-gnu`.
- GCC / Clang C compiler and `pkg-config`.
- ALSA, Fontconfig, and D-Bus development headers (`alsa-lib-devel`, `fontconfig-devel`, `dbus-devel` on Fedora/RHEL; `libasound2-dev`, `libfontconfig1-dev`, `libdbus-1-dev` on Debian/Ubuntu).
- Node.js for bundling the media provider (`worker.cjs`); a pinned official Node.js Linux x64 binary is bundled for end users in `resources/runtime/soundflow-node`.

## Build & Package

```bash
# 1. Install media provider dependencies and build the bundle
(cd media-provider && npm ci)
node scripts/build-provider.mjs

# 2. Build and test SoundFlow
./scripts/build-linux.sh Build
./scripts/build-linux.sh Test
(cd media-provider && npm test)

# 3. Package the standalone distribution
./scripts/package-linux.sh
```

Build outputs live under `target`, `output` and `dist`, separate from source code.

## Verification

```bash
./scripts/verify-linux.sh
```

Verification uses a temporary sandbox. It exercises the packaged JavaScript music worker, local file approvals, playlist/like writes, the Plus gate, and native WAV decoding. It renders the Slint UI and verifies playback, window sizing, and child processes. Results are written to `output/native/native-report.json`.

## Application identity & Single Instance

On Linux, SoundFlow registers a Unix domain socket (`native-instance.sock`) in its data directory. Launching a second instance detects the active process, pings it to bring the existing window to the front, and exits cleanly. The bundled media provider runs as an isolated child worker and exits cleanly upon application shutdown.

## Packaging & Installers

Inside `dist/`:
- **`SoundFlow-3.2.0-Linux-Installer.run`**: Universal graphical / self-extracting installer with a Zenity GUI setup wizard, directory selection, desktop/menu integration, and uninstaller creation.
- **`soundflow-3.2.0-1.x86_64.rpm`**: Native RPM package for Fedora/RHEL/openSUSE (`sudo dnf install dist/soundflow-3.2.0-1.x86_64.rpm`).
- **`SoundFlow-3.2.0-linux-x64.tar.gz`**: Portable release archive.
- **`linux-x64/`**: Staged application directory:
  - `./soundflow.sh` or `./soundflow`: Run directly in-place.
  - `./install.sh`: Scripted desktop and menu installer.

## Data

User data and libraries stay in `~/.local/share/app.soundflow.desktop` (conforming to XDG standards). Local library files are separated per account, and secret tokens utilize the FreeDesktop Secret Service via DBus.
