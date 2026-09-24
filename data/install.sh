#!/usr/bin/env bash
# Install Reclix user-level files after `cargo build --release`.
# Run from the workspace root: bash data/install.sh
set -euo pipefail

CARGO_BIN="${HOME}/.cargo/bin"
SYSTEMD_USER="${HOME}/.config/systemd/user"
DBUS_SERVICES="${HOME}/.local/share/dbus-1/services"
DESKTOP_DIR="${HOME}/.local/share/applications"

echo "==> Installing binaries to ${CARGO_BIN}"
# `cargo install --path` handles this; this script assumes they're already there.

echo "==> Installing systemd user unit"
mkdir -p "${SYSTEMD_USER}"
cp data/reclix.service "${SYSTEMD_USER}/reclix.service"
systemctl --user daemon-reload
systemctl --user enable reclix.service
echo "    To start now: systemctl --user start reclix.service"

echo "==> Installing D-Bus service activation file"
mkdir -p "${DBUS_SERVICES}"
# Substitute the actual binary path.
sed "s|/usr/local/bin/reclix-service|${CARGO_BIN}/reclix-service|g" \
    data/org.reclix.Service1.service \
    > "${DBUS_SERVICES}/org.reclix.Service1.service"

echo "==> Installing .desktop file"
mkdir -p "${DESKTOP_DIR}"
cp data/reclix.desktop "${DESKTOP_DIR}/reclix.desktop"
update-desktop-database "${DESKTOP_DIR}" 2>/dev/null || true

echo "==> Done. Start with: systemctl --user start reclix.service"
echo "    Then try:          reclix status"
