# 🎥 CivicSense Pi Stream: MJPEG Streaming Server in Rust

A lightweight, production-ready video streaming server for the **Raspberry Pi Zero 2 W** with an **Arducam IMX335** camera module. Built in Rust with no Python and no OpenCV, it shells out to `rpicam-vid` under the hood and ships **three binaries**:

| Binary | Transport | Use case |
|---|---|---|
| `pi_stream_http` | HTTP `multipart/x-mixed-replace` | browser / legacy clients (original behavior) |
| `pi_stream_udp` | raw MJPEG UDP datagrams | **phone app** streaming |
| `pi_stream` | HTTP **+** UDP | all-in-one **WiFi hotspot** mode |

Everything cross-compiles inside Docker for any Pi OS — 32-bit Raspbian, 64-bit Raspberry Pi OS, or your dev machine.

> *don't trust your vision if it's blurry,*
> *don't rush the yellow in a hurry,*
> *the math is proven, the call is true,*
> *better safe than sorry, let it clear, then pass through.*


[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.70+-orange.svg)](https://www.rust-lang.org)
[![Platform](https://img.shields.io/badge/Platform-Pi%20Zero%202%20W-A22846)](https://www.raspberrypi.com/)
[![Camera](https://img.shields.io/badge/Camera-Arducam%20IMX335-00BBFF)](https://www.arducam.com/)
[![CivicSense](https://img.shields.io/badge/CivicSense-Part%20of%20the%20ecosystem-8A2BE2)](https://github.com/arpanpathak/driving-civicsense-vision-model)

> Part of the [CivicSense](https://github.com/arpanpathak/driving-civicsense-vision-model) ecosystem: a privacy-first, edge-native AI vision system for intersection discipline and road civility. This server feeds the [stream client](https://github.com/arpanpathak/civicsense-stream-client), which runs YOLOv8 object detection on every frame, entirely on device.

---

## 📦 What's Inside

| Component | Detail |
|---|---|
| Camera | Arducam 5MP IMX335 low light camera (M12 lens, 15-15 and 15-22 pin cables) |
| Board | Raspberry Pi Zero 2 W |
| OS | Raspbian GNU/Linux 13 (Trixie), works on Bookworm too |
| Streaming | MJPEG over HTTP (`multipart/x-mixed-replace`) **and** raw MJPEG over UDP datagrams |
| Performance | ~15 FPS at 640x480, memory footprint ~50 MB |
| Language | 100% Rust, zero Python, zero OpenCV |

**Why Rust?** Lower memory usage (~50 MB) and faster startup than Python/OpenCV, which matters on a 512 MB Pi Zero 2 W. The stream stays alive while the rest of the device still has headroom for detection.

---

## 🧭 Table of Contents

1. [Hardware Setup](#1-hardware-setup)
2. [Mac Preparation: SSH Key and Imager](#2-mac-preparation-ssh-key-and-imager)
3. [First Boot and SSH Connection](#3-first-boot-and-ssh-connection)
4. [Camera Configuration](#4-camera-configuration)
5. [Rust Project Setup (on the Pi)](#5-rust-project-setup-on-the-pi)
6. [Build and Run the Streamer](#6-build-and-run-the-streamer)
7. [Systemd Autostart (optional)](#7-systemd-autostart-optional)
8. [Troubleshooting](#8-troubleshooting)
9. [Docker Cross-Compilation](#9-docker-cross-compilation)
10. [UDP Streaming for the Phone App](#10-udp-streaming-for-the-phone-app)
11. [WiFi Hotspot Mode](#11-wifi-hotspot-mode)
12. [Project Layout](#12-project-layout)

---

## 1. Hardware Setup

- Connect the camera ribbon cable to the Pi Zero 2 W CSI port (the long connector near the USB port).
- **Orientation is critical**: the metal contacts on the cable must face **away from the USB ports** (toward the HDMI port).
- Plug in a **5V 2.5A** micro-USB power supply. Avoid powering from a computer's USB port, it is too weak.

---

## 2. Mac Preparation: SSH Key and Imager

### Generate an SSH key (if you don't have one)

```bash
ssh-keygen -t ed25519 -a 100 -C "your_email@example.com"
# Save as ~/.ssh/civicsense (custom name)
```

### Flash the SD card with Raspberry Pi Imager

Open Raspberry Pi Imager on your Mac and choose **Raspberry Pi OS (other) → Raspberry Pi OS (32-bit)** (or the Lite version). Click the gear icon (⚙️) and set:

- **Hostname**: `civicsense`
- **Enable SSH** → Allow public-key authentication only
- **Authorized keys**: paste your public key (from `~/.ssh/civicsense.pub`)
- **Username**: `civicsense` (or whatever you prefer)
- **WiFi SSID and password** (critical for headless operation)
- Select your country

Write the image to the SD card.

---

## 3. First Boot and SSH Connection

Insert the SD card, power on the Pi, and wait about 2 minutes. Find its IP by checking your router, or use `arp -a`. In this guide it is `192.168.0.43`.

```bash
ssh -i ~/.ssh/civicsense civicsense@192.168.0.43
```

If you get a host key warning, clear it with:

```bash
ssh-keygen -R 192.168.0.43
```

Then retry.

---

## 4. Camera Configuration

### Install the Arducam libcamera packages (critical)

On the Pi:

```bash
wget -O install_pivariety_pkgs.sh https://github.com/ArduCAM/Arducam-Pivariety-V4L2-Driver/releases/download/install_script/install_pivariety_pkgs.sh
chmod +x install_pivariety_pkgs.sh
./install_pivariety_pkgs.sh -p libcamera_dev
./install_pivariety_pkgs.sh -p libcamera_apps
```

### Edit `/boot/firmware/config.txt`

```bash
sudo nano /boot/firmware/config.txt
```

Make sure these lines are present (add or uncomment them):

```text
camera_auto_detect=0
dtoverlay=imx335
```

Save, exit, and reboot:

```bash
sudo reboot
```

### Test the camera

After reboot, log back in and run:

```bash
rpicam-hello --list-cameras
```

You should see:

```text
0 : imx335 [2624x1944 12-bit RGGB] ...
```

---

## 5. Rust Project Setup (on the Pi)

### Install Rust (if not already)

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

### Copy your project to the Pi

From your Mac (replace the path with your actual project location):

```bash
scp -i ~/.ssh/civicsense -r /Users/arpanpathak/Projects/rust/pi_stream civicsense@192.168.0.43:~/projects/
```

On the Pi, navigate to it:

```bash
cd ~/projects/pi_stream
```

(Optional) Remove the existing target folder to clean up:

```bash
rm -rf target
```

> **Why build on the Pi and not cross-compile?** To avoid glibc mismatches and segfaults, we used to build directly on the Pi. Now we cross-compile **inside Docker** ([section 9](#9-docker-cross-compilation)) against the exact glibc each OS uses, which is faster and reproducible on any machine.

---

## 6. Build and Run the Streamer

Build the release binaries (all three at once):

```bash
cargo build --release
```

Run the HTTP server (original behavior):

```bash
./target/release/pi_stream_http
```

Run the UDP streamer (phone app):

```bash
./target/release/pi_stream_udp
```

Run the all-in-one (HTTP + UDP, for hotspot mode):

```bash
./target/release/pi_stream
```

You'll see:

```text
🌐 HTTP MJPEG streaming on http://0.0.0.0:8000
📡 UDP MJPEG streaming on port 9000 (broadcast: true)
```

Open your browser and go to `http://192.168.0.43:8000` and you should see the live video.

**Performance tip:** for 15 FPS at 640x480, the Pi Zero 2 W uses about 40-60% CPU. Increase the resolution to 800x600 or 1280x720 if you have a good power supply.

---

## 7. Systemd Autostart (optional)

The systemd unit lives in the repo at `deploy/pi_stream.service`. The fastest path is the install script (build first, see [section 9](#9-docker-cross-compilation)):

```bash
sudo ./scripts/install.sh
```

Or create the service manually:

```bash
sudo cp deploy/pi_stream.service /etc/systemd/system/pi_stream.service
```

Then enable and start:

```bash
sudo systemctl enable pi_stream
sudo systemctl start pi_stream
```

Now the stream starts automatically on boot.

---

## 8. Troubleshooting

| Problem | Solution |
|---|---|
| `rpicam-hello` command not found | Install rpicam-apps: `sudo apt install rpicam-apps` |
| `supply ovdd` not found in dmesg | This is a warning, ignore it; the Arducam packages handle it |
| Camera not listed in `--list-cameras` | Check ribbon cable orientation and that `dtoverlay=imx335` is set |
| SSH connection refused | Make sure SSH is enabled and the ssh file exists on the boot partition |
| Segfault when running the Rust binary | Build natively on the Pi, don't cross-compile |
| Low FPS | Reduce resolution in the code (change `--width` and `--height`) or lower the framerate |
| WiFi drops | Add the disable-WiFi-power-save service below |

### Disable WiFi power save (stability fix)

Create `/etc/systemd/system/disable-wifi-powersave.service`:

```ini
[Unit]
Description=Disable Wi-Fi power save
After=network.target

[Service]
Type=oneshot
ExecStart=/usr/sbin/iw dev wlan0 set power_save off

[Install]
WantedBy=multi-user.target
```

Then:

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now disable-wifi-powersave.service
```

---

## 9. Docker Cross-Compilation

The whole project cross-compiles inside one Docker container, so the build is identical on any machine (that's the point of the "dockerized" approach). One build produces binaries for **every supported Pi OS**:

| Target triple | Runs on |
|---|---|
| `armv7-unknown-linux-gnueabihf` | Pi Zero 2 W / Pi 2 / Pi 3 — Raspbian **32-bit** (what this project uses) |
| `aarch64-unknown-linux-gnu` | Pi 3 / Pi 4 / Pi 5 / Zero 2 W — Raspberry Pi OS **64-bit** |
| `x86_64-unknown-linux-gnu` | dev machines / CI |

### Quick start

```bash
make build
```

Results land in `bin/<target-triple>/`:

```
bin/
├── armv7-unknown-linux-gnueabihf/pi_stream
├── armv7-unknown-linux-gnueabihf/pi_stream_http
├── armv7-unknown-linux-gnueabihf/pi_stream_udp
├── aarch64-unknown-linux-gnu/...
└── x86_64-unknown-linux-gnu/...
```

Build just one target: `make build TARGETS="aarch64-unknown-linux-gnu"`.

No buildx? Use the fallback: `make build-legacy` (plain `docker build` + `docker cp`).

The binaries link against the container's glibc, which matches the glibc on the corresponding Raspberry Pi OS release — no more segfaults from glibc mismatches.

## 10. UDP Streaming for the Phone App

`pi_stream_udp` (and the all-in-one `pi_stream`) sends the live camera as raw MJPEG UDP datagrams. The phone app listens on the port and renders the latest complete frame.

### Wire format (16-byte header, little-endian integers)

| offset | size | field | meaning |
|---|---|---|---|
| 0 | 4 | magic | `0x50 0x49 0x53 0x54` ("PIST") |
| 4 | 4 | sequence | per-frame counter (wraps at u32::MAX) |
| 8 | 2 | fragment | 0-based index of this datagram within the frame |
| 10 | 2 | total | datagrams that make up this frame (1 = whole) |
| 12 | 4 | length | JPEG payload bytes in *this* datagram |
| 16 | .. | payload | JPEG fragment |

Frames bigger than `65507 - 16` bytes span several datagrams. The app buffers fragments with the same `sequence` until `fragment + 1 == total`, then reassembles.

### Discovery ("connect")

The app sends the 4-byte packet `PING` to `<pi-ip>:<port>`. The Pi replies `PONG` and (when broadcast is off) unicasts frames to that address. With broadcast on — the default — the Pi pushes frames to `255.255.255.255:<port>` and the app just listens; no IP knowledge needed.

### Configuration

| Variable | Default | Meaning |
|---|---|---|
| `PI_STREAM_UDP_PORT` | 9000 | UDP port |
| `PI_STREAM_UDP_BROADCAST` | 1 | send to 255.255.255.255 |
| `PI_STREAM_UDP_FPS` | 15 | UDP send rate |
| `PI_STREAM_UDP_TARGETS` | — | extra unicast `ip[:port]` list |

## 11. WiFi Hotspot Mode

Plug the Pi into the car's USB-A port: the Pi becomes a camera hotspot. Power comes from the USB port, the Pi broadcasts its own WiFi network, the phone joins it and streams over UDP — no router, no internet needed.

Two scripts are provided; pick the one that matches your OS:

| Script | For |
|---|---|
| `scripts/hotspot-nmcli.sh` | Raspberry Pi OS (desktop or Lite) with NetworkManager — **default since Bookworm** |
| `scripts/hotspot-hostapd.sh` | classic hostapd + dnsmasq setups (older Lite, or NetworkManager disabled) |

Both bring up SSID `CivicSense` (password `civicsense123`, override with `PI_STREAM_SSID` / `PI_STREAM_PASSWORD`) and set the Pi as the access point:

```bash
sudo ./scripts/hotspot-nmcli.sh
sudo systemctl start pi_stream   # all-in-one: HTTP :8000 + UDP :9000
```

The phone connects to `CivicSense`, opens the app, and receives MJPEG datagrams on UDP 9000.

## 12. Project Layout

```
pi_stream/
├── src/
│   ├── lib.rs               # library crate root
│   ├── config.rs            # env-var configuration shared by all binaries
│   ├── frame.rs             # shared latest-frame store + JPEG demuxer
│   ├── camera.rs            # rpicam-vid subprocess wrapper
│   ├── http.rs              # HTTP MJPEG server (original code)
│   ├── udp.rs               # UDP MJPEG datagram sender + PING discovery
│   └── bin/
│       ├── pi_stream.rs     # all-in-one: HTTP + UDP (hotspot mode)
│       ├── pi_stream_http.rs# HTTP-only server (original behavior)
│       └── pi_stream_udp.rs # UDP-only streamer (phone app)
├── bin/                     # cross-compiled artifacts (gitignored)
├── scripts/
│   ├── hotspot-nmcli.sh     # NetworkManager hotspot
│   ├── hotspot-hostapd.sh   # hostapd + dnsmasq hotspot
│   └── install.sh           # install binaries + systemd unit
├── deploy/pi_stream.service # systemd unit
├── Dockerfile               # multi-arch cross-compile builder
├── Makefile                 # make build → ./bin
└── .cargo/config.toml       # cross-linkers for armv7 / aarch64
```

---

## 🤝 Credits

- [Arducam](https://www.arducam.com/) for their IMX335 driver packages
- The Rust community for std and cross
- bachp for the raspivid-mjpeg-server (C fallback)

## 📄 License

MIT, do whatever you want with this code. See [LICENSE](LICENSE).
