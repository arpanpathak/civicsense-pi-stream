// ============================================================
// config - environment-variable configuration.
//
// Every binary reads the same variables with the same defaults,
// so one set of systemd / Docker settings works everywhere:
//
//   PI_STREAM_CAMERA_BIN     rpicam-vid executable       (rpicam-vid)
//   PI_STREAM_WIDTH          capture width               (640)
//   PI_STREAM_HEIGHT         capture height              (480)
//   PI_STREAM_FRAMERATE      capture FPS                 (15)
//   PI_STREAM_HTTP_PORT      HTTP MJPEG server port      (8000)
//   PI_STREAM_UDP_PORT       UDP streamer port           (9000)
//   PI_STREAM_UDP_BROADCAST  0/1 send to 255.255.255.255 (1)
//   PI_STREAM_UDP_FPS        UDP send rate               (15)
//   PI_STREAM_UDP_TARGETS    comma-separated ip[:port]   (none)
//
// No config file to manage - just set variables in the systemd unit,
// a shell wrapper, or a container.
// ============================================================

use std::net::{IpAddr, SocketAddr};

/// Returns `default` when the variable is unset or empty.
fn env_or(key: &str, default: &str) -> String {
    std::env::var(key)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| default.to_string())
}

/// Parses a u16 environment variable, falling back to `default`.
fn env_u16(key: &str, default: u16) -> u16 {
    env_or(key, &default.to_string())
        .parse()
        .unwrap_or(default)
}

/// Parses a boolean environment variable ("1"/"true"/"yes"/"on" -> true).
fn env_bool(key: &str, default: bool) -> bool {
    match env_or(key, if default { "1" } else { "0" })
        .to_ascii_lowercase()
        .as_str()
    {
        "1" | "true" | "yes" | "on" => true,
        "0" | "false" | "no" | "off" => false,
        _ => default,
    }
}

/// Camera capture settings (defaults mirror the original hard-coded values).
#[derive(Debug, Clone)]
pub struct CameraConfig {
    /// Path (or name) of the `rpicam-vid` executable.
    pub bin: String,
    pub width: u16,
    pub height: u16,
    pub framerate: u16,
}

impl CameraConfig {
    pub fn from_env() -> Self {
        Self {
            bin: env_or("PI_STREAM_CAMERA_BIN", "rpicam-vid"),
            width: env_u16("PI_STREAM_WIDTH", 640),
            height: env_u16("PI_STREAM_HEIGHT", 480),
            framerate: env_u16("PI_STREAM_FRAMERATE", 15),
        }
    }
}

/// HTTP MJPEG server settings.
#[derive(Debug, Clone)]
pub struct HttpConfig {
    pub port: u16,
}

impl HttpConfig {
    pub fn from_env() -> Self {
        Self {
            port: env_u16("PI_STREAM_HTTP_PORT", 8000),
        }
    }
}

/// UDP MJPEG streamer settings.
#[derive(Debug, Clone)]
pub struct UdpConfig {
    pub port: u16,
    /// When true (default) frames go to 255.255.255.255:<port>, which
    /// every phone on the hotspot receives without any configuration.
    pub broadcast: bool,
    /// UDP send rate in frames per second.
    pub fps: u16,
    /// Fixed unicast destinations, e.g. "192.168.4.50:9000,10.0.0.9".
    /// Always delivered, even when broadcast is on.
    pub targets: Vec<SocketAddr>,
}

impl UdpConfig {
    pub fn from_env() -> Self {
        let port = env_u16("PI_STREAM_UDP_PORT", 9000);
        Self {
            port,
            broadcast: env_bool("PI_STREAM_UDP_BROADCAST", true),
            fps: env_u16("PI_STREAM_UDP_FPS", 15),
            targets: parse_targets(&env_or("PI_STREAM_UDP_TARGETS", ""), port),
        }
    }
}

/// Parses "ip", "ip:port", "ip:port,ip2:port2" into `SocketAddr`s.
/// A bare IP gets the streamer's own port.
fn parse_targets(raw: &str, default_port: u16) -> Vec<SocketAddr> {
    raw.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .filter_map(|s| match s.parse::<SocketAddr>() {
            Ok(addr) => Some(addr),
            Err(_) => s
                .parse::<IpAddr>()
                .ok()
                .map(|ip| SocketAddr::new(ip, default_port)),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bare_ip_with_default_port() {
        let out = parse_targets("192.168.4.50", 9000);
        assert_eq!(out, vec!["192.168.4.50:9000".parse().unwrap()]);
    }

    #[test]
    fn parses_mixed_list_and_ignores_garbage() {
        let out = parse_targets(" 10.0.0.9:8000, 192.168.4.50 , not-an-ip", 9000);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0], "10.0.0.9:8000".parse().unwrap());
        assert_eq!(out[1], "192.168.4.50:9000".parse().unwrap());
    }

    #[test]
    fn empty_string_yields_no_targets() {
        assert!(parse_targets("", 9000).is_empty());
    }
}
