// ============================================================
// frame — the single source of truth for video frames.
//
// `SharedFrame` is an `Arc<Mutex<Option<Vec<u8>>>>`:
//   * the camera thread writes the newest complete JPEG frame,
//   * every output module (HTTP, UDP) reads the same store,
//   * `Option` is empty until the camera produces its first frame.
//
// `extract_jpeg` is the MJPEG demuxer: `rpicam-vid` emits a
// continuous stream of concatenated JPEGs on stdout, each
// delimited by the start marker 0xFF 0xD8 and end marker 0xFF 0xD9.
// ============================================================

use std::sync::{Arc, Mutex};

/// Thread-safe handle to the latest camera frame.
pub type SharedFrame = Arc<Mutex<Option<Vec<u8>>>>;

/// Scans `buffer` for the first complete JPEG frame.
///
/// Returns the frame (including both markers) and drains it from
/// `buffer`, so the caller can keep reading the remaining stream.
/// Returns `None` when no complete frame is present yet.
pub fn extract_jpeg(buffer: &mut Vec<u8>) -> Option<Vec<u8>> {
    // Find the first occurrence of the JPEG start marker.
    let start = buffer.windows(2).position(|w| w == [0xFF, 0xD8])?;

    // From the start, find the first end marker.
    let end = buffer[start..].windows(2).position(|w| w == [0xFF, 0xD9])?;
    let end_pos = start + end + 2; // include the two end-marker bytes

    // Extract the frame and drain it from the buffer.
    let frame = buffer[start..end_pos].to_vec();
    buffer.drain(..end_pos);

    Some(frame)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_one_frame_and_drains_buffer() {
        let mut buffer = b"\xff\xd8abc\xff\xd9XXXX".to_vec();
        let frame = extract_jpeg(&mut buffer).expect("a frame");
        assert_eq!(frame, b"\xff\xd8abc\xff\xd9");
        assert_eq!(buffer, b"XXXX");
    }

    #[test]
    fn returns_none_without_complete_frame() {
        // Start marker present but no end marker yet.
        let mut buffer = b"junk\xff\xd8partial".to_vec();
        assert!(extract_jpeg(&mut buffer).is_none());
        // Nothing extracted, buffer untouched.
        assert_eq!(buffer, b"junk\xff\xd8partial");
    }
}
