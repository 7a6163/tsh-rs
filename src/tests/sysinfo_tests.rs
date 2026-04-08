use crate::sysinfo::SystemInfo;

// ─── SystemInfo::collect ─────────────────────────────────────────────────────

#[test]
fn test_collect_returns_non_empty_fields() {
    let info = SystemInfo::collect();
    assert!(!info.hostname.is_empty(), "hostname should not be empty");
    assert!(!info.os.is_empty(), "os should not be empty");
    assert!(!info.arch.is_empty(), "arch should not be empty");
    assert!(!info.username.is_empty(), "username should not be empty");
    assert!(!info.home_dir.is_empty(), "home_dir should not be empty");
    assert!(
        !info.current_dir.is_empty(),
        "current_dir should not be empty"
    );
    assert!(info.pid > 0, "pid should be > 0");
}

#[test]
fn test_collect_os_contains_platform() {
    let info = SystemInfo::collect();
    let valid = ["macos", "linux", "windows"];
    assert!(
        valid.iter().any(|v| info.os.contains(v)),
        "os should contain a known platform, got: {}",
        info.os
    );
}

#[test]
fn test_collect_arch_is_known() {
    let info = SystemInfo::collect();
    let valid = ["x86_64", "aarch64", "x86", "arm"];
    assert!(
        valid.iter().any(|v| info.arch.contains(v)),
        "arch should be a known value, got: {}",
        info.arch
    );
}

// ─── Serialization round-trip ────────────────────────────────────────────────

#[test]
fn test_json_round_trip() {
    let info = SystemInfo::collect();
    let bytes = info.to_json_bytes();
    assert!(!bytes.is_empty(), "json bytes should not be empty");

    let parsed = SystemInfo::from_json_bytes(&bytes).expect("should parse back");
    assert_eq!(parsed.hostname, info.hostname);
    assert_eq!(parsed.os, info.os);
    assert_eq!(parsed.arch, info.arch);
    assert_eq!(parsed.username, info.username);
    assert_eq!(parsed.pid, info.pid);
    assert_eq!(parsed.is_elevated, info.is_elevated);
}

#[test]
fn test_json_bytes_are_valid_json() {
    let info = SystemInfo::collect();
    let bytes = info.to_json_bytes();
    let s = std::str::from_utf8(&bytes).expect("should be valid UTF-8");
    assert!(s.starts_with('{'));
    assert!(s.ends_with('}'));
    assert!(s.contains("\"hostname\""));
    assert!(s.contains("\"pid\""));
}

#[test]
fn test_from_json_bytes_known_json() {
    let json = r#"{"hostname":"test-host","os":"linux unix","arch":"x86_64","username":"user1","home_dir":"/home/user1","current_dir":"/tmp","pid":42,"is_elevated":false}"#;
    let info = SystemInfo::from_json_bytes(json.as_bytes()).expect("should parse");
    assert_eq!(info.hostname, "test-host");
    assert_eq!(info.pid, 42);
    assert!(!info.is_elevated);
}

#[test]
fn test_from_json_bytes_invalid() {
    assert!(SystemInfo::from_json_bytes(b"not json").is_none());
    assert!(SystemInfo::from_json_bytes(b"{}").is_none());
}

#[test]
fn test_json_escaping() {
    let info = SystemInfo {
        hostname: "host\"with\\quotes".to_string(),
        os: "linux unix".to_string(),
        arch: "x86_64".to_string(),
        username: "user\nnewline".to_string(),
        home_dir: "/home/test".to_string(),
        current_dir: "/tmp".to_string(),
        pid: 1,
        is_elevated: false,
    };
    let bytes = info.to_json_bytes();
    let parsed = SystemInfo::from_json_bytes(&bytes).expect("should survive escaping round-trip");
    assert_eq!(parsed.hostname, info.hostname);
    assert_eq!(parsed.username, info.username);
}

// ─── Display ─────────────────────────────────────────────────────────────────

#[test]
fn test_display_contains_all_fields() {
    let info = SystemInfo {
        hostname: "myhost".to_string(),
        os: "linux unix".to_string(),
        arch: "x86_64".to_string(),
        username: "root".to_string(),
        home_dir: "/root".to_string(),
        current_dir: "/tmp".to_string(),
        pid: 1234,
        is_elevated: true,
    };
    let output = info.display();
    assert!(output.contains("myhost"), "should contain hostname");
    assert!(output.contains("linux unix"), "should contain os");
    assert!(output.contains("x86_64"), "should contain arch");
    assert!(output.contains("root"), "should contain username");
    assert!(
        output.contains("root/admin"),
        "elevated user should show root/admin"
    );
    assert!(output.contains("1234"), "should contain pid");
}

#[test]
fn test_display_normal_user() {
    let info = SystemInfo {
        hostname: "laptop".to_string(),
        os: "macos unix".to_string(),
        arch: "aarch64".to_string(),
        username: "zac".to_string(),
        home_dir: "/Users/zac".to_string(),
        current_dir: "/Users/zac/dev".to_string(),
        pid: 5678,
        is_elevated: false,
    };
    let output = info.display();
    assert!(output.contains("normal"), "non-elevated should show normal");
    assert!(!output.contains("root/admin"));
}
