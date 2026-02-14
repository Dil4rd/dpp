//! PBZX archive writer/creator implementation.
//!
//! This module provides functionality to create new PBZX archives from files
//! or directories.

use std::fs::File;
use std::io::{BufWriter, Read, Write};
use std::path::Path;

use byteorder::{BigEndian, WriteBytesExt};
use xz2::write::XzEncoder;

use crate::error::{PbzxError, Result};
use crate::format::PBZX_MAGIC;

/// Default chunk size for compression (16 MB).
pub const DEFAULT_CHUNK_SIZE: usize = 16 * 1024 * 1024;

/// XZ compression preset (0-9, higher = better compression but slower).
pub const DEFAULT_COMPRESSION_LEVEL: u32 = 6;

/// Builder for creating PBZX archives.
///
/// # Example
///
/// ```no_run
/// use pbzx::PbzxWriter;
/// use std::fs::File;
///
/// let mut writer = PbzxWriter::new(File::create("output.pbzx").unwrap())
///     .chunk_size(8 * 1024 * 1024)
///     .compression_level(9);
///
/// // Write from CPIO data
/// let cpio_data = vec![/* CPIO archive data */];
/// writer.write_cpio(&cpio_data).unwrap();
/// writer.finish().unwrap();
/// ```
pub struct PbzxWriter<W> {
    writer: W,
    chunk_size: usize,
    compression_level: u32,
    flags: u64,
    header_written: bool,
    total_written: u64,
}

impl<W: Write> PbzxWriter<W> {
    /// Create a new PBZX writer.
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            chunk_size: DEFAULT_CHUNK_SIZE,
            compression_level: DEFAULT_COMPRESSION_LEVEL,
            flags: 0x0100000000000000, // Default flags (version 1)
            header_written: false,
            total_written: 0,
        }
    }

    /// Set the chunk size for compression.
    ///
    /// Larger chunks may compress better but use more memory.
    pub fn chunk_size(mut self, size: usize) -> Self {
        self.chunk_size = size;
        self
    }

    /// Set the XZ compression level (0-9).
    ///
    /// Higher levels produce smaller files but take longer.
    pub fn compression_level(mut self, level: u32) -> Self {
        self.compression_level = level.min(9);
        self
    }

    /// Set the flags field in the header.
    pub fn flags(mut self, flags: u64) -> Self {
        self.flags = flags;
        self
    }

    /// Write the PBZX header.
    fn write_header(&mut self) -> Result<()> {
        if self.header_written {
            return Ok(());
        }

        self.writer.write_all(&PBZX_MAGIC)?;
        self.writer.write_u64::<BigEndian>(self.flags)?;
        self.header_written = true;
        self.total_written += 12;

        Ok(())
    }

    /// Write a single chunk of data.
    fn write_chunk(&mut self, data: &[u8]) -> Result<()> {
        let uncompressed_size = data.len() as u64;

        // Compress the data
        let mut compressed = Vec::new();
        {
            let mut encoder = XzEncoder::new(&mut compressed, self.compression_level);
            encoder.write_all(data).map_err(|e| {
                PbzxError::Compression(format!("Failed to compress chunk: {}", e))
            })?;
            encoder.finish().map_err(|e| {
                PbzxError::Compression(format!("Failed to finish compression: {}", e))
            })?;
        }

        let compressed_size = compressed.len() as u64;

        // Write chunk header
        self.writer.write_u64::<BigEndian>(uncompressed_size)?;
        self.writer.write_u64::<BigEndian>(compressed_size)?;
        self.total_written += 16;

        // Write compressed data
        self.writer.write_all(&compressed)?;
        self.total_written += compressed_size;

        Ok(())
    }

    /// Write CPIO data to the archive, splitting into chunks.
    pub fn write_cpio(&mut self, data: &[u8]) -> Result<()> {
        self.write_header()?;

        // Split data into chunks and compress each
        for chunk in data.chunks(self.chunk_size) {
            self.write_chunk(chunk)?;
        }

        Ok(())
    }

    /// Write data from a reader, useful for large files.
    pub fn write_from_reader<R: Read>(&mut self, mut reader: R) -> Result<u64> {
        self.write_header()?;

        let mut total_read = 0u64;
        let mut buffer = vec![0u8; self.chunk_size];

        loop {
            let mut bytes_read = 0;
            // Fill the buffer
            while bytes_read < buffer.len() {
                match reader.read(&mut buffer[bytes_read..]) {
                    Ok(0) => break,
                    Ok(n) => bytes_read += n,
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(e) => return Err(e.into()),
                }
            }

            if bytes_read == 0 {
                break;
            }

            self.write_chunk(&buffer[..bytes_read])?;
            total_read += bytes_read as u64;
        }

        Ok(total_read)
    }

    /// Get the total bytes written so far.
    pub fn total_written(&self) -> u64 {
        self.total_written
    }

    /// Finish writing and return the inner writer.
    pub fn finish(mut self) -> Result<W> {
        if !self.header_written {
            self.write_header()?;
        }
        self.writer.flush()?;
        Ok(self.writer)
    }
}

