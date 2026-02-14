//! DMG writer implementation
//!
//! Provides creation of DMG disk images with various compression options.

use std::fs::File;
use std::io::{BufWriter, Seek, Write};
use std::path::Path;

use base64::Engine;
use byteorder::{BigEndian, WriteBytesExt};
use flate2::write::ZlibEncoder;
use flate2::Compression;

use crate::checksum::{create_checksum_array, crc32, CHECKSUM_TYPE_CRC32, CHECKSUM_TYPE_NONE};
use crate::error::{DppError, Result};
use crate::format::{BlockRun, BlockType, KolyHeader, KOLY_MAGIC, KOLY_SIZE, MISH_MAGIC};

/// Sector size in bytes
const SECTOR_SIZE: u64 = 512;

/// Default chunk size for compression (1 MB)
const DEFAULT_CHUNK_SIZE: usize = 1024 * 1024;

/// Compression method for DMG creation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CompressionMethod {
    /// No compression
    Raw,
    /// Zlib compression (default, best compatibility)
    #[default]
    Zlib,
    /// Bzip2 compression (better ratio, slower)
    Bzip2,
    /// LZFSE compression (fast, Apple-native)
    Lzfse,
}

impl CompressionMethod {
    fn block_type(&self) -> BlockType {
        match self {
            CompressionMethod::Raw => BlockType::Raw,
            CompressionMethod::Zlib => BlockType::Zlib,
            CompressionMethod::Bzip2 => BlockType::Bzip2,
            CompressionMethod::Lzfse => BlockType::Lzfse,
        }
    }
}

/// Builder for creating DMG files
pub struct DmgWriter<W> {
    writer: W,
    compression: CompressionMethod,
    compression_level: u32,
    chunk_size: usize,
    partitions: Vec<PartitionData>,
    current_offset: u64,
    /// Running CRC32 hasher for the data fork
    data_fork_hasher: crc32fast::Hasher,
    /// Skip checksum generation for faster DMG creation
    skip_checksums: bool,
}

struct PartitionData {
    name: String,
    id: i32,
    attributes: u32,
    first_sector: u64,
    sector_count: u64,
    block_runs: Vec<BlockRun>,
    checksum: [u8; 128],
}

impl<W: Write + Seek> DmgWriter<W> {
    /// Create a new DMG writer
    pub fn new(writer: W) -> Self {
        DmgWriter {
            writer,
            compression: CompressionMethod::Zlib,
            compression_level: 6,
            chunk_size: DEFAULT_CHUNK_SIZE,
            partitions: Vec::new(),
            current_offset: 0,
            data_fork_hasher: crc32fast::Hasher::new(),
            skip_checksums: false,
        }
    }

    /// Set compression method
    pub fn compression(mut self, method: CompressionMethod) -> Self {
        self.compression = method;
        self
    }

    /// Set compression level (0-9, only applies to zlib/bzip2)
    pub fn compression_level(mut self, level: u32) -> Self {
        self.compression_level = level.min(9);
        self
    }

    /// Set chunk size for compression
    pub fn chunk_size(mut self, size: usize) -> Self {
        self.chunk_size = size.max(4096);
        self
    }

    /// Skip checksum generation for faster DMG creation
    pub fn skip_checksums(mut self, skip: bool) -> Self {
        self.skip_checksums = skip;
        self
    }

