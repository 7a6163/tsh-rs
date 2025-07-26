use thiserror::Error;

pub type TshResult<T> = Result<T, TshError>;

#[derive(Error, Debug)]
pub enum TshError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Encryption error: {0}")]
    Encryption(String),

    #[error("Authentication failed")]
    AuthenticationFailed,

    #[error("Invalid challenge")]
    InvalidChallenge,

    #[error("Connection closed")]
    ConnectionClosed,

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Timeout occurred")]
    Timeout,

    #[error("Invalid operation mode: {0}")]
    InvalidOperationMode(u8),

    #[error("PTY error: {0}")]
    Pty(String),

    #[error("File transfer error: {0}")]
    FileTransfer(String),

    #[error("System error: {0}")]
    System(String),
}

impl TshError {
    pub fn network<S: Into<String>>(msg: S) -> Self {
        TshError::Network(msg.into())
    }

    pub fn encryption<S: Into<String>>(msg: S) -> Self {
        TshError::Encryption(msg.into())
    }

    pub fn protocol<S: Into<String>>(msg: S) -> Self {
        TshError::Protocol(msg.into())
    }

    pub fn pty<S: Into<String>>(msg: S) -> Self {
        TshError::Pty(msg.into())
    }

    pub fn file_transfer<S: Into<String>>(msg: S) -> Self {
        TshError::FileTransfer(msg.into())
    }

    pub fn system<S: Into<String>>(msg: S) -> Self {
        TshError::System(msg.into())
    }
}
