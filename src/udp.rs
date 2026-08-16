// ============================================================
// udp — MJPEG over UDP datagrams (the phone-app protocol).
//
// Wire format, 16-byte header followed by JPEG payload
// (all integers little-endian):
//
//   offset  size  field     meaning
//   0       4     magic     0x50 0x49 0x53 0x54   ("PIST")
//   4       4     sequence  per-frame counter, wraps at u32::MAX
//   8       2     fragment  0-based index of this datagram in the frame
//   10      2     total     datagrams making up this frame (1 = whole)
//   12      4     length    JPEG payload bytes in THIS datagram
//   16      ..    payload   JPEG fragment
//
// One frame may span several datagrams when it is bigger than
// 65507 - 16 bytes. The phone app buffers fragments with the same
// `sequence` until `fragment + 1 == total`, then reassembles.
//
// Discovery ("phone app can connect"):
//   the app sends the 4-byte packet "PING" to <pi-ip>:<port>; the Pi
//   replies "PONG" and starts unicasting frames to that address.
//   This matters when broadcast is off or the phone is off-subnet.
//
// Delivery rules:
//   * broadcast on  (default) -> 255.255.255.255:<port>, reaches every
//     phone on the hotspot with zero phone-side configuration
//   * broadcast off -> unicast to every address that said "PING"
//   * explicit targets (PI_STREAM_UDP_TARGETS) always get unicast
// ============================================================

use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::config::UdpConfig;
use crate::frame::SharedFrame;

/// Magic bytes that start every video datagram: "PIST".
pub const UDP_MAGIC: [u8; 4] = *b"PIST";
/// Control packet sent by the phone app to subscribe: "PING".
pub const PING_MAGIC: [u8; 4] = *b"PING";
/// Control packet sent back by the Pi to acknowledge: "PONG".
pub const PONG_MAGIC: [u8; 4] = *b"PONG";

/// Fixed header length of a video datagram.
pub const UDP_HEADER_LEN: usize = 16;
/// Largest UDP payload allowed by IP (65535 − 20 IP − 8 UDP).
pub const MAX_UDP_DATAGRAM: usize = 65507;
/// Largest JPEG payload that fits in a single datagram.
pub const MAX_UDP_PAYLOAD: usize = MAX_UDP_DATAGRAM - UDP_HEADER_LEN;

/// A discovered unicast target (a phone) and when it last said "PING".
struct Target {
    addr: SocketAddr,
    last_seen: Instant,
}

/// Phones that stop sending "PING" for this long are dropped from
/// the unicast list.
const TARGET_TTL: Duration = Duration::from_secs(30);

/// Starts the UDP streamer and never returns.
///
/// Runs two cooperating threads:
///   1. a discovery thread that watches for "PING" subscriptions, and
///   2. the sender loop (calling thread) that pushes frames at `fps`.
pub fn start_udp_sender(store: SharedFrame, cfg: &UdpConfig) -> Result<(), std::io::Error> {
    let socket = UdpSocket::bind(("0.0.0.0", cfg.port))?;
    socket.set_broadcast(cfg.broadcast)?;
    println!(
        "📡 UDP MJPEG streaming on port {} (broadcast: {})",
        cfg.port, cfg.broadcast
    );

    // Shared registry of discovered phones: written by the discovery
    // thread, read by the sender loop.
    let targets = Arc::new(Mutex::new(Vec::<Target>::new()));

    // Discovery thread — answer "PING" with "PONG" and register the
    // phone for unicast delivery.
    let disco_socket = socket.try_clone()?;
    let disco_targets = targets.clone();
    thread::spawn(move || discovery_loop(disco_socket, disco_targets));

    // The sender loop runs in the calling thread, forever.
    sender_loop(socket, store, cfg, targets)
}

/// Listens for "PING" control packets and registers senders.
fn discovery_loop(socket: UdpSocket, targets: Arc<Mutex<Vec<Target>>>) {
    let mut buf = [0u8; 64];
    loop {
        let (n, src) = match socket.recv_from(&mut buf) {
            Ok(v) => v,
            Err(_) => continue, // transient error — keep listening
        };

        // Only react to the exact "PING" magic; video datagrams start
        // with "PIST" and are ignored here.
        if n >= 4 && &buf[..4] == PING_MAGIC {
            // Acknowledge so the app knows the subscription is live.
            let _ = socket.send_to(&PONG_MAGIC, src);
            let mut list = targets.lock().unwrap();
            // Refresh: drop any stale entry for the same address, re-add.
            list.retain(|t| t.addr != src);
            list.push(Target { addr: src, last_seen: Instant::now() });
            println!("📱 phone subscribed: {src}");
        }
    }
}

