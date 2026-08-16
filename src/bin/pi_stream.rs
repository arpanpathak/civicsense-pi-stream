// ============================================================
// pi_stream - all-in-one streamer for WiFi-hotspot mode.
//
// Runs BOTH transports off one camera:
//   * HTTP MJPEG on port 8000  (browser / legacy clients)
//   * UDP MJPEG datagrams on port 9000  (phone app, broadcast)
//
// This is the binary the systemd unit starts after the hotspot
// scripts bring the Pi's access point up.
//
//   PI_STREAM_HTTP_PORT / PI_STREAM_UDP_PORT / PI_STREAM_UDP_BROADCAST
//   / PI_STREAM_UDP_TARGETS / PI_STREAM_CAMERA_* all apply.
// ============================================================

use std::thread;

use pi_stream::config::{CameraConfig, HttpConfig, UdpConfig};
use pi_stream::frame::SharedFrame;
use pi_stream::{camera, http, udp};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create the shared, empty frame storage.
    let frame_store = SharedFrame::default();

    // Read configuration from the environment (with defaults).
    let camera_cfg = CameraConfig::from_env();
    let http_cfg = HttpConfig::from_env();
    let udp_cfg = UdpConfig::from_env();

    // Start the background thread that reads from `rpicam-vid`.
    let camera_handle = camera::spawn_camera_thread(frame_store.clone(), &camera_cfg)?;

    // HTTP server runs in its own thread (it blocks forever).
    let http_store = frame_store.clone();
    let http_handle = thread::spawn(move || {
        if let Err(e) = http::start_http_server(http_store, &http_cfg) {
            eprintln!("HTTP server error: {e}");
        }
    });

    // UDP streamer runs in the main thread (also forever).
    udp::start_udp_sender(frame_store, &udp_cfg)?;

    // Unreachable in practice, but join both threads for cleanliness.
    let _ = http_handle.join();
    let _ = camera_handle.join();
    Ok(())
}
