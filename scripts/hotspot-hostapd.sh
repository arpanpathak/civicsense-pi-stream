#!/usr/bin/env bash
# ============================================================
# CivicSense Pi Stream — WiFi hotspot (hostapd + dnsmasq)
#
# The classic approach for headless/Lite images or setups where
# NetworkManager is disabled. Configures:
#   * wlan0     -> static 192.168.4.1/24 (via dhcpcd when present)
#   * hostapd   -> WPA2 access point (CivicSense)
#   * dnsmasq   -> DHCP for phones (192.168.4.2 - 192.168.4.100)
#
#   Usage:  sudo ./scripts/hotspot-hostapd.sh
#
# NOTE: Raspberry Pi OS Bookworm+ uses NetworkManager by default.
# If you run this script there, disable NM first (or just use
# hotspot-nmcli.sh instead):
#   sudo systemctl disable --now NetworkManager
#
# Tune with env vars: PI_STREAM_SSID, PI_STREAM_PASSWORD,
# PI_STREAM_WIFI_IFACE.
# ============================================================
set -euo pipefail

SSID="${PI_STREAM_SSID:-CivicSense}"
PASSWORD="${PI_STREAM_PASSWORD:-civicsense123}"
IFACE="${PI_STREAM_WIFI_IFACE:-wlan0}"
AP_IP="192.168.4.1"

if [ "$(id -u)" -ne 0 ]; then
    echo "error: run with sudo" >&2
    exit 1
fi

if [ "${#PASSWORD}" -lt 8 ]; then
    echo "error: password must be at least 8 characters" >&2
    exit 1
fi

echo "==> installing hostapd + dnsmasq"
apt-get update
apt-get install -y hostapd dnsmasq

echo "==> stopping stock services until configured"
systemctl stop hostapd dnsmasq || true

# --- static IP on wlan0 -------------------------------------------------
if [ -f /etc/dhcpcd.conf ]; then
    # Classic dhcpcd (Pi OS Bullseye and earlier, or NM disabled).
    cat >> /etc/dhcpcd.conf <<EOF

# CivicSense hotspot
interface $IFACE
static ip_address=$AP_IP/24
nohook wpa_supplicant
EOF
    systemctl restart dhcpcd
else
    echo "warning: no /etc/dhcpcd.conf found — set a static IP on $IFACE yourself" >&2
fi

# --- hostapd ------------------------------------------------------------
cat > /etc/hostapd/hostapd.conf <<EOF
interface=$IFACE
driver=nl80211
ssid=$SSID
hw_mode=g
channel=6
wmm_enabled=1
macaddr_acl=0
auth_algs=1
ignore_broadcast_ssid=0
wpa=2
wpa_passphrase=$PASSWORD
wpa_key_mgmt=WPA-PSK
rsn_pairwise=CCMP
EOF

# Point the init script at our config file.
sed -i 's|^#\?DAEMON_CONF=.*|DAEMON_CONF="/etc/hostapd/hostapd.conf"|' /etc/default/hostapd

# --- dnsmasq ------------------------------------------------------------
cat > /etc/dnsmasq.conf <<EOF
interface=$IFACE
dhcp-range=192.168.4.2,192.168.4.100,255.255.255.0,24h
EOF

echo "==> enabling services"
systemctl unmask hostapd
systemctl enable hostapd dnsmasq
systemctl restart hostapd dnsmasq

echo "==> hotspot up: $SSID on $IFACE, AP IP $AP_IP"
echo "==> phones receive UDP MJPEG on port 9000 (broadcast)"