/// CPIO archive builder for creating payloads.
///
/// This creates a CPIO archive in the newc format that can be wrapped
/// in a PBZX archive.
///
/// # Example
///
/// ```no_run
/// use pbzx::writer::CpioBuilder;
///
/// let mut builder = CpioBuilder::new();
/// builder.add_file("hello.txt", b"Hello, World!", 0o644);
/// builder.add_directory("subdir", 0o755);
/// builder.add_file("subdir/file.txt", b"Nested file", 0o644);
///
/// let cpio_data = builder.finish();
/// ```
pub struct CpioBuilder {
    data: Vec<u8>,
    inode_counter: u32,
}

impl CpioBuilder {
    /// Create a new CPIO builder.
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            inode_counter: 1,
        }
    }

    /// Add a regular file to the archive.
    pub fn add_file(&mut self, path: &str, content: &[u8], mode: u32) {
        self.add_entry(path, content, 0o100000 | (mode & 0o7777), content.len() as u32);
    }

    /// Add a directory to the archive.
    pub fn add_directory(&mut self, path: &str, mode: u32) {
        self.add_entry(path, &[], 0o040000 | (mode & 0o7777), 0);
    }

    /// Add a symbolic link to the archive.
    pub fn add_symlink(&mut self, path: &str, target: &str, mode: u32) {
        self.add_entry(
            path,
            target.as_bytes(),
            0o120000 | (mode & 0o7777),
            target.len() as u32,
        );
    }

    /// Add an entry to the CPIO archive.
    fn add_entry(&mut self, name: &str, data: &[u8], mode: u32, filesize: u32) {
        let namesize = name.len() + 1; // Include null terminator
        let inode = self.inode_counter;
        self.inode_counter += 1;

        // Write header in newc format (all fields as 8-char hex)
        write!(
            &mut self.data,
            "070701\
             {:08X}\
             {:08X}\
             {:08X}\
             {:08X}\
             {:08X}\
             {:08X}\
             {:08X}\
             {:08X}\
             {:08X}\
             {:08X}\
             {:08X}\
             {:08X}\
             {:08X}",
            inode,     // ino
            mode,      // mode
            0,         // uid
            0,         // gid
            1,         // nlink
            0,         // mtime
            filesize,  // filesize
            0,         // devmajor
            0,         // devminor
            0,         // rdevmajor
            0,         // rdevminor
            namesize,  // namesize
            0,         // check
        )
        .unwrap();

        // Write filename with null terminator
        self.data.extend_from_slice(name.as_bytes());
        self.data.push(0);

        // Pad to 4-byte boundary
        let header_len = 110 + namesize;
        let padding = (4 - (header_len % 4)) % 4;
        self.data.extend(std::iter::repeat_n(0, padding));

        // Write file data
        self.data.extend_from_slice(data);

        // Pad data to 4-byte boundary
        let data_padding = (4 - (data.len() % 4)) % 4;
        self.data.extend(std::iter::repeat_n(0, data_padding));
    }

    /// Finish the archive and return the CPIO data.
    pub fn finish(mut self) -> Vec<u8> {
        // Add trailer entry
        self.add_entry("TRAILER!!!", &[], 0, 0);
        self.data
    }

    /// Get the current size of the CPIO data.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if the builder is empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

