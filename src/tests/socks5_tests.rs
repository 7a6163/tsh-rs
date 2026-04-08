use crate::socks5::TargetAddr;

// ─── TargetAddr serialization ────────────────────────────────────────────────

#[test]
fn test_target_addr_round_trip_ipv4() {
    let addr = TargetAddr {
        host: "192.168.1.100".to_string(),
        port: 8080,
    };
    let bytes = addr.to_bytes();
    let parsed = TargetAddr::from_bytes(&bytes).expect("should parse");
    assert_eq!(parsed.host, "192.168.1.100");
    assert_eq!(parsed.port, 8080);
}

#[test]
fn test_target_addr_round_trip_domain() {
    let addr = TargetAddr {
        host: "example.com".to_string(),
        port: 443,
    };
    let bytes = addr.to_bytes();
    let parsed = TargetAddr::from_bytes(&bytes).expect("should parse");
    assert_eq!(parsed.host, "example.com");
    assert_eq!(parsed.port, 443);
}

#[test]
fn test_target_addr_round_trip_ipv6() {
    let addr = TargetAddr {
        host: "::1".to_string(),
        port: 22,
    };
    let bytes = addr.to_bytes();
    let parsed = TargetAddr::from_bytes(&bytes).expect("should parse");
    assert_eq!(parsed.host, "::1");
    assert_eq!(parsed.port, 22);
}

#[test]
fn test_target_addr_round_trip_port_zero() {
    let addr = TargetAddr {
        host: "host".to_string(),
        port: 0,
    };
    let bytes = addr.to_bytes();
    let parsed = TargetAddr::from_bytes(&bytes).expect("should parse");
    assert_eq!(parsed.port, 0);
}

#[test]
fn test_target_addr_round_trip_port_max() {
    let addr = TargetAddr {
        host: "host".to_string(),
        port: 65535,
    };
    let bytes = addr.to_bytes();
    let parsed = TargetAddr::from_bytes(&bytes).expect("should parse");
    assert_eq!(parsed.port, 65535);
}

#[test]
fn test_target_addr_long_hostname() {
    let long_host = "a".repeat(255);
    let addr = TargetAddr {
        host: long_host.clone(),
        port: 80,
    };
    let bytes = addr.to_bytes();
    let parsed = TargetAddr::from_bytes(&bytes).expect("should parse");
    assert_eq!(parsed.host, long_host);
    assert_eq!(parsed.port, 80);
}

// ─── Wire format structure ───────────────────────────────────────────────────

#[test]
fn test_target_addr_wire_format() {
    let addr = TargetAddr {
        host: "AB".to_string(), // 2 bytes
        port: 0x1F90,           // 8080 in hex
    };
    let bytes = addr.to_bytes();
    // [host_len: u16 BE][host][port: u16 BE]
    assert_eq!(bytes.len(), 2 + 2 + 2); // len(2) + "AB"(2) + port(2)
    assert_eq!(bytes[0], 0x00); // host_len high byte
    assert_eq!(bytes[1], 0x02); // host_len low byte = 2
    assert_eq!(bytes[2], b'A');
    assert_eq!(bytes[3], b'B');
    assert_eq!(bytes[4], 0x1F); // port high byte
    assert_eq!(bytes[5], 0x90); // port low byte
}

// ─── Error cases ─────────────────────────────────────────────────────────────

#[test]
fn test_target_addr_from_bytes_too_short() {
    let result = TargetAddr::from_bytes(&[0x00]);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("too short"), "Got: {err}");
}

#[test]
fn test_target_addr_from_bytes_empty() {
    let result = TargetAddr::from_bytes(&[]);
    assert!(result.is_err());
}

#[test]
fn test_target_addr_from_bytes_truncated_host() {
    // Says host is 10 bytes but only provides 2
    let data = [0x00, 0x0A, b'A', b'B'];
    let result = TargetAddr::from_bytes(&data);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("truncated"), "Got: {err}");
}

#[test]
fn test_target_addr_from_bytes_truncated_port() {
    // Host is "AB" (2 bytes) but no port bytes after
    let data = [0x00, 0x02, b'A', b'B'];
    let result = TargetAddr::from_bytes(&data);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("truncated"), "Got: {err}");
}

#[test]
fn test_target_addr_from_bytes_invalid_utf8() {
    // host_len=2, host=[0xFF, 0xFE] (invalid UTF-8), port=[0x00, 0x50]
    let data = [0x00, 0x02, 0xFF, 0xFE, 0x00, 0x50];
    let result = TargetAddr::from_bytes(&data);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("UTF-8"), "Got: {err}");
}

// ─── Empty host ──────────────────────────────────────────────────────────────

#[test]
fn test_target_addr_empty_host() {
    let addr = TargetAddr {
        host: "".to_string(),
        port: 80,
    };
    let bytes = addr.to_bytes();
    // host_len=0, no host bytes, then port
    assert_eq!(bytes.len(), 4); // 2 (len) + 0 (host) + 2 (port)
    let parsed = TargetAddr::from_bytes(&bytes).expect("should parse empty host");
    assert_eq!(parsed.host, "");
    assert_eq!(parsed.port, 80);
}
