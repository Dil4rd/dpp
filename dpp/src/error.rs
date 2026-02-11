use thiserror::Error;

#[derive(Error, Debug)]
pub enum DppError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("DMG error: {0}")]
    Dmg(#[from] udif::DppError),

    #[error("HFS+ error: {0}")]
    Hfs(#[from] hfsplus::HfsPlusError),

    #[error("XAR error: {0}")]
    Xar(#[from] xara::XarError),

    #[error("PBZX error: {0}")]
    Pbzx(#[from] pbzx::PbzxError),

    #[error("APFS error: {0}")]
    Apfs(#[from] apfs::ApfsError),

    #[error("file not found: {0}")]
    FileNotFound(String),

    #[error("no HFS+ partition found in DMG")]
    NoHfsPartition,

    #[error("no APFS partition found in DMG")]
    NoApfsPartition,

    #[error("no filesystem partition found in DMG")]
    NoFilesystemPartition,
}

pub type Result<T> = std::result::Result<T, DppError>;
