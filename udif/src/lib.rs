//! DPP - DMG + PKG + PBZX parser
//!
//! A cross-platform library for working with Apple disk images (DMG files).
//!
//! # Features
//!
//! - **List** partitions in DMG files
//! - **Extract** raw partition data
//! - **Create** DMG files with various compression methods
//! - **Cross-platform** - works on Windows, Linux, and macOS
//!
//! # Supported Compression
//!
//! - Raw (uncompressed)
//! - Zlib
//! - Bzip2
//! - LZFSE (Apple's native compression)
//! - LZVN
//!
//! # Example
//!
//! ```no_run
//! use udif::{DmgArchive, Result};
//!
//! fn main() -> Result<()> {
//!     // Open a DMG file
//!     let mut archive = DmgArchive::open("image.dmg")?;
//!
//!     // List partitions
//!     for partition in archive.partitions() {
//!         println!("{}: {} sectors", partition.name, partition.sectors);
//!     }
//!
//!     // Extract main partition
//!     let data = archive.extract_main_partition()?;
//!     std::fs::write("partition.raw", &data)?;
//!
//!     Ok(())
//! }
//! ```

pub mod checksum;
pub mod error;
pub mod format;
pub mod reader;
pub mod writer;

pub use checksum::{crc32, CHECKSUM_TYPE_CRC32, CHECKSUM_TYPE_NONE};
pub use error::{DppError, Result};
pub use format::{BlockType, KolyHeader, MishHeader, PartitionEntry};
pub use reader::{open, is_dmg, CompressionInfo, DmgReader, DmgReaderOptions, DmgStats};
pub use writer::{create, create_from_data, create_from_file, CompressionMethod, DmgWriter};

/// Partition filesystem type detected from the partition name
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartitionType {
    /// HFS+ (case-insensitive)
    Hfs,
    /// HFSX (case-sensitive HFS+)
    Hfsx,
    /// Apple APFS
    Apfs,
    /// Other or unknown partition type
    Other,
}

impl PartitionType {
    /// Classify a partition from its DMG partition name (e.g. "Apple_HFSX")
    pub fn from_partition_name(name: &str) -> Self {
        if name.contains("Apple_HFSX") {
            PartitionType::Hfsx
        } else if name.contains("Apple_HFS") {
            PartitionType::Hfs
        } else if name.contains("Apple_APFS") {
            PartitionType::Apfs
        } else {
            PartitionType::Other
        }
    }

    /// Returns `true` if this partition can be parsed as HFS+
    pub fn is_hfs_compatible(&self) -> bool {
        matches!(self, PartitionType::Hfs | PartitionType::Hfsx)
    }
}

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

/// High-level DMG archive interface
pub struct DmgArchive {
    reader: DmgReader<BufReader<File>>,
}

/// Partition information
#[derive(Debug, Clone)]
pub struct PartitionInfo {
    /// Partition name
    pub name: String,
    /// Partition ID
    pub id: i32,
    /// Number of sectors (512 bytes each)
    pub sectors: u64,
    /// Uncompressed size in bytes
    pub size: u64,
    /// Compressed size in bytes
    pub compressed_size: u64,
    /// Filesystem type detected from partition name
    pub partition_type: PartitionType,
}

