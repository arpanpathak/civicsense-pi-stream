#!/usr/bin/env bash
# ============================================================
# CivicSense Pi Stream — WiFi hotspot (NetworkManager / nmcli)
#
# For Raspberry Pi OS (desktop OR Lite since Bookworm), which ships
# NetworkManager by default. One command brings the access point up.
#
#   Usage:  sudo ./scripts/hotspot-nmcli.sh
#
# After it runs:
#   * the Pi broadcasts SSID  CivicSense  (password: civicsense123)
#   * the Pi's AP IP is 10.42.0.1
#   * phones that join receive MJPEG datagrams on UDP port 9000
#     via broadcast — the app needs no IP configuration
#
# Tune with env vars: PI_STREAM_SSID, PI_STREAM_PASSWORD,
# PI_STREAM_WIFI_IFACE.
# ============================================================
set -euo pipefail

SSID="${PI_STREAM_SSID:-CivicSense}"
PASSWORD="${PI_STREAM_PASSWORD:-civicsense123}"
IFACE="${PI_STREAM_WIFI_IFACE:-wlan0}"

if ! command -v nmcli >/dev/null 2>&1; then
    echo "error: nmcli not found — install network-manager or use hotspot-hostapd.sh" >&2
    exit 1
fi

# WPA2 requires 8..63 characters.
if [ "${#PASSWORD}" -lt 8 ]; then
    echo "error: password must be at least 8 characters" >&2
    exit 1
fi

echo "==> bringing up hotspot: $SSID on $IFACE"

# Idempotency: drop a previous connection with the same name first.
nmcli connection delete "$SSID" 2>/dev/null || true

nmcli device wifi hotspot ifname "$IFACE" ssid "$SSID" password "$PASSWORD"

echo "==> hotspot up"
ip -4 -o addr show "$IFACE" | awk '{print "   AP IP: " $4}'
echo "==> start the streamer:  sudo systemctl start pi_stream   (or run pi_stream directly)"