    /// Add raw disk data as a partition
    pub fn add_partition(&mut self, name: &str, data: &[u8]) -> Result<()> {
        let sector_count = (data.len() as u64).div_ceil(SECTOR_SIZE);
        let first_sector = self.partitions.iter().map(|p| p.first_sector + p.sector_count).max().unwrap_or(0);

        let mut block_runs = Vec::new();
        let mut data_offset = 0usize;
        let mut sector_number = 0u64;

        // Calculate partition checksum (CRC32 of padded uncompressed data), unless skipping
        let partition_checksum = if self.skip_checksums {
            [0u8; 128]
        } else {
            let padded_size = (sector_count * SECTOR_SIZE) as usize;
            let mut padded_data = data.to_vec();
            padded_data.resize(padded_size, 0);
            create_checksum_array(crc32(&padded_data))
        };

        // Process data in chunks
        while data_offset < data.len() {
            let chunk_end = (data_offset + self.chunk_size).min(data.len());
            let chunk = &data[data_offset..chunk_end];
            let chunk_sectors = (chunk.len() as u64).div_ceil(SECTOR_SIZE).max(1);

            // Check if chunk is all zeros
            if chunk.iter().all(|&b| b == 0) {
                block_runs.push(BlockRun {
                    block_type: BlockType::ZeroFill,
                    comment: 0,
                    sector_number,
                    sector_count: chunk_sectors,
                    compressed_offset: 0,
                    compressed_length: 0,
                });
            } else {
                // Compress the chunk
                let compressed = self.compress_chunk(chunk)?;
                let compressed_offset = self.current_offset;
                let compressed_length = compressed.len() as u64;

                // Write compressed data and update data fork checksum
                self.writer.write_all(&compressed)?;
                self.data_fork_hasher.update(&compressed);
                self.current_offset += compressed_length;

                block_runs.push(BlockRun {
                    block_type: self.compression.block_type(),
                    comment: 0,
                    sector_number,
                    sector_count: chunk_sectors,
                    compressed_offset,
                    compressed_length,
                });
            }

            sector_number += chunk_sectors;
            data_offset = chunk_end;
        }

        // Add end marker
        block_runs.push(BlockRun {
            block_type: BlockType::End,
            comment: 0,
            sector_number,
            sector_count: 0,
            compressed_offset: 0,
            compressed_length: 0,
        });

        let id = self.partitions.len() as i32;
        self.partitions.push(PartitionData {
            name: name.to_string(),
            id,
            attributes: 0x0050,
            first_sector,
            sector_count,
            block_runs,
            checksum: partition_checksum,
        });

        Ok(())
    }

    /// Compress a chunk of data
    fn compress_chunk(&self, data: &[u8]) -> Result<Vec<u8>> {
        match self.compression {
            CompressionMethod::Raw => Ok(data.to_vec()),
            CompressionMethod::Zlib => {
                let mut encoder = ZlibEncoder::new(Vec::new(), Compression::new(self.compression_level));
                encoder.write_all(data)?;
                encoder.finish().map_err(|e| DppError::Compression(e.to_string()))
            }
            CompressionMethod::Bzip2 => {
                let mut encoder = bzip2::write::BzEncoder::new(
                    Vec::new(),
                    bzip2::Compression::new(self.compression_level),
                );
                encoder.write_all(data)?;
                encoder.finish().map_err(|e| DppError::Compression(e.to_string()))
            }
            CompressionMethod::Lzfse => {
                // Allocate output buffer with some extra space for overhead
                let mut output = vec![0u8; data.len() + 4096];
                let compressed_size = lzfse::encode_buffer(data, &mut output)
                    .map_err(|e| DppError::Compression(format!("LZFSE: {:?}", e)))?;
                output.truncate(compressed_size);
                Ok(output)
            }
        }
    }

    /// Finalize and write the DMG file
    pub fn finish(mut self) -> Result<()> {
        let data_fork_length = self.current_offset;
        let plist_offset = self.current_offset;

        // Calculate checksums unless skipping
        let (checksum_type, data_fork_checksum, master_checksum) = if self.skip_checksums {
            (CHECKSUM_TYPE_NONE, [0u8; 128], [0u8; 128])
        } else {
            // Calculate data fork checksum (clone hasher since finalize consumes it)
            let data_fork_checksum = create_checksum_array(self.data_fork_hasher.clone().finalize());

            // Calculate master checksum (CRC32 of all partition checksums concatenated)
            let mut master_data = Vec::new();
            for partition in &self.partitions {
                // Each mish checksum is the first 4 bytes
                master_data.extend_from_slice(&partition.checksum[..4]);
            }
            let master_checksum = create_checksum_array(crc32(&master_data));

            (CHECKSUM_TYPE_CRC32, data_fork_checksum, master_checksum)
        };

        // Generate plist
        let plist = self.generate_plist()?;
        self.writer.write_all(plist.as_bytes())?;
        let plist_length = plist.len() as u64;

        // Calculate total sector count
        let total_sectors: u64 = self.partitions.iter().map(|p| p.first_sector + p.sector_count).max().unwrap_or(0);

        // Generate and write koly header
        let koly = KolyHeader {
            magic: *KOLY_MAGIC,
            version: 4,
            header_size: KOLY_SIZE as u32,
            flags: 1,
            running_data_fork_offset: 0,
            data_fork_offset: 0,
            data_fork_length,
            rsrc_fork_offset: 0,
            rsrc_fork_length: 0,
            segment_number: 1,
            segment_count: 1,
            segment_id: [0u8; 16],
            data_checksum_type: checksum_type,
            data_checksum_size: 32,
            data_checksum: data_fork_checksum,
            plist_offset,
            plist_length,
            reserved: [0u8; 64],
            master_checksum_type: checksum_type,
            master_checksum_size: 32,
            master_checksum,
            image_variant: 1,
            sector_count: total_sectors,
        };

        koly.write(&mut self.writer)?;
        self.writer.flush()?;

        Ok(())
    }

