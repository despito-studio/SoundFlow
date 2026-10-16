#!/usr/bin/env bash
set -euo pipefail

export PATH="$HOME/.cargo/bin:$HOME/.local/bin:$PATH"

DESTINATION="${1:-}"
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="3.4.0"
RELEASE_EXE="$PROJECT_ROOT/target/release/soundflow"

if [ -z "$DESTINATION" ]; then
    DESTINATION="$PROJECT_ROOT/dist/linux-x64"
fi

DISTRIBUTION_ROOT="$PROJECT_ROOT/dist"

echo "Checking required dependencies..."
for req in "$RELEASE_EXE" "$PROJECT_ROOT/resources/runtime/soundflow-node" "$PROJECT_ROOT/media-provider/dist/worker.cjs"; do
    if [ ! -f "$req" ]; then
        echo "Missing release dependency: $req"
        echo "Run ./scripts/build-linux.sh Build first."
        exit 1
    fi
done

echo "Preparing destination directory: $DESTINATION"
rm -rf "$DESTINATION"
mkdir -p "$DESTINATION"

# 1. Copy executable and wrapper
cp "$RELEASE_EXE" "$DESTINATION/soundflow"
chmod +x "$DESTINATION/soundflow"

cat > "$DESTINATION/soundflow.sh" << 'EOF'
#!/usr/bin/env bash
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec "$SCRIPT_DIR/soundflow" "$@"
EOF
chmod +x "$DESTINATION/soundflow.sh"

# 2. Copy resources
mkdir -p "$DESTINATION/resources"
cp -r "$PROJECT_ROOT/resources/runtime" "$DESTINATION/resources/"
cp -r "$PROJECT_ROOT/resources/covers" "$DESTINATION/resources/"
cp "$PROJECT_ROOT/resources/curated.json" "$DESTINATION/resources/"
cp "$PROJECT_ROOT/resources/icon.png" "$DESTINATION/resources/"
cp "$PROJECT_ROOT/resources/icon.svg" "$DESTINATION/resources/"
if [ -f "$PROJECT_ROOT/resources/icon.ico" ]; then
    cp "$PROJECT_ROOT/resources/icon.ico" "$DESTINATION/resources/"
fi
chmod +x "$DESTINATION/resources/runtime/soundflow-node"

# 3. Copy media-provider
mkdir -p "$DESTINATION/media-provider/dist"
cp "$PROJECT_ROOT/media-provider/dist/worker.cjs" "$DESTINATION/media-provider/dist/"

# 4. Copy licenses
LICENSES_DIR="$DESTINATION/licenses"
mkdir -p "$LICENSES_DIR"
cp "$PROJECT_ROOT/resources/runtime/NODE-LICENSE.txt" "$LICENSES_DIR/"
cp "$PROJECT_ROOT/resources/ARTWORK.md" "$LICENSES_DIR/"
cp "$PROJECT_ROOT/resources/licenses/SLINT-LICENSE.md" "$LICENSES_DIR/"
if [ -f "$PROJECT_ROOT/media-provider/dist/THIRD-PARTY-LICENSES.txt" ]; then
    cp "$PROJECT_ROOT/media-provider/dist/THIRD-PARTY-LICENSES.txt" "$LICENSES_DIR/MEDIA-LICENSES.txt"
fi
node "$PROJECT_ROOT/scripts/collect-licenses.mjs" "$LICENSES_DIR/RUST-LICENSES.txt"
if [ -d "$PROJECT_ROOT/media-provider/licenses" ]; then
    cp -r "$PROJECT_ROOT/media-provider/licenses" "$LICENSES_DIR/media"
fi

# 5. Desktop entry
cat > "$DESTINATION/soundflow.desktop" << 'EOF'
[Desktop Entry]
Name=SoundFlow
GenericName=Music Player
Comment=SoundFlow Native Music Player
Exec=soundflow %U
Icon=soundflow
Terminal=false
Type=Application
Categories=AudioVideo;Audio;Player;Music;
MimeType=audio/mpeg;audio/flac;audio/ogg;audio/wav;audio/x-m4a;audio/mp4;
StartupWMClass=soundflow
EOF

