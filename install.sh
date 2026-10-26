#!/bin/sh
# Builds and installs Battery Health Center system-wide.
#   ./install.sh             build and install
#   ./install.sh --uninstall remove everything except your history and settings
set -eu
cd "$(dirname "$0")"

PREFIX=/usr/local
LIBEXEC="$PREFIX/libexec/battery-health-center"
UNIT=battery-health-center-restore.service
POLICY=/usr/share/polkit-1/actions/io.github.BatteryHealthCenter.policy
APP_ID=io.github.BatteryHealthCenter

if [ "${1:-}" = "--uninstall" ]; then
    sudo systemctl disable "$UNIT" 2>/dev/null || true
    sudo rm -f "/etc/systemd/system/$UNIT" "$POLICY" \
        "$PREFIX/bin/battery-health-center" "$LIBEXEC/bhc-helper" \
        "$PREFIX/share/applications/$APP_ID.desktop" \
        "$PREFIX/share/icons/hicolor/scalable/apps/$APP_ID.svg"
    sudo rmdir "$LIBEXEC" 2>/dev/null || true
    sudo systemctl daemon-reload
    # Refresh the caches the install built. A stale icon cache in /usr/local keeps
    # listing the removed icon, and since /usr/local is searched before /usr, it
    # hides the icon of any other copy of the app (such as the .deb): blank icon.
    if [ -d "$PREFIX/share/icons/hicolor" ]; then
        sudo gtk-update-icon-cache -q -t -f "$PREFIX/share/icons/hicolor" 2>/dev/null || true
    fi
    sudo update-desktop-database -q "$PREFIX/share/applications" 2>/dev/null || true
    echo "Uninstalled. Charge limit config (if any) left at /etc/battery-health-center.conf"
    exit 0
fi

cargo build --release

sudo install -Dm755 target/release/battery-health-center "$PREFIX/bin/battery-health-center"
sudo install -Dm755 target/release/bhc-helper "$LIBEXEC/bhc-helper"
sudo install -Dm644 "data/$APP_ID.policy" "$POLICY"
sudo install -Dm644 "data/$APP_ID.desktop" "$PREFIX/share/applications/$APP_ID.desktop"
sudo install -Dm644 "data/icons/hicolor/scalable/apps/$APP_ID.svg" \
    "$PREFIX/share/icons/hicolor/scalable/apps/$APP_ID.svg"
sudo install -Dm644 "data/$UNIT" "/etc/systemd/system/$UNIT"
sudo systemctl daemon-reload
sudo systemctl enable "$UNIT"
sudo gtk-update-icon-cache -q -t "$PREFIX/share/icons/hicolor" 2>/dev/null || true
sudo update-desktop-database -q "$PREFIX/share/applications" 2>/dev/null || true

echo "Installed. Launch \"Battery Health Center\" from Activities."
