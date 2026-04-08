use crate::persistence::{load_config, PersistConfig};
use std::fs;

// ─── PersistConfig serialization ─────────────────────────────────────────────

#[test]
fn test_persist_config_serialize_with_connect_back() {
    let config = PersistConfig {
        psk: "my-secret-key".to_string(),
        port: 4444,
        connect_back_host: Some("10.0.0.1".to_string()),
        delay: 30,
    };
    let json = config.to_json_string().expect("should serialize");
    assert!(json.contains("my-secret-key"));
    assert!(json.contains("4444"));
    assert!(json.contains("10.0.0.1"));
    assert!(json.contains("30"));
}

#[test]
fn test_persist_config_serialize_listen_mode() {
    let config = PersistConfig {
        psk: "listen-key".to_string(),
        port: 1234,
        connect_back_host: None,
        delay: 5,
    };
    let json = config.to_json_string().expect("should serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed["connect_back_host"].is_null());
}

#[test]
fn test_persist_config_round_trip() {
    let original = PersistConfig {
        psk: "round-trip-key".to_string(),
        port: 8080,
        connect_back_host: Some("192.168.1.100".to_string()),
        delay: 60,
    };
    let json = original.to_json_string().expect("should serialize");
    let parsed = PersistConfig::from_json_str(&json).expect("should deserialize");
    assert_eq!(parsed.psk, original.psk);
    assert_eq!(parsed.port, original.port);
    assert_eq!(parsed.connect_back_host, original.connect_back_host);
    assert_eq!(parsed.delay, original.delay);
}

#[test]
fn test_persist_config_round_trip_none_host() {
    let original = PersistConfig {
        psk: "no-host".to_string(),
        port: 5555,
        connect_back_host: None,
        delay: 10,
    };
    let json = original.to_json_string().expect("should serialize");
    let parsed = PersistConfig::from_json_str(&json).expect("should deserialize");
    assert_eq!(parsed.connect_back_host, None);
}

// ─── load_config ─────────────────────────────────────────────────────────────

#[test]
fn test_load_config_from_file() {
    let dir = std::env::temp_dir().join("tsh_test_persist");
    let _ = fs::create_dir_all(&dir);
    let path = dir.join("test_config.json");

    let config = PersistConfig {
        psk: "file-test-key".to_string(),
        port: 9999,
        connect_back_host: Some("10.10.10.10".to_string()),
        delay: 15,
    };
    let json = config.to_json_string().unwrap();
    fs::write(&path, &json).unwrap();

    let loaded = load_config(path.to_str().unwrap()).expect("should load config");
    assert_eq!(loaded.psk, "file-test-key");
    assert_eq!(loaded.port, 9999);
    assert_eq!(loaded.connect_back_host.as_deref(), Some("10.10.10.10"));
    assert_eq!(loaded.delay, 15);

    // Cleanup
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_load_config_missing_file() {
    let result = load_config("/tmp/tsh_nonexistent_config_12345.json");
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("Failed to read config file"), "Got: {err}");
}

#[test]
fn test_load_config_invalid_json() {
    let dir = std::env::temp_dir().join("tsh_test_persist_invalid");
    let _ = fs::create_dir_all(&dir);
    let path = dir.join("bad.json");
    fs::write(&path, "not valid json{{{").unwrap();

    let result = load_config(path.to_str().unwrap());
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("Failed to parse config"), "Got: {err}");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_load_config_missing_field() {
    let dir = std::env::temp_dir().join("tsh_test_persist_missing");
    let _ = fs::create_dir_all(&dir);
    let path = dir.join("partial.json");
    // Missing "delay" field
    fs::write(&path, r#"{"psk":"key","port":1234}"#).unwrap();

    let result = load_config(path.to_str().unwrap());
    assert!(result.is_err(), "should fail with missing field");

    let _ = fs::remove_dir_all(&dir);
}
