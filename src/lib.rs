// ============================================================
// CivicSense Pi Stream - library crate
//
// The crate is split into a reusable library plus three small
// binaries so every deployment mode shares the same code:
//
//   pi_stream_http  – original HTTP MJPEG server   (port 8000)
//   pi_stream_udp   – UDP MJPEG datagram streamer  (port 9000)
//   pi_stream       – all-in-one: HTTP + UDP, for WiFi-hotspot mode
//
// Module map:
//   config – environment-variable configuration shared by all binaries
//   frame  – the shared "latest frame" store + JPEG boundary parser
//   camera – rpicam-vid subprocess wrapper (MJPEG over stdout)
//   http   – HTTP multipart/x-mixed-replace MJPEG server
//   udp    – UDP MJPEG datagram sender with PING discovery
// ============================================================

pub mod camera;
pub mod config;
pub mod frame;
pub mod http;
pub mod udp;
