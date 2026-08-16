// ============================================================
// pi_stream_udp — UDP MJPEG datagram streamer (phone-app protocol).
//
// Sends the live camera as raw MJPEG datagrams on port 9000
// (override with PI_STREAM_UDP_PORT). See src/udp.rs for the wire
// format and the PING/PONG discovery handshake.
//
//   PI_STREAM_CAMERA_BIN / WIDTH / HEIGHT / FRAMERATE also apply.
// ============================================================

use pi_stream::config::{CameraConfig, UdpConfig};
use pi_stream::frame::SharedFrame;
use pi_stream::{camera, udp};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create the shared, empty frame storage.
    let frame_store = SharedFrame::default();

    // Read configuration from the environment (with defaults).
    let camera_cfg = CameraConfig::from_env();
    let udp_cfg = UdpConfig::from_env();

    // Start the background thread that reads from `rpicam-vid`.
    let camera_handle = camera::spawn_camera_thread(frame_store.clone(), &camera_cfg)?;

    // Start the UDP streamer — this runs forever.
    udp::start_udp_sender(frame_store, &udp_cfg)?;

    // Wait for the camera thread (never exits, but join anyway).
    camera_handle
        .join()
        .unwrap_or_else(|e| eprintln!("Camera thread panicked: {:?}", e));
    Ok(())
}
