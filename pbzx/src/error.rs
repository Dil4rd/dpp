//! Error types for PBZX operations.

use std::io;
use thiserror::Error;

/// Errors that can occur during PBZX operations.
#[derive(Error, Debug)]
pub enum PbzxError {
    /// Invalid magic bytes - not a PBZX file
    #[error("Invalid PBZX magic: expected 'pbzx', got {0:?}")]
    InvalidMagic([u8; 4]),

    /// I/O error during read/write operations
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// XZ/LZMA decompression error
    #[error("Decompression error: {0}")]
    Decompression(String),

    /// Invalid chunk header
    #[error("Invalid chunk at offset {offset}: {message}")]
    InvalidChunk { offset: u64, message: String },

    /// Unexpected end of file
    #[error("Unexpected end of file at offset {0}")]
    UnexpectedEof(u64),

    /// Invalid CPIO archive
    #[error("Invalid CPIO archive: {0}")]
    InvalidCpio(String),

    /// File not found in archive
    #[error("File not found: {0}")]
    FileNotFound(String),

    /// Invalid path (e.g., path traversal attempt)
    #[error("Invalid path: {0}")]
    InvalidPath(String),

    /// Compression error during packing
    #[error("Compression error: {0}")]
    Compression(String),

    /// Unsupported feature or format variant
    #[error("Unsupported: {0}")]
    Unsupported(String),
}

/// Result type alias for PBZX operations.
pub type Result<T> = std::result::Result<T, PbzxError>;
