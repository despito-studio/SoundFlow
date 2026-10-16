#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="3.4.0"
DIST_DIR="$PROJECT_ROOT/dist"
STAGE_DIR="$DIST_DIR/linux-x64"
INSTALLER_BIN="$DIST_DIR/SoundFlow-$VERSION-Linux-Installer.run"

if [ ! -d "$STAGE_DIR" ]; then
    echo "Staged files not found in $STAGE_DIR. Run ./scripts/package-linux.sh first."
    exit 1
fi

PAYLOAD_TAR="/tmp/soundflow-payload-$$.tar.gz"
echo "Creating payload tarball..."
tar -czf "$PAYLOAD_TAR" -C "$DIST_DIR" linux-x64

echo "Assembling self-extracting installer: $INSTALLER_BIN"

cat > "$INSTALLER_BIN" << 'INSTALLER_HEADER_EOF'
#!/usr/bin/env bash
# SoundFlow Linux Native Installer
set -euo pipefail

APP_NAME="SoundFlow"
APP_VERSION="3.4.0"
DEFAULT_INSTALL_DIR="$HOME/.local/share/soundflow"

# Detect desktop / GUI mode
HAS_GUI=false
if [ -n "${WAYLAND_DISPLAY:-}" ] || [ -n "${DISPLAY:-}" ]; then
    if command -v zenity >/dev/null 2>&1; then
        HAS_GUI=true
    fi
fi

# 1. Welcome Screen
if [ "$HAS_GUI" = true ]; then
    if ! zenity --question \
        --title="Майстер встановлення SoundFlow $APP_VERSION" \
        --width=450 \
        --text="<b>Вас вітає майстер встановлення SoundFlow $APP_VERSION!</b>\n\nSoundFlow — це нативний музичний плеєр на Rust з підтримкою онлайн-потоків та високою якістю звуку.\n\nБажаєте встановити SoundFlow у вашу систему?" \
        --ok-label="Встановити" \
        --cancel-label="Скасувати"; then
        exit 0
    fi
else
    echo "================================================="
    echo "  Майстер встановлення SoundFlow $APP_VERSION"
    echo "================================================="
    echo "SoundFlow — нативний музичний плеєр на Rust."
    echo ""
    read -rp "Продовжити встановлення? [Y/n]: " CONFIRM
    CONFIRM="${CONFIRM:-Y}"
    if [[ ! "$CONFIRM" =~ ^[YyДд]$ ]]; then
        echo "Встановлення скасовано."
        exit 0
    fi
fi

# 2. Target Directory Selection
INSTALL_DIR="$DEFAULT_INSTALL_DIR"
if [ "$HAS_GUI" = true ]; then
    CHOSEN_DIR=$(zenity --entry \
        --title="Папка встановлення" \
        --text="Оберіть папку для встановлення SoundFlow:" \
        --entry-text="$DEFAULT_INSTALL_DIR" || true)
    if [ -n "$CHOSEN_DIR" ]; then
        INSTALL_DIR="$CHOSEN_DIR"
    fi
fi

# Expand tilde if needed
INSTALL_DIR="${INSTALL_DIR/#\~/$HOME}"

# 3. Extraction with Progress
extract_payload() {
    local target="$1"
    mkdir -p "$target"
    local payload_line
    payload_line=$(grep -a -n '^__PAYLOAD_BEGINS__$' "$0" | cut -d: -f1)
    local tmp_extract
    tmp_extract=$(mktemp -d /tmp/soundflow-inst-XXXXXX)
    tail -n +$((payload_line + 1)) "$0" | tar -xz -C "$tmp_extract"
    cp -r "$tmp_extract/linux-x64/"* "$target/"
    rm -rf "$tmp_extract"
    chmod +x "$target/soundflow" "$target/soundflow.sh"
}

if [ "$HAS_GUI" = true ]; then
    (
        echo "20"; echo "# Підготовка каталогу встановлення..."; sleep 0.2
        echo "50"; echo "# Розпакування файлів SoundFlow..."; sleep 0.2
        extract_payload "$INSTALL_DIR"
        echo "80"; echo "# Створення системних ярликів та інтеграція..."; sleep 0.2
        echo "100"; echo "# Завершення встановлення..."; sleep 0.2
    ) | zenity --progress \
        --title="Встановлення SoundFlow" \
        --text="Розпакування файлів програми..." \
        --percentage=0 \
        --auto-close \
        --no-cancel
else
    echo "Розпакування файлів програми у $INSTALL_DIR..."
    extract_payload "$INSTALL_DIR"
fi

# 4. Desktop Integration
BIN_DIR="$HOME/.local/bin"
DESKTOP_DIR="$HOME/.local/share/applications"
ICON_DIR="$HOME/.local/share/icons/hicolor/512x512/apps"