# 6. User install script
cat > "$DESTINATION/install.sh" << 'EOF'
#!/usr/bin/env bash
set -euo pipefail

APP_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_DIR="$HOME/.local/bin"
DESKTOP_DIR="$HOME/.local/share/applications"
ICON_DIR="$HOME/.local/share/icons/hicolor/512x512/apps"

mkdir -p "$BIN_DIR" "$DESKTOP_DIR" "$ICON_DIR"

# Symlink binary to ~/.local/bin
ln -sf "$APP_DIR/soundflow" "$BIN_DIR/soundflow"

# Install icon
cp "$APP_DIR/resources/icon.png" "$ICON_DIR/soundflow.png"

# Install desktop entry with quoted absolute path and working directory
cat > "$DESKTOP_DIR/soundflow.desktop" << DESKTOPEOF
[Desktop Entry]
Name=SoundFlow
GenericName=Music Player
Comment=SoundFlow Native Music Player
Exec="$APP_DIR/soundflow" %U
Path=$APP_DIR
Icon=$ICON_DIR/soundflow.png
Terminal=false
Type=Application
Categories=AudioVideo;Audio;Player;Music;
MimeType=audio/mpeg;audio/flac;audio/ogg;audio/wav;audio/x-m4a;audio/mp4;
StartupWMClass=soundflow
DESKTOPEOF

chmod +x "$DESKTOP_DIR/soundflow.desktop"

# Also install shortcut on the user's Desktop for 2-click launch
USER_DESKTOP="$(xdg-user-dir DESKTOP 2>/dev/null || echo "$HOME/Рабочий стол")"
if [ ! -d "$USER_DESKTOP" ] && [ -d "$HOME/Рабочий стол" ]; then
    USER_DESKTOP="$HOME/Рабочий стол"
elif [ ! -d "$USER_DESKTOP" ] && [ -d "$HOME/Desktop" ]; then
    USER_DESKTOP="$HOME/Desktop"
fi

if [ -d "$USER_DESKTOP" ]; then
    cp "$DESKTOP_DIR/soundflow.desktop" "$USER_DESKTOP/SoundFlow.desktop"
    chmod +x "$USER_DESKTOP/SoundFlow.desktop"
    gio set "$USER_DESKTOP/SoundFlow.desktop" metadata::trusted yes 2>/dev/null || true
fi

if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$DESKTOP_DIR" || true
fi

echo "SoundFlow successfully installed into your desktop applications and on your Desktop!"
echo "You can now launch SoundFlow by double-clicking the icon on your Desktop or from your application menu."
EOF
chmod +x "$DESTINATION/install.sh"

# 7. README notes
cat > "$DESTINATION/README.txt" << 'EOF'
SoundFlow 3.2 — Native Linux x64

How to run:
  ./soundflow.sh
  or
  ./soundflow

To install into your desktop application menu:
  ./install.sh

No Python, global Node.js installation, browser, or Electron/WebView is required.
The interface and audio output run natively in Rust with Slint and Rodio/PipeWire/PulseAudio.
Online music starts an isolated bundled JavaScript media worker that exits with SoundFlow.

Library and settings are kept in: ~/.local/share/app.soundflow.desktop
EOF

# 8. Archive
echo "Creating release tarball: $DISTRIBUTION_ROOT/SoundFlow-$VERSION-linux-x64.tar.gz"
tar -czf "$DISTRIBUTION_ROOT/SoundFlow-$VERSION-linux-x64.tar.gz" -C "$DISTRIBUTION_ROOT" linux-x64

# 9. Standalone Self-Extracting Installer
if [ -f "$PROJECT_ROOT/scripts/create-installer.sh" ]; then
    "$PROJECT_ROOT/scripts/create-installer.sh"
fi

echo "Package complete: $DESTINATION"
echo "Tarball ready: $DISTRIBUTION_ROOT/SoundFlow-$VERSION-linux-x64.tar.gz"
