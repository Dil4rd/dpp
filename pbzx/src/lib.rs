//! # pbzx
//!
//! A Rust library for parsing, extracting, and creating PBZX archives.
//!
//! PBZX (Payload BZX) is Apple's streaming compression format used in macOS
//! software updates and installer packages (.pkg files). The format wraps
//! XZ-compressed chunks of a CPIO archive.
//!
//! ## Features
//!
//! - **`list`** (default): List files in PBZX archives
//! - **`extract`** (default): Extract files from PBZX archives
//! - **`pack`** (default): Create new PBZX archives
//!
//! ## Quick Start
//!
//! ### Reading a PBZX archive
//!
//! ```no_run
//! use pbzx::{open, Archive};
//!
//! // Open and decompress
//! let mut reader = pbzx::open("Payload").unwrap();
//! let cpio_data = reader.decompress().unwrap();
//!
//! // Parse the CPIO content
//! let mut archive = Archive::from_cpio(&cpio_data).unwrap();
//!
//! // List all files
//! for entry in archive.list().unwrap() {
//!     println!("{}: {} bytes", entry.path, entry.size);
//! }
//! ```
//!
//! ### Extracting files
//!
//! ```no_run
//! use pbzx::Archive;
//!
//! let mut archive = Archive::open("Payload").unwrap();
//!
//! // Extract a single file
//! let data = archive.extract_file("path/to/file.txt").unwrap();
//!
//! // Extract all files to a directory
//! archive.extract_all("output_dir").unwrap();
//! ```
//!
//! ### Creating a PBZX archive
//!
//! ```no_run
//! use pbzx::writer::{CpioBuilder, PbzxWriter};
//! use std::fs::File;
//!
//! // Build CPIO content
//! let mut cpio = CpioBuilder::new();
//! cpio.add_file("hello.txt", b"Hello, World!", 0o644);
//! cpio.add_directory("subdir", 0o755);
//! let cpio_data = cpio.finish();
//!
//! // Write PBZX archive
//! let file = File::create("output.pbzx").unwrap();
//! let mut writer = PbzxWriter::new(file);
//! writer.write_cpio(&cpio_data).unwrap();
//! writer.finish().unwrap();
//! ```
//!
//! ## Format Details
//!
//! A PBZX file consists of:
//!
//! 1. **Header** (12 bytes)
//!    - Magic: `pbzx` (4 bytes)
//!    - Flags: Big-endian u64 (8 bytes)
//!
//! 2. **Chunks** (repeated)
//!    - Uncompressed size: Big-endian u64 (8 bytes)
//!    - Compressed size: Big-endian u64 (8 bytes)
//!    - Data: XZ-compressed payload
//!
//! The concatenated decompressed chunks form a CPIO archive containing
//! the actual payload files.

pub mod cpio;
pub mod error;
pub mod format;
pub mod reader;
pub mod writer;

// Re-exports for convenience
pub use cpio::{CpioEntry, CpioReader};
pub use error::{PbzxError, Result};
pub use format::{ChunkHeader, CpioHeader, FileEntry, PbzxHeader};
pub use reader::{is_pbzx, open, ChunkInfo, PbzxReader};
pub use writer::{CpioBuilder, PbzxWriter};

use std::fs::File;
use std::io::{BufReader, Cursor, Read};
use std::path::Path;

/// High-level interface for working with PBZX archives.
///
/// This struct provides a convenient API for common operations like
/// listing and extracting files without needing to manually handle
/// the PBZX/CPIO layers.
///
/// # Example
///
/// ```no_run
/// use pbzx::Archive;
///
/// let mut archive = Archive::open("Payload").unwrap();
///
/// // List files
/// for entry in archive.list().unwrap() {
///     println!("{}", entry.path);
/// }
///
/// // Extract everything
/// archive.extract_all("output").unwrap();
/// ```
pub struct Archive {
    cpio_data: Vec<u8>,
}

impl Archive {
    /// Open a PBZX archive from a file path.
    ///
    /// When the `parallel` feature is enabled, XZ chunks are decompressed
    /// across multiple threads for significantly faster extraction.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut reader = open(path)?;

        #[cfg(feature = "parallel")]
        let cpio_data = reader.decompress_parallel()?;
        #[cfg(not(feature = "parallel"))]
        let cpio_data = reader.decompress()?;

