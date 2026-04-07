/// Buffer size for data operations
pub const BUFSIZE: usize = 4096;

/// Operation modes
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OperationMode {
    GetFile = 1,
    PutFile = 2,
    RunShell = 3,
    RunCommand = 4,
}

impl TryFrom<u8> for OperationMode {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(OperationMode::GetFile),
            2 => Ok(OperationMode::PutFile),
            3 => Ok(OperationMode::RunShell),
            4 => Ok(OperationMode::RunCommand),
            other => Err(other),
        }
    }
}

impl From<OperationMode> for u8 {
    fn from(mode: OperationMode) -> Self {
        mode as u8
    }
}

/// Handshake timeout in seconds
pub const HANDSHAKE_RW_TIMEOUT: u64 = 3;

