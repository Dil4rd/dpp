use thiserror::Error;

#[derive(Error, Debug)]
pub enum XarError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid XAR magic: 0x{0:08X} (expected 0x78617221)")]
    InvalidMagic(u32),

    #[error("invalid TOC: {0}")]
    InvalidToc(String),

    #[error("XML parse error: {0}")]
    XmlParse(String),

    #[error("file not found: {0}")]
    FileNotFound(String),

    #[error("unsupported encoding: {0}")]
    UnsupportedEncoding(String),

    #[error("decompression failed: {0}")]
    DecompressionFailed(String),
}

pub type Result<T> = std::result::Result<T, XarError>;
