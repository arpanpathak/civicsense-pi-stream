// ============================================================
// camera — rpicam-vid subprocess wrapper.
//
// `rpicam-vid` (the modern replacement for `raspivid`) encodes the
// Arducam IMX335 sensor to MJPEG and writes the stream to stdout.
// We spawn it once, read stdout in a dedicated thread, demux the
// concatenated JPEGs with `extract_jpeg` and publish every complete
// frame to the `SharedFrame` store.
// ============================================================

use std::io::{BufReader, Read};
use std::process::{Command, Stdio};
use std::thread;

use crate::config::CameraConfig;
use crate::frame::{extract_jpeg, SharedFrame};

/// Launches `rpicam-vid` and returns a handle to the reader thread.
///
/// The reader thread runs until the camera process exits (crash,
/// EOF, SIGKILL, ...). Errors spawning the process are returned to
/// the caller; reader errors simply end the loop (the process died
/// anyway) and the frame store keeps its last frame.
pub fn spawn_camera_thread(
    store: SharedFrame,
    cfg: &CameraConfig,
) -> Result<thread::JoinHandle<()>, std::io::Error> {
    // Build the rpicam-vid command line.
    // `-t 0` means run forever; `--output -` writes to stdout.
    let mut cmd = Command::new(&cfg.bin);
    cmd.arg("-t").arg("0") // indefinitely
        .arg("--width").arg(cfg.width.to_string())
        .arg("--height").arg(cfg.height.to_string())
        .arg("--framerate").arg(cfg.framerate.to_string())
        .arg("--codec").arg("mjpeg") // each frame is a standalone JPEG
        .arg("--output").arg("-"); // send to stdout

    // Capture stdout (the video stream), discard stderr to avoid clutter.
    let child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?; // propagate error if the process fails to start

    // Take ownership of stdout — if None, the process didn't pipe it.
    let stdout = child.stdout.ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::BrokenPipe, "No stdout from rpicam-vid")
    })?;

    // Spawn the reader thread.
    let handle = thread::spawn(move || {
        // `BufReader` reduces system calls.
        let mut reader = BufReader::new(stdout);
        let mut buffer = Vec::new(); // accumulates raw bytes from the stream
        let mut chunk = [0u8; 4096]; // fixed-size chunk buffer

        // Read in a loop until the child process exits.
        // `while let` + `match` keeps the loop alive on transient errors.
        while let Ok(n) = reader.read(&mut chunk) {
            if n == 0 {
                break; // EOF — camera died
            }
            // Append new data to the buffer.
            buffer.extend_from_slice(&chunk[..n]);

            // Extract as many complete JPEGs as possible and publish
            // each one, replacing the previous frame.
            while let Some(frame) = extract_jpeg(&mut buffer) {
                let _ = store.lock().map(|mut guard| *guard = Some(frame));
            }
        }
    });

    Ok(handle)
}
