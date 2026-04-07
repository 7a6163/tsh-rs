use crate::constants::OperationMode;
use crate::error::{TshError, TshResult};

// ─── Error constructors ─────────────────────────────────────────────────────

#[test]
fn test_error_network() {
    let err = TshError::network("connection refused");
    assert_eq!(err.to_string(), "Network error: connection refused");
}

#[test]
fn test_error_encryption() {
    let err = TshError::encryption("bad key");
    assert_eq!(err.to_string(), "Encryption error: bad key");
}

#[test]
fn test_error_protocol() {
    let err = TshError::protocol("invalid message");
    assert_eq!(err.to_string(), "Protocol error: invalid message");
}

#[test]
fn test_error_pty() {
    let err = TshError::pty("failed to spawn");
    assert_eq!(err.to_string(), "PTY error: failed to spawn");
}

#[test]
fn test_error_file_transfer() {
    let err = TshError::file_transfer("file not found");
    assert_eq!(err.to_string(), "File transfer error: file not found");
}

#[test]
fn test_error_system() {
    let err = TshError::system("out of memory");
    assert_eq!(err.to_string(), "System error: out of memory");
}

#[test]
fn test_error_connection_closed() {
    let err = TshError::ConnectionClosed;
    assert_eq!(err.to_string(), "Connection closed");
}

#[test]
fn test_error_timeout() {
    let err = TshError::Timeout;
    assert_eq!(err.to_string(), "Timeout occurred");
}

#[test]
fn test_error_authentication_failed() {
    let err = TshError::AuthenticationFailed;
    assert_eq!(err.to_string(), "Authentication failed");
}

#[test]
fn test_error_invalid_operation_mode() {
    let err = TshError::InvalidOperationMode(99);
    assert_eq!(err.to_string(), "Invalid operation mode: 99");
}

#[test]
fn test_error_io_conversion() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "not found");
    let err: TshError = io_err.into();
    assert!(err.to_string().contains("not found"));
}

// ─── TshResult ──────────────────────────────────────────────────────────────

#[test]
fn test_tsh_result_ok() {
    let result: TshResult<i32> = Ok(42);
    assert!(result.is_ok());
}

#[test]
fn test_tsh_result_err() {
    let result: TshResult<i32> = Err(TshError::Timeout);
    assert!(result.is_err());
}

// ─── OperationMode ──────────────────────────────────────────────────────────

#[test]
fn test_operation_mode_to_u8() {
    assert_eq!(OperationMode::GetFile as u8, 1);
    assert_eq!(OperationMode::PutFile as u8, 2);
    assert_eq!(OperationMode::RunShell as u8, 3);
    assert_eq!(OperationMode::RunCommand as u8, 4);
}

#[test]
fn test_operation_mode_try_from_valid() {
    assert_eq!(OperationMode::try_from(1).unwrap(), OperationMode::GetFile);
    assert_eq!(OperationMode::try_from(2).unwrap(), OperationMode::PutFile);
    assert_eq!(OperationMode::try_from(3).unwrap(), OperationMode::RunShell);
    assert_eq!(
        OperationMode::try_from(4).unwrap(),
        OperationMode::RunCommand
    );
}

#[test]
fn test_operation_mode_try_from_invalid() {
    assert_eq!(OperationMode::try_from(0).unwrap_err(), 0);
    assert_eq!(OperationMode::try_from(5).unwrap_err(), 5);
    assert_eq!(OperationMode::try_from(255).unwrap_err(), 255);
}

#[test]
fn test_operation_mode_u8_round_trip() {
    for mode in [
        OperationMode::GetFile,
        OperationMode::PutFile,
        OperationMode::RunShell,
        OperationMode::RunCommand,
    ] {
        let byte: u8 = mode.into();
        let recovered = OperationMode::try_from(byte).unwrap();
        assert_eq!(recovered, mode);
    }
}

#[test]
fn test_operation_mode_debug() {
    let mode = OperationMode::RunShell;
    let debug = format!("{mode:?}");
    assert_eq!(debug, "RunShell");
}

#[test]
fn test_operation_mode_clone_copy() {
    let mode = OperationMode::GetFile;
    let cloned = mode;
    assert_eq!(mode, cloned);
}