impl Default for CpioBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a PBZX archive from a directory.
///
/// This recursively adds all files and directories to the archive.
///
/// # Example
///
/// ```no_run
/// use pbzx::writer::pack_directory;
///
/// pack_directory("input_dir", "output.pbzx", 6).unwrap();
/// ```
#[cfg(feature = "pack")]
pub fn pack_directory<P: AsRef<Path>, Q: AsRef<Path>>(
    source: P,
    dest: Q,
    compression_level: u32,
) -> Result<u64> {
    let source = source.as_ref();
    let dest = dest.as_ref();

    let mut builder = CpioBuilder::new();
    add_directory_to_cpio(&mut builder, source, "")?;

    let cpio_data = builder.finish();
    let file = File::create(dest)?;
    let writer = BufWriter::new(file);

    let mut pbzx = PbzxWriter::new(writer).compression_level(compression_level);
    pbzx.write_cpio(&cpio_data)?;
    pbzx.finish()?;

    Ok(cpio_data.len() as u64)
}

/// Recursively add a directory to a CPIO builder.
#[cfg(feature = "pack")]
fn add_directory_to_cpio(builder: &mut CpioBuilder, base: &Path, prefix: &str) -> Result<()> {
    for entry in std::fs::read_dir(base)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        let archive_path = if prefix.is_empty() {
            name_str.to_string()
        } else {
            format!("{}/{}", prefix, name_str)
        };

        let metadata = entry.metadata()?;

        if metadata.is_dir() {
            #[cfg(unix)]
            let mode = {
                use std::os::unix::fs::PermissionsExt;
                metadata.permissions().mode() & 0o7777
            };
            #[cfg(not(unix))]
            let mode = 0o755;

            builder.add_directory(&archive_path, mode);
            add_directory_to_cpio(builder, &path, &archive_path)?;
        } else if metadata.is_file() {
            let mut file = File::open(&path)?;
            let mut content = Vec::new();
            file.read_to_end(&mut content)?;

            #[cfg(unix)]
            let mode = {
                use std::os::unix::fs::PermissionsExt;
                metadata.permissions().mode() & 0o7777
            };
            #[cfg(not(unix))]
            let mode = 0o644;

            builder.add_file(&archive_path, &content, mode);
        } else if metadata.file_type().is_symlink() {
            #[cfg(unix)]
            {
                let target = std::fs::read_link(&path)?;
                let target_str = target.to_string_lossy();
                builder.add_symlink(&archive_path, &target_str, 0o777);
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpio_builder() {
        let mut builder = CpioBuilder::new();
        builder.add_file("test.txt", b"Hello, World!", 0o644);
        builder.add_directory("subdir", 0o755);
        let data = builder.finish();

        // Check magic
        assert_eq!(&data[0..6], b"070701");
        // Should end with trailer
        assert!(String::from_utf8_lossy(&data).contains("TRAILER!!!"));
    }

    #[test]
    fn test_pbzx_writer() {
        let mut output = Vec::new();
        let mut writer = PbzxWriter::new(&mut output)
            .chunk_size(1024)
            .compression_level(0);

        let test_data = b"Hello, PBZX World!";
        writer.write_cpio(test_data).unwrap();
        writer.finish().unwrap();

        // Check magic
        assert_eq!(&output[0..4], b"pbzx");
    }
}