mkdir -p "$BIN_DIR" "$DESKTOP_DIR" "$ICON_DIR"

ln -sf "$INSTALL_DIR/soundflow" "$BIN_DIR/soundflow"
cp "$INSTALL_DIR/resources/icon.png" "$ICON_DIR/soundflow.png"

cat > "$DESKTOP_DIR/soundflow.desktop" << DESKTOPEOF
[Desktop Entry]
Name=SoundFlow
GenericName=Music Player
Comment=SoundFlow Native Music Player
Exec="$INSTALL_DIR/soundflow" %U
Path=$INSTALL_DIR
Icon=$ICON_DIR/soundflow.png
Terminal=false
Type=Application
Categories=AudioVideo;Audio;Player;Music;
MimeType=audio/mpeg;audio/flac;audio/ogg;audio/wav;audio/x-m4a;audio/mp4;
StartupWMClass=soundflow
DESKTOPEOF
chmod +x "$DESKTOP_DIR/soundflow.desktop"

# User's Desktop shortcut
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

# 5. Create Uninstaller
cat > "$INSTALL_DIR/uninstall.sh" << UNINSTALLEOF
#!/usr/bin/env bash
set -euo pipefail

APP_DIR="$INSTALL_DIR"
BIN_LINK="$HOME/.local/bin/soundflow"
DESKTOP_FILE="$HOME/.local/share/applications/soundflow.desktop"
ICON_FILE="$HOME/.local/share/icons/hicolor/512x512/apps/soundflow.png"
DESKTOP_SHORTCUT="$USER_DESKTOP/SoundFlow.desktop"

HAS_GUI=false
if [ -n "\${WAYLAND_DISPLAY:-}" ] || [ -n "\${DISPLAY:-}" ]; then
    if command -v zenity >/dev/null 2>&1; then
        HAS_GUI=true
    fi
fi

if [ "\$HAS_GUI" = true ]; then
    if ! zenity --question \
        --title="Видалення SoundFlow" \
        --width=380 \
        --text="Ви дійсно бажаєте видалити SoundFlow з вашої системи?" \
        --ok-label="Видалити" \
        --cancel-label="Скасувати"; then
        exit 0
    fi
else
    read -rp "Видалити SoundFlow? [y/N]: " CONF
    if [[ ! "\$CONF" =~ ^[YyДд]$ ]]; then
        echo "Скасовано."
        exit 0
    fi
fi

rm -f "\$BIN_LINK" "\$DESKTOP_FILE" "\$ICON_FILE" "\$DESKTOP_SHORTCUT"
rm -rf "\$APP_DIR"

if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$HOME/.local/share/applications" || true
fi

if [ "\$HAS_GUI" = true ]; then
    zenity --info --title="SoundFlow" --text="SoundFlow успішно видалено з вашої системи."
else
    echo "SoundFlow успішно видалено."
fi
UNINSTALLEOF
chmod +x "$INSTALL_DIR/uninstall.sh"

if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$DESKTOP_DIR" || true
fi

# 6. Finish Screen & Launch prompt
if [ "$HAS_GUI" = true ]; then
    if zenity --question \
        --title="Встановлення завершено" \
        --width=450 \
        --text="<b>SoundFlow $APP_VERSION успішно встановлено!</b>\n\nПапка: $INSTALL_DIR\nЯрлики створено на Робочому столі та в меню програм.\n\nЗапустити SoundFlow зараз?" \
        --ok-label="Запустити" \
        --cancel-label="Закрити"; then
        nohup "$INSTALL_DIR/soundflow" >/dev/null 2>&1 &
    fi
else
    echo "================================================="
    echo "  SoundFlow $APP_VERSION успішно встановлено!"
    echo "================================================="
    echo "Шлях: $INSTALL_DIR"
    echo "Запуск: soundflow або $INSTALL_DIR/soundflow"
    echo "Видалення: $INSTALL_DIR/uninstall.sh"
    echo ""
    read -rp "Запустити SoundFlow зараз? [Y/n]: " LAUNCH
    LAUNCH="${LAUNCH:-Y}"
    if [[ "$LAUNCH" =~ ^[YyДд]$ ]]; then
        nohup "$INSTALL_DIR/soundflow" >/dev/null 2>&1 &
    fi
fi

exit 0

__PAYLOAD_BEGINS__
INSTALLER_HEADER_EOF

cat "$PAYLOAD_TAR" >> "$INSTALLER_BIN"
chmod +x "$INSTALLER_BIN"
rm -f "$PAYLOAD_TAR"

echo "Installer successfully created: $INSTALLER_BIN"
ls -lh "$INSTALLER_BIN"