impl DmgArchive {
    /// Open a DMG file with default options (checksum verification enabled)
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let reader = DmgReader::open(path)?;
        Ok(DmgArchive { reader })
    }

    /// Open a DMG file with custom options
    pub fn open_with_options<P: AsRef<Path>>(
        path: P,
        options: reader::DmgReaderOptions,
    ) -> Result<Self> {
        let reader = DmgReader::open_with_options(path, options)?;
        Ok(DmgArchive { reader })
    }

    /// Get archive statistics
    pub fn stats(&self) -> DmgStats {
        self.reader.stats()
    }

    /// Get compression info
    pub fn compression_info(&self) -> CompressionInfo {
        self.reader.compression_info()
    }

    /// List all partitions
    pub fn partitions(&self) -> Vec<PartitionInfo> {
        self.reader
            .partitions()
            .iter()
            .map(|p| PartitionInfo {
                name: p.name.clone(),
                id: p.id,
                sectors: p.block_map.sector_count,
                size: p.block_map.uncompressed_size(),
                compressed_size: p.block_map.compressed_size(),
                partition_type: PartitionType::from_partition_name(&p.name),
            })
            .collect()
    }

    /// Get partition by name
    pub fn partition(&self, name: &str) -> Option<PartitionInfo> {
        self.reader.partition(name).map(|p| PartitionInfo {
            name: p.name.clone(),
            id: p.id,
            sectors: p.block_map.sector_count,
            size: p.block_map.uncompressed_size(),
            compressed_size: p.block_map.compressed_size(),
            partition_type: PartitionType::from_partition_name(&p.name),
        })
    }

    /// Extract a partition by ID
    pub fn extract_partition(&mut self, id: i32) -> Result<Vec<u8>> {
        self.reader.decompress_partition(id)
    }

    /// Extract a partition by name
    pub fn extract_partition_by_name(&mut self, name: &str) -> Result<Vec<u8>> {
        let partition = self
            .reader
            .partition(name)
            .ok_or_else(|| DppError::FileNotFound(name.to_string()))?;
        self.reader.decompress_partition(partition.id)
    }

    /// Extract the main HFS+/APFS partition
    pub fn extract_main_partition(&mut self) -> Result<Vec<u8>> {
        self.reader.decompress_main_partition()
    }

    /// Extract all partitions as a raw disk image
    pub fn extract_all(&mut self) -> Result<Vec<u8>> {
        self.reader.decompress_all()
    }

    /// Stream a partition to a writer block-by-block (low memory usage)
    pub fn extract_partition_to<W: std::io::Write>(
        &mut self,
        id: i32,
        writer: &mut W,
    ) -> Result<u64> {
        self.reader.decompress_partition_to(id, writer)
    }

    /// Stream the main HFS+/APFS partition to a writer (low memory usage)
    pub fn extract_main_partition_to<W: std::io::Write>(
        &mut self,
        writer: &mut W,
    ) -> Result<u64> {
        self.reader.decompress_main_partition_to(writer)
    }

    /// Get the ID of the main HFS+/APFS partition
    pub fn main_partition_id(&self) -> Result<i32> {
        self.reader.main_partition_id()
    }

    /// Get the ID of the main HFS+/HFSX partition (excludes APFS).
    /// Returns `Err(FileNotFound)` if no HFS-compatible partition exists.
    pub fn hfs_partition_id(&self) -> Result<i32> {
        self.reader.hfs_partition_id()
    }

    /// Extract a partition to a file
    pub fn extract_partition_to_file<P: AsRef<Path>>(&mut self, id: i32, path: P) -> Result<()> {
        let mut file = File::create(path)?;
        self.reader.decompress_partition_to(id, &mut file)?;
        Ok(())
    }

    /// Extract main partition to a file
    pub fn extract_main_partition_to_file<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        let mut file = File::create(path)?;
        self.reader.decompress_main_partition_to(&mut file)?;
        Ok(())
    }

    /// Get the raw koly header
    pub fn koly(&self) -> &KolyHeader {
        self.reader.koly()
    }
}

/// Builder for creating DMG files
pub struct DmgBuilder {
    compression: CompressionMethod,
    compression_level: u32,
    chunk_size: usize,
    partitions: Vec<(String, Vec<u8>)>,
    skip_checksums: bool,
}

impl Default for DmgBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl DmgBuilder {
    /// Create a new DMG builder
    pub fn new() -> Self {
        DmgBuilder {
            compression: CompressionMethod::Zlib,
            compression_level: 6,
            chunk_size: 1024 * 1024,
            partitions: Vec::new(),
            skip_checksums: false,
        }
    }

    /// Set compression method
    pub fn compression(mut self, method: CompressionMethod) -> Self {
        self.compression = method;
        self
    }

    /// Set compression level (0-9)
    pub fn compression_level(mut self, level: u32) -> Self {
        self.compression_level = level;
        self
    }

    /// Set chunk size
    pub fn chunk_size(mut self, size: usize) -> Self {
        self.chunk_size = size;
        self
    }

    /// Skip checksum generation for faster DMG creation
    pub fn skip_checksums(mut self, skip: bool) -> Self {
        self.skip_checksums = skip;
        self
    }

    /// Add a partition
    pub fn add_partition(mut self, name: &str, data: Vec<u8>) -> Self {
        self.partitions.push((name.to_string(), data));
        self
    }

