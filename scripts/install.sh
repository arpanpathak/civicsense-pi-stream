#!/usr/bin/env bash
# ============================================================
# CivicSense Pi Stream — install binaries + systemd service
#
#   Usage:  sudo ./scripts/install.sh [path-to-binaries]
#
# [path-to-binaries] defaults to
#   ./bin/armv7-unknown-linux-gnueabihf
# which is what `make build` produces for the 32-bit Raspbian Pi.
# Pass the arch directory explicitly if you built another target.
#
# Installs to /usr/local/bin and enables the all-in-one pi_stream
# service (HTTP :8000 + UDP :9000) via systemd.
# ============================================================
set -euo pipefail

if [ "$(id -u)" -ne 0 ]; then
    echo "error: run with sudo" >&2
    exit 1
fi

# Resolve the binary directory (default: the 32-bit Raspbian build).
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(dirname "$SCRIPT_DIR")"
BIN_DIR="${1:-$REPO_ROOT/bin/armv7-unknown-linux-gnueabihf}"

if [ ! -d "$BIN_DIR" ]; then
    echo "error: no binaries in $BIN_DIR — run 'make build' first" >&2
    exit 1
fi

echo "==> installing binaries from $BIN_DIR"
install -m 0755 "$BIN_DIR"/pi_stream* /usr/local/bin/

echo "==> installing systemd unit"
install -m 0644 "$REPO_ROOT/deploy/pi_stream.service" /etc/systemd/system/pi_stream.service
systemctl daemon-reload
systemctl enable pi_stream

echo "==> done"
echo "    start now:        sudo systemctl start pi_stream"
echo "    enable hotspot:   sudo ./scripts/hotspot-nmcli.sh   (or hotspot-hostapd.sh)"
