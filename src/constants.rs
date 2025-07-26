/// Buffer size for data operations
pub const BUFSIZE: usize = 4096;

/// Operation modes
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OperationMode {
    GetFile = 1,
    PutFile = 2,
    RunShell = 3,
}

impl From<u8> for OperationMode {
    fn from(value: u8) -> Self {
        match value {
            1 => OperationMode::GetFile,
            2 => OperationMode::PutFile,
            3 => OperationMode::RunShell,
            _ => OperationMode::RunShell, // Default fallback
        }
    }
}

impl From<OperationMode> for u8 {
    fn from(mode: OperationMode) -> Self {
        mode as u8
    }
}

/// PEL (Packet Encryption Layer) status codes
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PelStatus {
    Success = 1,
    Failure = 0,
    SystemError = -1,
    ConnClosed = -2,
    WrongChallenge = -3,
    BadMsgLength = -4,
    CorruptedData = -5,
    UndefinedError = -6,
}

impl From<i8> for PelStatus {
    fn from(value: i8) -> Self {
        match value {
            1 => PelStatus::Success,
            0 => PelStatus::Failure,
            -1 => PelStatus::SystemError,
            -2 => PelStatus::ConnClosed,
            -3 => PelStatus::WrongChallenge,
            -4 => PelStatus::BadMsgLength,
            -5 => PelStatus::CorruptedData,
            _ => PelStatus::UndefinedError,
        }
    }
}

/// Handshake timeout in seconds
pub const HANDSHAKE_RW_TIMEOUT: u64 = 3;

/// Challenge bytes for authentication
pub const CHALLENGE: [u8; 16] = [
    0x58, 0x90, 0xAE, 0x86, 0xF1, 0xB9, 0x1C, 0xF6, 0x29, 0x83, 0x95, 0x71, 0x1D, 0xDE, 0x58, 0x0D,
];

/// Default port for connections
pub const DEFAULT_PORT: u16 = 1234;

/// Default secret for authentication
pub const DEFAULT_SECRET: &str = "1234";