        Ok(Self { cpio_data })
    }

    /// Create an Archive from raw CPIO data.
    ///
    /// Use this if you've already decompressed the PBZX data.
    pub fn from_cpio(data: &[u8]) -> Result<Self> {
        Ok(Self {
            cpio_data: data.to_vec(),
        })
    }

    /// Create an Archive from a reader containing PBZX data.
    ///
    /// When the `parallel` feature is enabled, XZ chunks are decompressed
    /// across multiple threads for significantly faster extraction.
    pub fn from_reader<R: Read>(reader: R) -> Result<Self> {
        let mut pbzx = PbzxReader::new(reader)?;

        #[cfg(feature = "parallel")]
        let cpio_data = pbzx.decompress_parallel()?;
        #[cfg(not(feature = "parallel"))]
        let cpio_data = pbzx.decompress()?;

        Ok(Self { cpio_data })
    }

    /// List all files in the archive.
    #[cfg(feature = "list")]
    pub fn list(&self) -> Result<Vec<FileEntry>> {
        let cursor = Cursor::new(&self.cpio_data);
        let mut cpio = CpioReader::new(cursor);
        cpio.list()
    }

    /// Extract a single file by path.
    #[cfg(feature = "extract")]
    pub fn extract_file(&self, path: &str) -> Result<Vec<u8>> {
        let cursor = Cursor::new(&self.cpio_data);
        let mut cpio = CpioReader::new(cursor);
        cpio.extract_file(path)
    }

    /// Extract all files to a directory.
    #[cfg(feature = "extract")]
    pub fn extract_all<P: AsRef<Path>>(&self, dest: P) -> Result<Vec<std::path::PathBuf>> {
        let cursor = Cursor::new(&self.cpio_data);
        let mut cpio = CpioReader::new(cursor);
        cpio.extract_all(dest)
    }

    /// Get all entries with their data.
    ///
    /// Note: This loads all file data into memory. For large archives,
    /// consider using `list()` first to identify files of interest,
    /// then `extract_file()` for specific files.
    pub fn entries(&self) -> Result<Vec<CpioEntry>> {
        let cursor = Cursor::new(&self.cpio_data);
        let mut cpio = CpioReader::new(cursor);
        let mut entries = Vec::new();
        for entry in cpio.entries()? {
            entries.push(entry?);
        }
        Ok(entries)
    }

    /// Get the raw CPIO data.
    pub fn cpio_data(&self) -> &[u8] {
        &self.cpio_data
    }

    /// Get the size of the decompressed CPIO data.
    pub fn decompressed_size(&self) -> usize {
        self.cpio_data.len()
    }
}

/// Statistics about a PBZX archive.
#[derive(Debug, Clone)]
pub struct ArchiveStats {
    /// Number of chunks in the archive
    pub chunk_count: usize,
    /// Total compressed size (excluding headers)
    pub compressed_size: u64,
    /// Total uncompressed size
    pub uncompressed_size: u64,
    /// Number of files in the CPIO payload
    pub file_count: usize,
    /// Number of directories in the CPIO payload
    pub directory_count: usize,
    /// Total size of all files
    pub total_file_size: u64,
}

impl ArchiveStats {
    /// Calculate the overall compression ratio.
    pub fn compression_ratio(&self) -> f64 {
        if self.uncompressed_size == 0 {
            1.0
        } else {
            self.compressed_size as f64 / self.uncompressed_size as f64
        }
    }

    /// Get the compression ratio as a percentage saved.
    pub fn space_savings(&self) -> f64 {
        (1.0 - self.compression_ratio()) * 100.0
    }
}

/// Get statistics about a PBZX archive.
pub fn stats<P: AsRef<Path>>(path: P) -> Result<ArchiveStats> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut pbzx = PbzxReader::new(reader)?;

    // Get chunk info
    let chunks = pbzx.chunk_info()?;
    let chunk_count = chunks.len();
    let compressed_size: u64 = chunks.iter().map(|c| c.compressed_size).sum();
    let uncompressed_size: u64 = chunks.iter().map(|c| c.uncompressed_size).sum();

    // Decompress and analyze CPIO
    pbzx.reset()?;

    #[cfg(feature = "parallel")]
    let cpio_data = pbzx.decompress_parallel()?;
    #[cfg(not(feature = "parallel"))]
    let cpio_data = pbzx.decompress()?;

    let cursor = Cursor::new(&cpio_data);
    let mut cpio = CpioReader::new(cursor);
    let entries = cpio.list()?;

    let file_count = entries.iter().filter(|e| !e.is_dir).count();
    let directory_count = entries.iter().filter(|e| e.is_dir).count();
    let total_file_size: u64 = entries.iter().filter(|e| !e.is_dir).map(|e| e.size).sum();

    Ok(ArchiveStats {
        chunk_count,
        compressed_size,
        uncompressed_size,
        file_count,
        directory_count,
        total_file_size,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpio_roundtrip() {
        // Create a CPIO archive
        let mut builder = CpioBuilder::new();
        builder.add_file("test.txt", b"Hello, World!", 0o644);
        builder.add_directory("subdir", 0o755);
        builder.add_file("subdir/nested.txt", b"Nested content", 0o644);
        let cpio_data = builder.finish();

        // Parse it back
        let cursor = Cursor::new(&cpio_data);
        let mut reader = CpioReader::new(cursor);
        let entries = reader.list().unwrap();

        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].path, "test.txt");
        assert_eq!(entries[0].size, 13);
        assert!(!entries[0].is_dir);

        assert_eq!(entries[1].path, "subdir");
        assert!(entries[1].is_dir);

        assert_eq!(entries[2].path, "subdir/nested.txt");
    }

    #[test]
    fn test_pbzx_roundtrip() {
        // Create CPIO content
        let mut builder = CpioBuilder::new();
        builder.add_file("hello.txt", b"Hello, PBZX!", 0o644);
        let cpio_data = builder.finish();

        // Write to PBZX
        let mut output = Vec::new();
        let mut writer = PbzxWriter::new(&mut output).compression_level(0);
        writer.write_cpio(&cpio_data).unwrap();
        writer.finish().unwrap();

        // Read it back
        let cursor = Cursor::new(&output);
        let mut reader = PbzxReader::new(cursor).unwrap();
        let decompressed = reader.decompress().unwrap();

        // Verify content
        assert_eq!(decompressed, cpio_data);
    }
}
