// ============================================================
// pi_stream_http — HTTP MJPEG streaming server.
//
// This is the original pi_stream behavior, preserved as its own
// binary. Serves the live camera as multipart MJPEG on port 8000
// (override with PI_STREAM_HTTP_PORT).
//
//   PI_STREAM_CAMERA_BIN / WIDTH / HEIGHT / FRAMERATE also apply.
// ============================================================

use pi_stream::config::{CameraConfig, HttpConfig};
use pi_stream::frame::SharedFrame;
use pi_stream::{camera, http};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create the shared, empty frame storage.
    let frame_store = SharedFrame::default();

    // Read configuration from the environment (with defaults).
    let camera_cfg = CameraConfig::from_env();
    let http_cfg = HttpConfig::from_env();

    // Start the background thread that reads from `rpicam-vid`.
    let camera_handle = camera::spawn_camera_thread(frame_store.clone(), &camera_cfg)?;

    // Start the HTTP server — this runs forever.
    http::start_http_server(frame_store, &http_cfg)?;

    // Wait for the camera thread (never exits, but join anyway).
    camera_handle
        .join()
        .unwrap_or_else(|e| eprintln!("Camera thread panicked: {:?}", e));
    Ok(())
}