    /// Generate the XML plist for the DMG
    fn generate_plist(&self) -> Result<String> {
        let mut plist = String::new();
        plist.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        plist.push_str("<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n");
        plist.push_str("<plist version=\"1.0\">\n");
        plist.push_str("<dict>\n");
        plist.push_str("\t<key>resource-fork</key>\n");
        plist.push_str("\t<dict>\n");
        plist.push_str("\t\t<key>blkx</key>\n");
        plist.push_str("\t\t<array>\n");

        for partition in &self.partitions {
            plist.push_str("\t\t\t<dict>\n");
            plist.push_str(&format!("\t\t\t\t<key>Attributes</key>\n\t\t\t\t<string>{:#06x}</string>\n", partition.attributes));
            plist.push_str(&format!("\t\t\t\t<key>CFName</key>\n\t\t\t\t<string>{}</string>\n", partition.name));

            // Generate mish data
            let mish_data = self.generate_mish(partition)?;
            let base64_data = base64::engine::general_purpose::STANDARD.encode(&mish_data);
            plist.push_str("\t\t\t\t<key>Data</key>\n");
            plist.push_str("\t\t\t\t<data>\n");

            // Split base64 into lines
            for chunk in base64_data.as_bytes().chunks(64) {
                plist.push_str("\t\t\t\t");
                plist.push_str(std::str::from_utf8(chunk).unwrap());
                plist.push('\n');
            }
            plist.push_str("\t\t\t\t</data>\n");

            plist.push_str(&format!("\t\t\t\t<key>ID</key>\n\t\t\t\t<string>{}</string>\n", partition.id));
            plist.push_str(&format!("\t\t\t\t<key>Name</key>\n\t\t\t\t<string>{}</string>\n", partition.name));
            plist.push_str("\t\t\t</dict>\n");
        }

        plist.push_str("\t\t</array>\n");
        plist.push_str("\t</dict>\n");
        plist.push_str("</dict>\n");
        plist.push_str("</plist>\n");

        Ok(plist)
    }

    /// Generate mish (block map) data for a partition
    fn generate_mish(&self, partition: &PartitionData) -> Result<Vec<u8>> {
        let mut data = Vec::new();

        // Mish header
        data.extend_from_slice(MISH_MAGIC);
        data.write_u32::<BigEndian>(1)?; // version
        data.write_u64::<BigEndian>(partition.first_sector)?;
        data.write_u64::<BigEndian>(partition.sector_count)?;
        data.write_u64::<BigEndian>(0)?; // data offset
        data.write_u32::<BigEndian>(0)?; // buffers needed
        data.write_u32::<BigEndian>(partition.block_runs.len() as u32)?;

        // Reserved (24 bytes)
        data.extend_from_slice(&[0u8; 24]);

        // Checksum
        data.write_u32::<BigEndian>(2)?; // checksum type (CRC32)
        data.write_u32::<BigEndian>(32)?; // checksum size
        data.extend_from_slice(&partition.checksum); // 128 bytes (ends at offset 199)

        // Actual block count at offset 200 (required for Apple DMG format)
        data.write_u32::<BigEndian>(partition.block_runs.len() as u32)?;

        // Block runs start at offset 204
        for block_run in &partition.block_runs {
            data.extend_from_slice(&block_run.to_bytes());
        }

        Ok(data)
    }
}

impl DmgWriter<BufWriter<File>> {
    /// Create a new DMG file
    pub fn create<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = File::create(path)?;
        let writer = BufWriter::new(file);
        Ok(Self::new(writer))
    }
}

/// Convenience function to create a DMG file
pub fn create<P: AsRef<Path>>(path: P) -> Result<DmgWriter<BufWriter<File>>> {
    DmgWriter::create(path)
}

/// Create a simple DMG from raw disk data
pub fn create_from_data<P: AsRef<Path>>(path: P, name: &str, data: &[u8]) -> Result<()> {
    let mut writer = create(path)?;
    writer.add_partition(name, data)?;
    writer.finish()
}

/// Create a simple DMG from a file
pub fn create_from_file<P: AsRef<Path>, Q: AsRef<Path>>(
    dmg_path: P,
    source_path: Q,
    partition_name: &str,
) -> Result<()> {
    let data = std::fs::read(source_path)?;
    create_from_data(dmg_path, partition_name, &data)
}
