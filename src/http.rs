// ============================================================
// http — MJPEG over HTTP (the original pi_stream behavior).
//
// Serves the shared frame store as `multipart/x-mixed-replace` so
// any browser or `<img>` tag shows a live stream at
// `http://<pi-ip>:<port>`.
// ============================================================

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

use crate::config::HttpConfig;
use crate::frame::SharedFrame;

/// Starts the HTTP MJPEG server on `0.0.0.0:<port>` and never returns.
/// Each client gets its own thread so they don't block each other.
pub fn start_http_server(store: SharedFrame, cfg: &HttpConfig) -> Result<(), std::io::Error> {
    let listener = TcpListener::bind(("0.0.0.0", cfg.port))?;
    println!("🌐 HTTP MJPEG streaming on http://0.0.0.0:{}", cfg.port);

    // `filter_map(Result::ok)` ignores failed connections.
    for stream in listener.incoming().filter_map(Result::ok) {
        let store = store.clone();
        thread::spawn(move || handle_client(stream, store));
    }

    Ok(())
}

/// Sends the MJPEG stream to a single client.
///
/// The HTTP headers are written once, then the latest frame is pushed
/// repeatedly. If any write fails, the client has disconnected and we
/// exit the loop.
fn handle_client(mut stream: TcpStream, store: SharedFrame) {
    // Read the HTTP request (we don't need it, but must consume it).
    let _ = stream.read(&mut [0u8; 1024]);

    // `multipart/x-mixed-replace` tells the browser to replace the
    // image with each new part as it arrives.
    let headers = concat!(
        "HTTP/1.1 200 OK\r\n",
        "Server: pi-stream\r\n",
        "Cache-Control: no-cache\r\n",
        "Pragma: no-cache\r\n",
        "Connection: close\r\n",
        "Content-Type: multipart/x-mixed-replace; boundary=--frame\r\n\r\n"
    );

    // If writing headers fails, the client is already gone — just exit.
    if stream.write_all(headers.as_bytes()).is_err() {
        return;
    }

    loop {
        // Try to lock the mutex; a poisoned mutex simply yields `None`.
        let frame = match store.lock() {
            Ok(guard) => guard.clone(),
            Err(_) => None,
        };

        // No frame yet — wait a bit and try again.
        let Some(frame) = frame else {
            thread::sleep(Duration::from_millis(50));
            continue;
        };

        // Build the multipart boundary with the frame's length.
        let boundary = format!(
            "\r\n--frame\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n",
            frame.len()
        );

        // Write boundary + frame. Any failure means the connection is
        // broken — exit.
        if stream.write_all(boundary.as_bytes()).is_err() {
            break;
        }
        if stream.write_all(&frame).is_err() {
            break;
        }

        // Small sleep to avoid CPU spinning when frames arrive slowly.
        thread::sleep(Duration::from_millis(50));
    }
}