    /// Build and write the DMG to a file
    pub fn build<P: AsRef<Path>>(self, path: P) -> Result<()> {
        let mut writer = DmgWriter::create(path)?
            .compression(self.compression)
            .compression_level(self.compression_level)
            .chunk_size(self.chunk_size)
            .skip_checksums(self.skip_checksums);

        for (name, data) in self.partitions {
            writer.add_partition(&name, &data)?;
        }

        writer.finish()
    }
}

/// Quick check if a file is a valid DMG
pub fn check_dmg<P: AsRef<Path>>(path: P) -> bool {
    is_dmg(path)
}

/// Get statistics about a DMG file
pub fn stats<P: AsRef<Path>>(path: P) -> Result<DmgStats> {
    let reader = DmgReader::open(path)?;
    Ok(reader.stats())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_block_type_conversion() {
        assert_eq!(BlockType::try_from(0x00000000).unwrap(), BlockType::ZeroFill);
        assert_eq!(BlockType::try_from(0x80000005).unwrap(), BlockType::Zlib);
        assert_eq!(BlockType::try_from(0x80000006).unwrap(), BlockType::Bzip2);
        assert_eq!(BlockType::try_from(0x80000007).unwrap(), BlockType::Lzfse);
        assert_eq!(BlockType::try_from(0x80000008).unwrap(), BlockType::Lzvn);
        assert_eq!(BlockType::try_from(0xFFFFFFFF).unwrap(), BlockType::End);
        assert_eq!(BlockType::try_from(0x7FFFFFFE).unwrap(), BlockType::Comment);

        // Unknown block type should error
        assert!(BlockType::try_from(0x12345678).is_err());
    }

    #[test]
    fn test_compression_method() {
        assert_eq!(CompressionMethod::default(), CompressionMethod::Zlib);
    }

    // =========================================================================
    // TRICKY PIECE #1: Koly header must be exactly 512 bytes at -512 from EOF
    // =========================================================================
    #[test]
    fn test_koly_header_size_is_512() {
        use crate::format::{KolyHeader, KOLY_MAGIC, KOLY_SIZE};

        assert_eq!(KOLY_SIZE, 512, "KOLY_SIZE constant must be 512");

        // Create a koly header and serialize it
        let koly = KolyHeader {
            magic: *KOLY_MAGIC,
            version: 4,
            header_size: 512,
            flags: 1,
            running_data_fork_offset: 0,
            data_fork_offset: 0,
            data_fork_length: 1000,
            rsrc_fork_offset: 0,
            rsrc_fork_length: 0,
            segment_number: 1,
            segment_count: 1,
            segment_id: [0u8; 16],
            data_checksum_type: 2,
            data_checksum_size: 32,
            data_checksum: [0u8; 128],
            plist_offset: 1000,
            plist_length: 500,
            reserved: [0u8; 64],
            master_checksum_type: 2,
            master_checksum_size: 32,
            master_checksum: [0u8; 128],
            image_variant: 1,
            sector_count: 100,
        };

        let mut buf = Vec::new();
        koly.write(&mut buf).unwrap();

        assert_eq!(buf.len(), 512, "Koly header serialization must be exactly 512 bytes");
    }

    #[test]
    fn test_koly_magic_position() {
        use crate::format::{KolyHeader, KOLY_MAGIC};

        // Create a minimal DMG-like structure
        let mut dmg_data = vec![0u8; 1024]; // Some data

        // Append plist placeholder
        let plist = b"<?xml version=\"1.0\"?><plist></plist>";
        let plist_offset = dmg_data.len() as u64;
        dmg_data.extend_from_slice(plist);
        let plist_length = plist.len() as u64;

        // Create and append koly header
        let koly = KolyHeader {
            magic: *KOLY_MAGIC,
            version: 4,
            header_size: 512,
            flags: 1,
            running_data_fork_offset: 0,
            data_fork_offset: 0,
            data_fork_length: plist_offset,
            rsrc_fork_offset: 0,
            rsrc_fork_length: 0,
            segment_number: 1,
            segment_count: 1,
            segment_id: [0u8; 16],
            data_checksum_type: 2,
            data_checksum_size: 32,
            data_checksum: [0u8; 128],
            plist_offset,
            plist_length,
            reserved: [0u8; 64],
            master_checksum_type: 2,
            master_checksum_size: 32,
            master_checksum: [0u8; 128],
            image_variant: 1,
            sector_count: 2,
        };
        koly.write(&mut dmg_data).unwrap();

        // Verify koly magic is at exactly -512 from end
        let total_len = dmg_data.len();
        let koly_start = total_len - 512;
        assert_eq!(&dmg_data[koly_start..koly_start + 4], b"koly");
    }

    // =========================================================================
    // TRICKY PIECE #2: Mish header actual_block_count is at offset 200, not 36
    // =========================================================================
    #[test]
    fn test_mish_block_count_at_offset_200() {
        use crate::format::{MishHeader, MISH_MAGIC};
        use byteorder::{BigEndian, WriteBytesExt};

        // Create a mish header manually with known values
        let mut mish_data = Vec::new();

        // Header (204 bytes)
        mish_data.extend_from_slice(MISH_MAGIC);           // 0-3: magic
        mish_data.write_u32::<BigEndian>(1).unwrap();      // 4-7: version
        mish_data.write_u64::<BigEndian>(0).unwrap();      // 8-15: first_sector
        mish_data.write_u64::<BigEndian>(10).unwrap();     // 16-23: sector_count
        mish_data.write_u64::<BigEndian>(0).unwrap();      // 24-31: data_offset
        mish_data.write_u32::<BigEndian>(0).unwrap();      // 32-35: buffers_needed
        mish_data.write_u32::<BigEndian>(999).unwrap();    // 36-39: WRONG block count (should be ignored)
        mish_data.extend_from_slice(&[0u8; 24]);           // 40-63: reserved
        mish_data.write_u32::<BigEndian>(2).unwrap();      // 64-67: checksum_type
        mish_data.write_u32::<BigEndian>(32).unwrap();     // 68-71: checksum_size
        mish_data.extend_from_slice(&[0u8; 128]);          // 72-199: checksum
        mish_data.write_u32::<BigEndian>(2).unwrap();      // 200-203: ACTUAL block count

        // Add 2 block runs (40 bytes each)
        // Block 0: ZeroFill
        mish_data.write_u32::<BigEndian>(0x00000000).unwrap(); // type
        mish_data.write_u32::<BigEndian>(0).unwrap();          // comment
        mish_data.write_u64::<BigEndian>(0).unwrap();          // sector_number
        mish_data.write_u64::<BigEndian>(10).unwrap();         // sector_count
        mish_data.write_u64::<BigEndian>(0).unwrap();          // compressed_offset
        mish_data.write_u64::<BigEndian>(0).unwrap();          // compressed_length

        // Block 1: End marker
        mish_data.write_u32::<BigEndian>(0xFFFFFFFF).unwrap(); // type
        mish_data.write_u32::<BigEndian>(0).unwrap();
        mish_data.write_u64::<BigEndian>(10).unwrap();
        mish_data.write_u64::<BigEndian>(0).unwrap();
        mish_data.write_u64::<BigEndian>(0).unwrap();
        mish_data.write_u64::<BigEndian>(0).unwrap();

        // Parse
        let mish = MishHeader::from_bytes(&mish_data).unwrap();

        // Should have 2 block runs (from offset 200), not 999 (from offset 36)
        assert_eq!(mish.block_runs.len(), 2);
        assert_eq!(mish.actual_block_count, 2);
        assert_eq!(mish.block_descriptor_count, 999); // This field is at 36 but not used for counting
    }

    #[test]
    fn test_mish_header_size_is_204() {
        use crate::format::{MishHeader, MISH_MAGIC};
        use byteorder::{BigEndian, WriteBytesExt};

        // Build minimal mish header
        let mut mish_data = Vec::new();
        mish_data.extend_from_slice(MISH_MAGIC);
        mish_data.write_u32::<BigEndian>(1).unwrap();
        mish_data.write_u64::<BigEndian>(0).unwrap();
        mish_data.write_u64::<BigEndian>(1).unwrap();
        mish_data.write_u64::<BigEndian>(0).unwrap();
        mish_data.write_u32::<BigEndian>(0).unwrap();
        mish_data.write_u32::<BigEndian>(0).unwrap();
        mish_data.extend_from_slice(&[0u8; 24]);
        mish_data.write_u32::<BigEndian>(2).unwrap();
        mish_data.write_u32::<BigEndian>(32).unwrap();
        mish_data.extend_from_slice(&[0u8; 128]);
        mish_data.write_u32::<BigEndian>(0).unwrap(); // actual_block_count = 0

        // Header should be exactly 204 bytes
        assert_eq!(mish_data.len(), 204);

        // Should parse successfully with 0 block runs
        let mish = MishHeader::from_bytes(&mish_data).unwrap();
        assert_eq!(mish.block_runs.len(), 0);
    }

    // =========================================================================
    // TRICKY PIECE #3: LZFSE decoder needs buffer larger than output size
    // =========================================================================
    #[test]
    fn test_lzfse_needs_larger_buffer() {
        // Compress some test data with LZFSE
        let original = b"Hello, World! This is a test of LZFSE compression. ".repeat(10);

        let mut compressed = vec![0u8; original.len() + 4096];
        let compressed_len = lzfse::encode_buffer(&original, &mut compressed).unwrap();
        compressed.truncate(compressed_len);

        // Try to decompress with exact-sized buffer - should fail
        let mut exact_buf = vec![0u8; original.len()];
        let result = lzfse::decode_buffer(&compressed, &mut exact_buf);

        // This might fail with BufferTooSmall
        if result.is_err() {
            // Retry with 2x buffer - should work
            let mut large_buf = vec![0u8; original.len() * 2];
            let decoded_len = lzfse::decode_buffer(&compressed, &mut large_buf).unwrap();
            assert_eq!(decoded_len, original.len());
            assert_eq!(&large_buf[..decoded_len], &original[..]);
        }
    }

    // =========================================================================
    // TRICKY PIECE #4: Zlib decompressed size might be less than sector_count * 512
    // =========================================================================
    #[test]
    fn test_zlib_partial_sector() {
        use flate2::write::ZlibEncoder;
        use flate2::read::ZlibDecoder;
        use flate2::Compression;
        use std::io::{Read, Write};

        // Create data that's not sector-aligned (100 bytes, not multiple of 512)
        let original = b"This is test data that is not aligned to 512-byte sectors!";

        // Compress it
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(original).unwrap();
        let compressed = encoder.finish().unwrap();

        // Try to decompress into a full sector buffer (512 bytes)
        let mut sector_buf = vec![0u8; 512];
        let mut decoder = ZlibDecoder::new(&compressed[..]);

        // read() should return actual bytes, not error
        let bytes_read = decoder.read(&mut sector_buf).unwrap();

        assert_eq!(bytes_read, original.len());
        assert_eq!(&sector_buf[..bytes_read], &original[..]);

        // Rest of buffer should be zeros
        assert!(sector_buf[bytes_read..].iter().all(|&b| b == 0));
    }

    // =========================================================================
    // TRICKY PIECE #5: DMG roundtrip with various data sizes
    // =========================================================================
    #[test]
    fn test_roundtrip_sector_aligned() {
        // Test with sector-aligned data (1024 bytes = 2 sectors)
        let original = vec![0x42u8; 1024];

        let mut dmg_buf = Vec::new();
        {
            let mut writer = DmgWriter::new(Cursor::new(&mut dmg_buf));
            writer.add_partition("test", &original).unwrap();
            writer.finish().unwrap();
        }

        // Read back
        let mut reader = DmgReader::new(Cursor::new(&dmg_buf)).unwrap();
        let extracted = reader.decompress_partition(0).unwrap();

        // Should match (might be padded to sector boundary)
        assert!(extracted.len() >= original.len());
        assert_eq!(&extracted[..original.len()], &original[..]);
    }

    #[test]
    fn test_roundtrip_non_sector_aligned() {
        // Test with non-sector-aligned data (100 bytes)
        let original = b"Short test data that is not sector aligned".to_vec();

        let mut dmg_buf = Vec::new();
        {
            let mut writer = DmgWriter::new(Cursor::new(&mut dmg_buf));
            writer.add_partition("test", &original).unwrap();
            writer.finish().unwrap();
        }

        // Read back
        let mut reader = DmgReader::new(Cursor::new(&dmg_buf)).unwrap();
        let extracted = reader.decompress_partition(0).unwrap();

        // First bytes should match original
        assert!(extracted.len() >= original.len());
        assert_eq!(&extracted[..original.len()], &original[..]);
    }

    #[test]
    fn test_roundtrip_empty_data() {
        // Edge case: empty data
        let original: Vec<u8> = vec![];

        let mut dmg_buf = Vec::new();
        {
            let mut writer = DmgWriter::new(Cursor::new(&mut dmg_buf));
            writer.add_partition("empty", &original).unwrap();
            writer.finish().unwrap();
        }

        let reader = DmgReader::new(Cursor::new(&dmg_buf)).unwrap();
        let partitions = reader.partitions();
        assert_eq!(partitions.len(), 1);
    }

    #[test]
    fn test_roundtrip_zeros() {
        // Test that zero-filled blocks are handled correctly
        let original = vec![0u8; 2048]; // 4 sectors of zeros

        let mut dmg_buf = Vec::new();
        {
            let mut writer = DmgWriter::new(Cursor::new(&mut dmg_buf));
            writer.add_partition("zeros", &original).unwrap();
            writer.finish().unwrap();
        }

        let mut reader = DmgReader::new(Cursor::new(&dmg_buf)).unwrap();
        let extracted = reader.decompress_partition(0).unwrap();

        assert_eq!(extracted.len(), original.len());
        assert!(extracted.iter().all(|&b| b == 0));
    }

    // =========================================================================
    // TRICKY PIECE #6: Block run structure is exactly 40 bytes
    // =========================================================================
    #[test]
    fn test_block_run_size() {
        use crate::format::{BlockRun, BlockType};

        let block_run = BlockRun {
            block_type: BlockType::Zlib,
            comment: 0,
            sector_number: 100,
            sector_count: 50,
            compressed_offset: 1000,
            compressed_length: 500,
        };

        let bytes = block_run.to_bytes();
        assert_eq!(bytes.len(), 40, "Block run must be exactly 40 bytes");

        // Verify round-trip
        let parsed = BlockRun::from_bytes(&bytes).unwrap();
        assert_eq!(parsed.block_type, BlockType::Zlib);
        assert_eq!(parsed.sector_number, 100);
        assert_eq!(parsed.sector_count, 50);
        assert_eq!(parsed.compressed_offset, 1000);
        assert_eq!(parsed.compressed_length, 500);
    }

    // =========================================================================
    // TRICKY PIECE #7: is_dmg checks magic at -512, not -4
    // =========================================================================
    #[test]
    fn test_is_dmg_checks_correct_offset() {
        use crate::format::is_dmg;

        // Create a file that has "koly" at -4 but not at -512 (should NOT be valid)
        let mut fake_dmg = vec![0u8; 600];
        // Put "koly" at the wrong place (-4 from end)
        let len = fake_dmg.len();
        fake_dmg[len - 4..].copy_from_slice(b"koly");

        let mut cursor = Cursor::new(&fake_dmg);
        assert!(!is_dmg(&mut cursor), "Should not detect koly at wrong offset");

        // Create a file that has "koly" at -512 (should be valid)
        let mut real_dmg = vec![0u8; 600];
        let len = real_dmg.len();
        real_dmg[len - 512..len - 508].copy_from_slice(b"koly");

        let mut cursor = Cursor::new(&real_dmg);
        assert!(is_dmg(&mut cursor), "Should detect koly at correct offset");
    }

    // =========================================================================
    // TRICKY PIECE #8: Different compression methods
    // =========================================================================
    #[test]
    fn test_compression_methods() {
        let original = b"Test data for compression testing. ".repeat(100);

        for method in [
            CompressionMethod::Raw,
            CompressionMethod::Zlib,
            CompressionMethod::Bzip2,
            // LZFSE tested separately due to buffer quirks
        ] {
            let mut dmg_buf = Vec::new();
            {
                let mut writer = DmgWriter::new(Cursor::new(&mut dmg_buf))
                    .compression(method);
                writer.add_partition("test", &original).unwrap();
                writer.finish().unwrap();
            }

            let mut reader = DmgReader::new(Cursor::new(&dmg_buf)).unwrap();
            let extracted = reader.decompress_partition(0).unwrap();

            assert!(
                extracted.len() >= original.len(),
                "Extracted should be at least as large as original for {:?}",
                method
            );
            assert_eq!(
                &extracted[..original.len()],
                &original[..],
                "Data mismatch for {:?}",
                method
            );
        }
    }

    #[test]
    fn test_lzfse_compression_roundtrip() {
        let original = b"LZFSE compression test data. ".repeat(100);

        let mut dmg_buf = Vec::new();
        {
            let mut writer = DmgWriter::new(Cursor::new(&mut dmg_buf))
                .compression(CompressionMethod::Lzfse);
            writer.add_partition("test", &original).unwrap();
            writer.finish().unwrap();
        }

        let mut reader = DmgReader::new(Cursor::new(&dmg_buf)).unwrap();
        let extracted = reader.decompress_partition(0).unwrap();

        assert!(extracted.len() >= original.len());
        assert_eq!(&extracted[..original.len()], &original[..]);
    }

    // =========================================================================
    // Integration test with real DMG file (requires fixture)
    // =========================================================================

    /// Requires ../tests/Kernel_Debug_Kit_26.3_build_25D5087f.dmg fixture.
    /// Run with `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn test_real_dmg_if_available() {
        let test_dmg = "../tests/Kernel_Debug_Kit_26.3_build_25D5087f.dmg";

        let archive = DmgArchive::open(test_dmg).unwrap();
        let stats = archive.stats();

        assert_eq!(stats.version, 4);
        assert!(stats.partition_count > 0);
        assert!(stats.total_uncompressed > stats.total_compressed);

        let partitions = archive.partitions();
        assert!(!partitions.is_empty());

        let hfsx = partitions.iter().find(|p| p.name.contains("HFSX"));
        assert!(hfsx.is_some(), "Should have HFSX partition");
    }

    /// Requires ../tests/Kernel_Debug_Kit_26.3_build_25D5087f.dmg fixture.
    /// Run with `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn test_real_dmg_decompress() {
        let test_dmg = "../tests/Kernel_Debug_Kit_26.3_build_25D5087f.dmg";

        let mut archive = DmgArchive::open(test_dmg).unwrap();
        let data = archive.extract_main_partition().unwrap();
        assert_eq!(&data[1024..1026], &[0x48, 0x58], "Should be HFSX signature");

        let mut archive2 = DmgArchive::open(test_dmg).unwrap();
        let mut buf = Vec::new();
        let _n = archive2.extract_main_partition_to(&mut buf).unwrap();
        assert_eq!(&buf[1024..1026], &[0x48, 0x58], "Should be HFSX signature");

        assert_eq!(data.len(), buf.len());
        assert_eq!(data, buf, "Buffered and streaming should produce identical output");
    }

    // =========================================================================
    // TRICKY PIECE #9: Checksum verification
    // =========================================================================
    #[test]
    fn test_checksum_verification_disabled() {
        // Create a DMG and verify we can read it with checksums disabled
        let original = b"Test data for checksum verification".repeat(20);

        let mut dmg_buf = Vec::new();
        {
            let mut writer = DmgWriter::new(Cursor::new(&mut dmg_buf));
            writer.add_partition("test", &original).unwrap();
            writer.finish().unwrap();
        }

        // Read with checksums disabled
        let options = reader::DmgReaderOptions {
            verify_checksums: false,
        };
        let mut reader = DmgReader::with_options(Cursor::new(&dmg_buf), options).unwrap();
        let extracted = reader.decompress_partition(0).unwrap();

        assert!(extracted.len() >= original.len());
        assert_eq!(&extracted[..original.len()], &original[..]);
    }

    #[test]
    fn test_checksum_verification_enabled_with_zero_checksums() {
        // Create a DMG (writer currently writes zero checksums)
        // This should still work because zero checksums are skipped
        let original = b"Test data for checksum verification".repeat(20);

        let mut dmg_buf = Vec::new();
        {
            let mut writer = DmgWriter::new(Cursor::new(&mut dmg_buf));
            writer.add_partition("test", &original).unwrap();
            writer.finish().unwrap();
        }

        // Read with checksums enabled (default)
        let mut reader = DmgReader::new(Cursor::new(&dmg_buf)).unwrap();
        let extracted = reader.decompress_partition(0).unwrap();

        assert!(extracted.len() >= original.len());
        assert_eq!(&extracted[..original.len()], &original[..]);
    }

    #[test]
    fn test_dmg_archive_with_options() {
        // Test that DmgArchive::open_with_options works
        let original = b"Test data".repeat(10);

        let mut dmg_buf = Vec::new();
        {
            let mut writer = DmgWriter::new(Cursor::new(&mut dmg_buf));
            writer.add_partition("test", &original).unwrap();
            writer.finish().unwrap();
        }

        // Write to temp file
        let temp_dir = tempfile::tempdir().unwrap();
        let temp_path = temp_dir.path().join("test.dmg");
        std::fs::write(&temp_path, &dmg_buf).unwrap();

        // Open with custom options
        let options = reader::DmgReaderOptions {
            verify_checksums: false,
        };
        let mut archive = DmgArchive::open_with_options(&temp_path, options).unwrap();
        let extracted = archive.extract_partition(0).unwrap();

        assert!(extracted.len() >= original.len());
        assert_eq!(&extracted[..original.len()], &original[..]);
    }

    // =========================================================================
    // TRICKY PIECE #10: Checksum roundtrip verification
    // =========================================================================
    #[test]
    fn test_checksum_roundtrip_with_verification() {
        // Test that checksums are written and verified correctly
        let original = b"Test data for checksum roundtrip verification. ".repeat(50);

        let mut dmg_buf = Vec::new();
        {
            let mut writer = DmgWriter::new(Cursor::new(&mut dmg_buf));
            writer.add_partition("test", &original).unwrap();
            writer.finish().unwrap();
        }

        // Read with checksum verification enabled (default)
        // This will fail if checksums don't match
        let mut reader = DmgReader::new(Cursor::new(&dmg_buf)).unwrap();
        let extracted = reader.decompress_partition(0).unwrap();

        assert!(extracted.len() >= original.len());
        assert_eq!(&extracted[..original.len()], &original[..]);

        // Verify checksums are non-zero in koly header
        let koly = reader.koly();
        assert_eq!(koly.data_checksum_type, 2); // CRC32
        assert_ne!(&koly.data_checksum[..4], &[0u8; 4]); // Non-zero checksum
        assert_eq!(koly.master_checksum_type, 2); // CRC32
        assert_ne!(&koly.master_checksum[..4], &[0u8; 4]); // Non-zero checksum
    }

    #[test]
    fn test_checksum_detection_corrupted_data() {
        // Test that corrupted data fork is detected
        let original = b"Test data for corruption detection".repeat(20);

        let mut dmg_buf = Vec::new();
        {
            let mut writer = DmgWriter::new(Cursor::new(&mut dmg_buf));
            writer.add_partition("test", &original).unwrap();
            writer.finish().unwrap();
        }

        // Corrupt the data fork (first 100 bytes)
        for i in 0..100 {
            dmg_buf[i] ^= 0xFF;
        }

        // Try to read with checksum verification - should fail
        let result = DmgReader::new(Cursor::new(&dmg_buf));
        assert!(result.is_err());
        if let Err(DppError::ChecksumMismatch { expected, actual }) = result {
            assert_ne!(expected, actual); // Checksums should differ
        } else {
            panic!("Expected ChecksumMismatch error");
        }
    }

    #[test]
    fn test_checksum_all_compression_methods() {
        // Test checksum verification with all compression methods
        let original = b"Testing checksums with all compressions! ".repeat(100);

        for method in [
            CompressionMethod::Raw,
            CompressionMethod::Zlib,
            CompressionMethod::Bzip2,
            CompressionMethod::Lzfse,
        ] {
            let mut dmg_buf = Vec::new();
            {
                let mut writer = DmgWriter::new(Cursor::new(&mut dmg_buf))
                    .compression(method);
                writer.add_partition("test", &original).unwrap();
                writer.finish().unwrap();
            }

            // Read with checksum verification
            let mut reader = DmgReader::new(Cursor::new(&dmg_buf))
                .unwrap_or_else(|e| panic!("Failed to open DMG with {:?}: {:?}", method, e));

            let extracted = reader.decompress_partition(0)
                .unwrap_or_else(|e| panic!("Failed to decompress with {:?}: {:?}", method, e));

            assert!(
                extracted.len() >= original.len(),
                "Extracted size mismatch for {:?}",
                method
            );
            assert_eq!(
                &extracted[..original.len()],
                &original[..],
                "Data mismatch for {:?}",
                method
            );
        }
    }
}
