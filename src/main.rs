// ============================================================
// 1. Imports – only what we need.
//    `BufReader` for efficient reading of camera output.
//    `Read/Write` for I/O traits.
//    `TcpListener/Stream` for HTTP server.
//    `Command/Stdio` to spawn the camera process.
//    `Arc/Mutex` for thread‑safe shared state.
//    `thread` and `Duration` for timing.
// ============================================================
use std::io::{BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

// Type alias for the shared frame store.
// `Arc` allows multiple threads to own it.
// `Mutex` ensures safe access (one writer, multiple readers).
// `Option` because we may not have a frame yet at startup.
type SharedFrame = Arc<Mutex<Option<Vec<u8>>>>;

// ============================================================
// 2. main() – sets up the camera reader and the HTTP server.
//    Both run concurrently.  If either fails, we exit with an error.
// ============================================================
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create the shared, empty frame storage.
    let frame_store = SharedFrame::default();

    // Start the background thread that reads from `rpicam-vid`.
    let camera_handle = spawn_camera_thread(frame_store.clone())?;

    // Start the HTTP server – this runs forever.
    start_http_server(frame_store)?;

    // Wait for the camera thread (should never exit, but we join anyway).
    camera_handle.join().unwrap_or_else(|e| eprintln!("Camera thread panicked: {:?}", e));
    Ok(())
}

// ============================================================
// 3. spawn_camera_thread() – launches rpicam-vid and returns a thread handle.
//    The thread continuously reads MJPEG frames from the child's stdout,
//    extracts complete JPEGs, and stores the latest one in `frame_store`.
// ============================================================
fn spawn_camera_thread(store: SharedFrame) -> Result<thread::JoinHandle<()>, std::io::Error> {
    // Launch `rpicam-vid` with MJPEG output.
    // `-t 0` means run forever; `--output -` writes to stdout.
    let child = Command::new("rpicam-vid")
        .args([
            "-t", "0",               // indefinitely
            "--width", "640",        // decent resolution for Pi Zero 2W
            "--height", "480",
            "--framerate", "15",     // good balance between quality and CPU
            "--codec", "mjpeg",      // each frame is a standalone JPEG – easy to parse
            "--output", "-",         // send to stdout
        ])
        .stdout(Stdio::piped())      // capture stdout
        .stderr(Stdio::null())       // discard stderr to avoid clutter
        .spawn()?;                   // propagate error if process fails

    // Take ownership of stdout – if None, the process didn't pipe it (shouldn't happen).
    let stdout = child.stdout.ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::BrokenPipe, "No stdout from rpicam-vid")
    })?;

    // Spawn the reader thread.
    let handle = thread::spawn(move || {
        // `BufReader` reduces system calls.
        let mut reader = BufReader::new(stdout);
        let mut buffer = Vec::new();   // accumulates raw bytes from the stream
        let mut chunk = [0u8; 4096];   // fixed‑size buffer for reading chunks

        // Read in a loop until the child process exits.
        // `while let` with `?` inside would panic on error, so we use `match` to handle errors gracefully.
        while let Ok(n) = reader.read(&mut chunk) {
            if n == 0 {
                break;  // EOF – camera died
            }
            // Append new data to the buffer.
            buffer.extend_from_slice(&chunk[..n]);

            // Extract as many complete JPEGs as possible.
            // `extract_jpeg` returns `Option<Vec<u8>>` and removes the frame from `buffer`.
            while let Some(frame) = extract_jpeg(&mut buffer) {
                // Store the frame, replacing any previous one.
                // We ignore lock errors – if the mutex is poisoned, just skip the frame.
                let _ = store.lock().map(|mut guard| *guard = Some(frame));
            }
        }
    });

    Ok(handle)
}

// ============================================================
// 4. extract_jpeg() – searches the buffer for a complete JPEG.
//    JPEG frames in the MJPEG stream are delimited by:
//      * start marker:  0xFF 0xD8
//      * end marker:    0xFF 0xD9
//    This function returns the frame and removes it from the buffer.
//    If no complete frame is found, it returns `None`.
// ============================================================
fn extract_jpeg(buffer: &mut Vec<u8>) -> Option<Vec<u8>> {
    // Find the first occurrence of the start marker.
    let start = buffer.windows(2).position(|w| w == [0xFF, 0xD8])?;

    // From the start, find the first end marker.
    let end = buffer[start..].windows(2).position(|w| w == [0xFF, 0xD9])?;
    let end_pos = start + end + 2; // include the two bytes of the end marker

    // Extract the frame and drain it from the buffer.
    let frame = buffer[start..end_pos].to_vec();
    buffer.drain(..end_pos);

    Some(frame)
}

// ============================================================
// 5. start_http_server() – binds to port 8000 and spawns a thread per client.
//    Each client receives a MJPEG stream via HTTP multipart responses.
// ============================================================
fn start_http_server(store: SharedFrame) -> Result<(), std::io::Error> {
    let listener = TcpListener::bind("0.0.0.0:8000")?;
    println!("🌐 Streaming on http://0.0.0.0:8000");

    // `filter_map(Result::ok)` ignores failed connections (logged elsewhere).
    for stream in listener.incoming().filter_map(Result::ok) {
        let store = store.clone();
        // Each client gets its own thread so they don't block each other.
        thread::spawn(move || handle_client(stream, store));
    }

    Ok(())
}

// ============================================================
// 6. handle_client() – sends MJPEG stream to a single client.
//    It sends the HTTP headers once, then repeatedly sends the latest frame.
//    If the client disconnects, we break out of the loop.
// ============================================================
fn handle_client(mut stream: TcpStream, store: SharedFrame) {
    // Read the HTTP request (we don't need it, but must consume it).
    let _ = stream.read(&mut [0u8; 1024]);

    // Send the mandatory headers for MJPEG streaming.
    // `multipart/x-mixed-replace` tells the browser to replace the image with each new part.
    let headers = concat!(
        "HTTP/1.1 200 OK\r\n",
        "Server: pi-stream\r\n",
        "Cache-Control: no-cache\r\n",
        "Pragma: no-cache\r\n",
        "Connection: close\r\n",
        "Content-Type: multipart/x-mixed-replace; boundary=--frame\r\n\r\n"
    );

    // If writing headers fails, the client is already gone – just exit.
    if stream.write_all(headers.as_bytes()).is_err() {
        return;
    }

    // Main loop: fetch the latest frame and send it.
    loop {
        // Try to lock the mutex. If it's poisoned, we just get `None`.
        let frame = match store.lock() {
            Ok(guard) => guard.clone(),
            Err(_) => None,
        };

        // If no frame yet, wait a bit and try again.
        let Some(frame) = frame else {
            thread::sleep(Duration::from_millis(50));
            continue;
        };

        // Build the multipart boundary with the frame's length.
        let boundary = format!(
            "\r\n--frame\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n",
            frame.len()
        );

        // Write boundary and frame. If any write fails, the connection is broken – exit.
        if stream.write_all(boundary.as_bytes()).is_err() {
            break;
        }
        if stream.write_all(&frame).is_err() {
            break;
        }

        // Small sleep to avoid CPU spinning when new frames arrive slowly.
        thread::sleep(Duration::from_millis(50));
    }
}

