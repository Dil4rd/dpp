use thiserror::Error;

#[derive(Error, Debug)]
pub enum ApfsError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid magic: 0x{0:08X}")]
    InvalidMagic(u32),

    #[error("invalid checksum")]
    InvalidChecksum,

    #[error("invalid B-tree: {0}")]
    InvalidBTree(String),

    #[error("file not found: {0}")]
    FileNotFound(String),

    #[error("not a directory: {0}")]
    NotADirectory(String),

    #[error("corrupted data: {0}")]
    CorruptedData(String),

    #[error("no volume found in container")]
    NoVolume,
}

pub type Result<T> = std::result::Result<T, ApfsError>;