/// Pushes the newest frame to every destination at `fps` frames/sec.
fn sender_loop(
    socket: UdpSocket,
    store: SharedFrame,
    cfg: &UdpConfig,
    targets: Arc<Mutex<Vec<Target>>>,
) -> Result<(), std::io::Error> {
    // The limited broadcast address works on any single-subnet hotspot,
    // so the phone does not even need to know the Pi's IP to receive.
    let broadcast_addr = SocketAddr::new(Ipv4Addr::BROADCAST.into(), cfg.port);
    let interval = Duration::from_millis((1000 / cfg.fps.max(1)) as u64);
    let mut sequence: u32 = 0;

    loop {
        // Grab the newest frame — the same store the HTTP server reads.
        let frame = match store.lock() {
            Ok(guard) => guard.clone(),
            Err(_) => None,
        };

        if let Some(frame) = frame {
            // Decide where this frame goes.
            let mut addrs: Vec<SocketAddr> = Vec::new();
            if cfg.broadcast {
                // One broadcast reaches every phone on the hotspot;
                // discovered unicast is redundant here, so skip it.
                addrs.push(broadcast_addr);
            } else {
                // Broadcast off: deliver to every phone that said "PING".
                let mut list = targets.lock().unwrap();
                list.retain(|t| t.last_seen.elapsed() < TARGET_TTL);
                addrs.extend(list.iter().map(|t| t.addr));
            }
            // Explicit targets always get unicast (e.g. a phone on a
            // different subnet or a fixed relay).
            addrs.extend(cfg.targets.iter().copied());

            for addr in addrs {
                send_frame(&socket, addr, &frame, sequence);
            }
            sequence = sequence.wrapping_add(1);
        }

        thread::sleep(interval);
    }
}

/// Splits one JPEG frame into datagrams and sends each to `addr`.
fn send_frame(socket: &UdpSocket, addr: SocketAddr, frame: &[u8], sequence: u32) {
    // ceil(len / MAX_UDP_PAYLOAD), at least one datagram per frame.
    let total = frame.len().div_ceil(MAX_UDP_PAYLOAD).max(1) as u16;

    for (i, chunk) in frame.chunks(MAX_UDP_PAYLOAD).enumerate() {
        let datagram = build_datagram(chunk, sequence, i as u16, total);
        // Ignore send errors: UDP is fire-and-forget — the next frame
        // replaces anything that gets lost.
        let _ = socket.send_to(&datagram, addr);
    }
}

/// Encodes one fragment of a frame into a full UDP datagram (header +
/// payload). Kept as a pure function so tests can verify the layout.
fn build_datagram(payload: &[u8], sequence: u32, fragment: u16, total: u16) -> Vec<u8> {
    let mut datagram = Vec::with_capacity(UDP_HEADER_LEN + payload.len());
    datagram.extend_from_slice(&UDP_MAGIC);
    datagram.extend_from_slice(&sequence.to_le_bytes());
    datagram.extend_from_slice(&fragment.to_le_bytes());
    datagram.extend_from_slice(&total.to_le_bytes());
    datagram.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    datagram.extend_from_slice(payload);
    datagram
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn datagram_header_layout_is_correct() {
        let payload = b"\xff\xd8jpeg\xff\xd9";
        let dg = build_datagram(payload, 7, 0, 1);

        // magic
        assert_eq!(&dg[0..4], b"PIST");
        // sequence (u32 LE) = 7
        assert_eq!(u32::from_le_bytes(dg[4..8].try_into().unwrap()), 7);
        // fragment / total (u16 LE each)
        assert_eq!(u16::from_le_bytes(dg[8..10].try_into().unwrap()), 0);
        assert_eq!(u16::from_le_bytes(dg[10..12].try_into().unwrap()), 1);
        // payload length (u32 LE)
        assert_eq!(
            u32::from_le_bytes(dg[12..16].try_into().unwrap()) as usize,
            payload.len()
        );
        // payload itself
        assert_eq!(&dg[16..], payload);
    }

    #[test]
    fn fragments_report_index_and_total() {
        // Simulate a large frame split into two datagrams.
        let dg0 = build_datagram(&[1u8; 10], 3, 0, 2);
        let dg1 = build_datagram(&[2u8; 10], 3, 1, 2);
        assert_eq!(u16::from_le_bytes(dg0[10..12].try_into().unwrap()), 2);
        assert_eq!(u16::from_le_bytes(dg1[8..10].try_into().unwrap()), 1);
        assert_eq!(u16::from_le_bytes(dg1[10..12].try_into().unwrap()), 2);
    }
}
