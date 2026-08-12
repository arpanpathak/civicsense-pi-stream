# 🎥 CivicSense Pi Stream: MJPEG Streaming Server in Rust

A lightweight, production-ready MJPEG streaming server for the **Raspberry Pi Zero 2 W** with an **Arducam IMX335** camera module. Built in Rust with no Python and no OpenCV, it is a single static binary that shells out to `rpicam-vid` under the hood.

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
| Streaming | MJPEG over HTTP, served as `multipart/x-mixed-replace` |
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

> **Why build on the Pi and not cross-compile?** To avoid glibc mismatches and segfaults. Building directly on the Pi, even if slower, guarantees binary compatibility. (A cross-compile linker config for `armv7-unknown-linux-gnueabihf` is included in `.cargo/config.toml` if you do want to try.)

---

## 6. Build and Run the Streamer

Build the release binary:

```bash
cargo build --release
```

Run it:

```bash
./target/release/pi_stream
```

You'll see:

```text
🌐 Streaming on http://0.0.0.0:8000
```

Open your browser and go to `http://192.168.0.43:8000` and you should see the live video.

**Performance tip:** for 15 FPS at 640x480, the Pi Zero 2 W uses about 40-60% CPU. Increase the resolution to 800x600 or 1280x720 if you have a good power supply.

---

## 7. Systemd Autostart (optional)

Create a service file:

```bash
sudo nano /etc/systemd/system/pi_stream.service
```

Paste:

```ini
[Unit]
Description=Pi Camera MJPEG Stream
After=network.target

[Service]
ExecStart=/home/civicsense/projects/pi_stream/target/release/pi_stream
Restart=always
User=civicsense

[Install]
WantedBy=multi-user.target
```

Enable and start:

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

## 🤝 Credits

- [Arducam](https://www.arducam.com/) for their IMX335 driver packages
- The Rust community for std and cross
- bachp for the raspivid-mjpeg-server (C fallback)

## 📄 License

MIT, do whatever you want with this code. See [LICENSE](LICENSE).
