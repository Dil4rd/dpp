//! PBZX format definitions and constants.
//!
//! # PBZX Format Structure
//!
//! PBZX (Payload BZX) is Apple's streaming compression format used in macOS
//! software updates and installer packages. The format consists of:
//!
//! ```text
//! +------------------+
//! | Header (12 bytes)|
//! +------------------+
//! | Chunk 1          |
//! +------------------+
//! | Chunk 2          |
//! +------------------+
//! | ...              |
//! +------------------+
//! | Chunk N          |
//! +------------------+
//! ```
//!
//! ## Header
//! - 4 bytes: Magic ("pbzx")
//! - 8 bytes: Flags (big-endian u64)
//!
//! ## Chunk
//! - 8 bytes: Uncompressed size (big-endian u64)
//! - 8 bytes: Compressed size (big-endian u64)
//! - N bytes: Compressed data (XZ/LZMA stream)
//!
//! When compressed_size == uncompressed_size, the data is stored uncompressed.
//!
//! The decompressed output is typically a CPIO archive containing the payload files.

/// PBZX magic bytes: "pbzx"
pub const PBZX_MAGIC: [u8; 4] = [0x70, 0x62, 0x7a, 0x78];

/// XZ magic bytes for validation
pub const XZ_MAGIC: [u8; 6] = [0xfd, 0x37, 0x7a, 0x58, 0x5a, 0x00];

/// Size of the PBZX header (magic + flags)
pub const HEADER_SIZE: usize = 12;

/// Size of a chunk header (uncompressed_size + compressed_size)
pub const CHUNK_HEADER_SIZE: usize = 16;

/// CPIO magic for "newc" format (ASCII, SVR4 with no CRC)
pub const CPIO_MAGIC_NEWC: &[u8; 6] = b"070701";

/// CPIO magic for "crc" format (ASCII, SVR4 with CRC)
pub const CPIO_MAGIC_CRC: &[u8; 6] = b"070702";

/// CPIO magic for "odc" format (ASCII, POSIX.1 portable)
pub const CPIO_MAGIC_ODC: &[u8; 6] = b"070707";

/// CPIO trailer filename
pub const CPIO_TRAILER: &str = "TRAILER!!!";

/// CPIO format variant
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpioFormat {
    /// SVR4 newc format (070701)
    Newc,
    /// SVR4 crc format (070702)
    Crc,
    /// POSIX.1 odc format (070707)
    Odc,
}

/// PBZX file header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PbzxHeader {
    /// Magic bytes (should be "pbzx")
    pub magic: [u8; 4],
    /// Flags field (purpose varies by version)
    pub flags: u64,
}

impl PbzxHeader {
    /// Check if the header has valid magic bytes.
    pub fn is_valid(&self) -> bool {
        self.magic == PBZX_MAGIC
    }
}

/// A chunk within the PBZX archive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkHeader {
    /// Size of data after decompression
    pub uncompressed_size: u64,
    /// Size of compressed data in the archive
    pub compressed_size: u64,
}

impl ChunkHeader {
    /// Check if this chunk's data is stored without compression.
    pub fn is_uncompressed(&self) -> bool {
        self.compressed_size == self.uncompressed_size
    }

    /// Check if this is likely the end marker (both sizes are 0).
    pub fn is_end_marker(&self) -> bool {
        self.uncompressed_size == 0 && self.compressed_size == 0
    }
}

/// CPIO entry header (newc format).
#[derive(Debug, Clone)]
pub struct CpioHeader {
    /// Inode number
    pub ino: u32,
    /// File mode and permissions
    pub mode: u32,
    /// User ID
    pub uid: u32,
    /// Group ID
    pub gid: u32,
    /// Number of hard links
    pub nlink: u32,
    /// Modification time
    pub mtime: u32,
    /// File size
    pub filesize: u32,
    /// Device major number (for device files)
    pub devmajor: u32,
    /// Device minor number (for device files)
    pub devminor: u32,
    /// Rdev major (for special files)
    pub rdevmajor: u32,
    /// Rdev minor (for special files)
    pub rdevminor: u32,
    /// Length of filename (including null terminator)
    pub namesize: u32,
    /// Checksum (only used in CRC format)
    pub check: u32,
    /// Filename
    pub name: String,
}

impl CpioHeader {
    /// Size of the fixed portion of a CPIO newc header (in bytes).
    pub const HEADER_SIZE: usize = 110;

    /// Check if this entry is a regular file.
    pub fn is_file(&self) -> bool {
        (self.mode & 0o170000) == 0o100000
    }

    /// Check if this entry is a directory.
    pub fn is_directory(&self) -> bool {
        (self.mode & 0o170000) == 0o040000
    }

    /// Check if this entry is a symbolic link.
    pub fn is_symlink(&self) -> bool {
        (self.mode & 0o170000) == 0o120000
    }

    /// Check if this is the trailer entry.
    pub fn is_trailer(&self) -> bool {
        self.name == CPIO_TRAILER
    }

    /// Get the permission bits.
    pub fn permissions(&self) -> u32 {
        self.mode & 0o7777
    }

    /// Get the file type as a human-readable string.
    pub fn file_type(&self) -> &'static str {
        match self.mode & 0o170000 {
            0o100000 => "file",
            0o040000 => "directory",
            0o120000 => "symlink",
            0o060000 => "block device",
            0o020000 => "character device",
            0o010000 => "fifo",
            0o140000 => "socket",
            _ => "unknown",
        }
    }
}

/// File entry information for listing.
#[derive(Debug, Clone)]
pub struct FileEntry {
    /// Path within the archive
    pub path: String,
    /// File size in bytes
    pub size: u64,
    /// File mode/permissions
    pub mode: u32,
    /// Modification time (Unix timestamp)
    pub mtime: u32,
    /// User ID
    pub uid: u32,
    /// Group ID
    pub gid: u32,
    /// Whether this is a directory
    pub is_dir: bool,
    /// Whether this is a symlink
    pub is_symlink: bool,
    /// Symlink target (if is_symlink)
    pub link_target: Option<String>,
}

impl FileEntry {
    /// Get a formatted mode string (like ls -l).
    pub fn mode_string(&self) -> String {
        let file_type = if self.is_dir {
            'd'
        } else if self.is_symlink {
            'l'
        } else {
            '-'
        };

        let mode = self.mode & 0o7777;
        let mut s = String::with_capacity(10);
        s.push(file_type);

        for shift in [6, 3, 0] {
            let bits = (mode >> shift) & 0o7;
            s.push(if bits & 0o4 != 0 { 'r' } else { '-' });
            s.push(if bits & 0o2 != 0 { 'w' } else { '-' });
            s.push(if bits & 0o1 != 0 { 'x' } else { '-' });
        }

        s
    }
}
