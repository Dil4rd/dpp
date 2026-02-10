//! DMG reader implementation
//!
//! Provides streaming and full decompression of DMG disk images.

use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom, Write};
use std::path::Path;

use crate::checksum::{has_checksum, verify_crc32};
use crate::error::{DppError, Result};
use crate::format::{BlockType, KolyHeader, MishHeader, PartitionEntry};

/// Sector size in bytes
const SECTOR_SIZE: u64 = 512;

/// Read from a decoder until the buffer is full or EOF.
/// Unlike `read()`, this loops to handle decoders that return partial data.
fn read_full<R: Read>(reader: &mut R, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut total = 0;
    while total < buf.len() {
        match reader.read(&mut buf[total..])? {
            0 => break, // EOF
            n => total += n,
        }
    }
    Ok(total)
}

/// Options for DMG reader
#[derive(Debug, Clone)]
pub struct DmgReaderOptions {
    /// Whether to verify checksums when opening the DMG
    pub verify_checksums: bool,
}

impl Default for DmgReaderOptions {
    fn default() -> Self {
        Self {
            verify_checksums: true,
        }
    }
}

/// DMG reader for parsing and extracting disk images
pub struct DmgReader<R> {
    reader: R,
    koly: KolyHeader,
    partitions: Vec<PartitionEntry>,
    #[allow(dead_code)]
    options: DmgReaderOptions,
}

impl<R: Read + Seek> DmgReader<R> {
    /// Create a new DMG reader with default options (checksum verification enabled)
    pub fn new(reader: R) -> Result<Self> {
        Self::with_options(reader, DmgReaderOptions::default())
    }

    /// Create a new DMG reader with custom options
    pub fn with_options(mut reader: R, options: DmgReaderOptions) -> Result<Self> {
        // Read koly header
        let koly = KolyHeader::read(&mut reader)?;

        // Verify data fork checksum if enabled
        if options.verify_checksums {
            Self::verify_data_fork_checksum(&mut reader, &koly)?;
        }

        // Read and parse plist
        reader.seek(SeekFrom::Start(koly.plist_offset))?;
        let mut plist_data = vec![0u8; koly.plist_length as usize];
        reader.read_exact(&mut plist_data)?;

        let partitions = parse_plist(&plist_data)?;

        // Verify master checksum (CRC32 of all mish checksums)
        if options.verify_checksums {
            Self::verify_master_checksum(&koly, &partitions)?;
        }

        Ok(DmgReader {
            reader,
            koly,
            partitions,
            options,
        })
    }

    /// Verify the data fork checksum
    fn verify_data_fork_checksum(reader: &mut R, koly: &KolyHeader) -> Result<()> {
        // Skip if no checksum is set
        if !has_checksum(koly.data_checksum_type, &koly.data_checksum) {
            return Ok(());
        }

        // Read the data fork
        reader.seek(SeekFrom::Start(koly.data_fork_offset))?;
        let mut data_fork = vec![0u8; koly.data_fork_length as usize];
        reader.read_exact(&mut data_fork)?;

        // Verify checksum
        verify_crc32(koly.data_checksum_type, &koly.data_checksum, &data_fork).map_err(
            |(expected, actual)| DppError::ChecksumMismatch { expected, actual },
        )
    }

    /// Verify the master checksum (CRC32 of all mish checksums concatenated)
    fn verify_master_checksum(koly: &KolyHeader, partitions: &[PartitionEntry]) -> Result<()> {
        // Skip if no checksum is set
        if !has_checksum(koly.master_checksum_type, &koly.master_checksum) {
            return Ok(());
        }

        // Concatenate all mish checksums
        let mut all_checksums = Vec::new();
        for partition in partitions {
            // Each mish checksum is the first 4 bytes of the 128-byte array
            all_checksums.extend_from_slice(&partition.block_map.checksum[..4]);
        }

        // Verify master checksum
        verify_crc32(koly.master_checksum_type, &koly.master_checksum, &all_checksums).map_err(
            |(expected, actual)| DppError::ChecksumMismatch { expected, actual },
        )
    }

    /// Get the koly header
    pub fn koly(&self) -> &KolyHeader {
        &self.koly
    }

    /// Get all partitions
    pub fn partitions(&self) -> &[PartitionEntry] {
        &self.partitions
    }

    /// Get partition by name
    pub fn partition(&self, name: &str) -> Option<&PartitionEntry> {
        self.partitions.iter().find(|p| p.name == name)
    }

    /// List all partition names
    pub fn list_partitions(&self) -> Vec<&str> {
        self.partitions.iter().map(|p| p.name.as_str()).collect()
    }

    /// Get DMG statistics
    pub fn stats(&self) -> DmgStats {
        let total_sectors: u64 = self.partitions.iter().map(|p| p.block_map.sector_count).sum();
        let total_compressed: u64 = self.partitions.iter().map(|p| p.block_map.compressed_size()).sum();

        DmgStats {
            version: self.koly.version,
            sector_count: self.koly.sector_count,
            partition_count: self.partitions.len(),
            total_uncompressed: total_sectors * SECTOR_SIZE,
            total_compressed,
            data_fork_length: self.koly.data_fork_length,
        }
    }

    /// Decompress a specific partition to raw disk data
    pub fn decompress_partition(&mut self, partition_id: i32) -> Result<Vec<u8>> {
        let partition = self
            .partitions
            .iter()
            .find(|p| p.id == partition_id)
            .ok_or_else(|| DppError::FileNotFound(format!("partition {}", partition_id)))?
            .clone();

        let total_size = partition.block_map.sector_count * SECTOR_SIZE;
        let mut output = vec![0u8; total_size as usize];

        for block_run in &partition.block_map.block_runs {
            let out_offset = block_run.sector_number * SECTOR_SIZE;
            let out_size = block_run.sector_count * SECTOR_SIZE;

            match block_run.block_type {
                BlockType::ZeroFill => {
                    // Already zero-filled
                }
                BlockType::Raw | BlockType::Ignore => {
                    if block_run.compressed_length > 0 {
                        self.reader.seek(SeekFrom::Start(
                            self.koly.data_fork_offset + block_run.compressed_offset,
                        ))?;
                        // Read only compressed_length bytes (actual stored size)
                        // remaining bytes in the sector stay zero-filled
                        let end = out_offset as usize + block_run.compressed_length as usize;
                        self.reader.read_exact(&mut output[out_offset as usize..end])?;
                    }
                }
                BlockType::Zlib => {
                    self.reader.seek(SeekFrom::Start(
                        self.koly.data_fork_offset + block_run.compressed_offset,
                    ))?;
                    let mut compressed = vec![0u8; block_run.compressed_length as usize];
                    self.reader.read_exact(&mut compressed)?;

                    let mut decoder = flate2::read::ZlibDecoder::new(&compressed[..]);
                    let slice = &mut output[out_offset as usize..(out_offset + out_size) as usize];
                    read_full(&mut decoder, slice)?;
                }
                BlockType::Bzip2 => {
                    self.reader.seek(SeekFrom::Start(
                        self.koly.data_fork_offset + block_run.compressed_offset,
                    ))?;
                    let mut compressed = vec![0u8; block_run.compressed_length as usize];
                    self.reader.read_exact(&mut compressed)?;

                    let mut decoder = bzip2::read::BzDecoder::new(&compressed[..]);
                    let slice = &mut output[out_offset as usize..(out_offset + out_size) as usize];
                    read_full(&mut decoder, slice)?;
                }
                BlockType::Lzfse | BlockType::Lzvn => {
                    self.reader.seek(SeekFrom::Start(
                        self.koly.data_fork_offset + block_run.compressed_offset,
                    ))?;
                    let mut compressed = vec![0u8; block_run.compressed_length as usize];
                    self.reader.read_exact(&mut compressed)?;

                    // LZFSE decoder needs extra buffer space beyond the actual output size
                    // Allocate 2x the expected size to be safe
                    let expected_size = out_size as usize;
                    let mut temp_buf = vec![0u8; expected_size * 2];
                    let decoded_size = lzfse::decode_buffer(&compressed, &mut temp_buf)
                        .map_err(|e| DppError::Decompression(format!("LZFSE/LZVN: {:?}", e)))?;

                    // Copy only the expected amount to output
                    let copy_size = decoded_size.min(expected_size);
                    let end = out_offset as usize + copy_size;
                    output[out_offset as usize..end].copy_from_slice(&temp_buf[..copy_size]);
                }
                BlockType::Adc => {
                    return Err(DppError::Unsupported("ADC compression".into()));
                }
                BlockType::Comment | BlockType::End => {
                    // No data
                }
            }
        }

        Ok(output)
    }

    /// Decompress a partition and stream to a writer block-by-block.
    /// Only uses ~block_size memory per block instead of buffering the full partition.
    /// Integrity is ensured by koly checksums verified on open.
    /// Returns the total number of bytes written.
    pub fn decompress_partition_to<W: Write>(
        &mut self,
        partition_id: i32,
        writer: &mut W,
    ) -> Result<u64> {
        let partition = self
            .partitions
            .iter()
            .find(|p| p.id == partition_id)
            .ok_or_else(|| DppError::FileNotFound(format!("partition {}", partition_id)))?
            .clone();

        let block_size = partition.block_map.sector_count * SECTOR_SIZE;
        let mut bytes_written = 0u64;

        for block_run in &partition.block_map.block_runs {
            let out_offset = block_run.sector_number * SECTOR_SIZE;
            let out_size = block_run.sector_count * SECTOR_SIZE;

            // Emit zero padding if there's a gap between the current position and this block
            if out_offset > bytes_written {
                let gap = (out_offset - bytes_written) as usize;
                let zeros = vec![0u8; gap];
                writer.write_all(&zeros)?;
                bytes_written += gap as u64;
            }

            match block_run.block_type {
                BlockType::ZeroFill => {
                    let zeros = vec![0u8; out_size as usize];
                    writer.write_all(&zeros)?;
                    bytes_written += out_size;
                }
                BlockType::Raw | BlockType::Ignore => {
                    if block_run.compressed_length > 0 {
                        self.reader.seek(SeekFrom::Start(
                            self.koly.data_fork_offset + block_run.compressed_offset,
                        ))?;
                        let mut buf = vec![0u8; block_run.compressed_length as usize];
                        self.reader.read_exact(&mut buf)?;
                        writer.write_all(&buf)?;
                        bytes_written += block_run.compressed_length;
                        let remaining = out_size - block_run.compressed_length;
                        if remaining > 0 {
                            let zeros = vec![0u8; remaining as usize];
                            writer.write_all(&zeros)?;
                            bytes_written += remaining;
                        }
                    } else {
                        let zeros = vec![0u8; out_size as usize];
                        writer.write_all(&zeros)?;
                        bytes_written += out_size;
                    }
                }
                BlockType::Zlib => {
                    self.reader.seek(SeekFrom::Start(
                        self.koly.data_fork_offset + block_run.compressed_offset,
                    ))?;
                    let mut compressed = vec![0u8; block_run.compressed_length as usize];
                    self.reader.read_exact(&mut compressed)?;

                    let mut decoder = flate2::read::ZlibDecoder::new(&compressed[..]);
                    let mut decompressed = vec![0u8; out_size as usize];
                    read_full(&mut decoder, &mut decompressed)?;
                    writer.write_all(&decompressed)?;
                    bytes_written += out_size;
                }
                BlockType::Bzip2 => {
                    self.reader.seek(SeekFrom::Start(
                        self.koly.data_fork_offset + block_run.compressed_offset,
                    ))?;
                    let mut compressed = vec![0u8; block_run.compressed_length as usize];
                    self.reader.read_exact(&mut compressed)?;

                    let mut decoder = bzip2::read::BzDecoder::new(&compressed[..]);
                    let mut decompressed = vec![0u8; out_size as usize];
                    read_full(&mut decoder, &mut decompressed)?;
                    writer.write_all(&decompressed)?;
                    bytes_written += out_size;
                }
                BlockType::Lzfse | BlockType::Lzvn => {
                    self.reader.seek(SeekFrom::Start(
                        self.koly.data_fork_offset + block_run.compressed_offset,
                    ))?;
                    let mut compressed = vec![0u8; block_run.compressed_length as usize];
                    self.reader.read_exact(&mut compressed)?;

                    let expected_size = out_size as usize;
                    let mut temp_buf = vec![0u8; expected_size * 2];
                    let decoded_size = lzfse::decode_buffer(&compressed, &mut temp_buf)
                        .map_err(|e| DppError::Decompression(format!("LZFSE/LZVN: {:?}", e)))?;

                    let mut block = vec![0u8; expected_size];
                    let copy_size = decoded_size.min(expected_size);
                    block[..copy_size].copy_from_slice(&temp_buf[..copy_size]);
                    writer.write_all(&block)?;
                    bytes_written += expected_size as u64;
                }
                BlockType::Adc => {
                    return Err(DppError::Unsupported("ADC compression".into()));
                }
                BlockType::Comment | BlockType::End => {
                    // No data
                }
            }
        }

        // Pad to full partition size if needed
        if bytes_written < block_size {
            let remaining = (block_size - bytes_written) as usize;
            let zeros = vec![0u8; remaining];
            writer.write_all(&zeros)?;
            bytes_written += remaining as u64;
        }

        Ok(bytes_written)
    }

    /// Decompress the main HFS+ partition (largest one)
    pub fn decompress_main_partition(&mut self) -> Result<Vec<u8>> {
        let id = self.main_partition_id()?;
        self.decompress_partition(id)
    }

    /// Stream the main HFS+/APFS partition to a writer.
    pub fn decompress_main_partition_to<W: Write>(&mut self, writer: &mut W) -> Result<u64> {
        let id = self.main_partition_id()?;
        self.decompress_partition_to(id, writer)
    }

    /// Find the partition ID of the main HFS+/APFS partition.
    pub fn main_partition_id(&self) -> Result<i32> {
        let partition = self
            .partitions
            .iter()
            .filter(|p| {
                p.name.contains("Apple_HFS")
                    || p.name.contains("Apple_HFSX")
                    || p.name.contains("Apple_APFS")
            })
            .max_by_key(|p| p.block_map.sector_count)
            .or_else(|| {
                self.partitions
                    .iter()
                    .max_by_key(|p| p.block_map.sector_count)
            })
            .ok_or_else(|| DppError::FileNotFound("no partitions found".into()))?;
        Ok(partition.id)
    }

    /// Find the partition ID of the main HFS+/HFSX partition (excludes APFS).
    /// Returns `Err(FileNotFound)` if no HFS-compatible partition exists.
    pub fn hfs_partition_id(&self) -> Result<i32> {
        let partition = self
            .partitions
            .iter()
            .filter(|p| {
                p.name.contains("Apple_HFS") || p.name.contains("Apple_HFSX")
            })
            .max_by_key(|p| p.block_map.sector_count)
            .ok_or_else(|| {
                DppError::FileNotFound("no HFS+/HFSX partition found".into())
            })?;
        Ok(partition.id)
    }

    /// Decompress all partitions into a single raw disk image
    pub fn decompress_all(&mut self) -> Result<Vec<u8>> {
        let total_sectors = self.koly.sector_count;
        let total_size = total_sectors * SECTOR_SIZE;
        let mut output = vec![0u8; total_size as usize];

        for partition in self.partitions.clone() {
            for block_run in &partition.block_map.block_runs {
                let out_offset = (partition.block_map.first_sector + block_run.sector_number) * SECTOR_SIZE;
                let out_size = block_run.sector_count * SECTOR_SIZE;

                if out_offset + out_size > total_size {
                    continue; // Skip out-of-bounds blocks
                }

                match block_run.block_type {
                    BlockType::ZeroFill => {}
                    BlockType::Raw | BlockType::Ignore => {
                        if block_run.compressed_length > 0 {
                            self.reader.seek(SeekFrom::Start(
                                self.koly.data_fork_offset + block_run.compressed_offset,
                            ))?;
                            // Read only compressed_length bytes (actual stored size)
                            // remaining bytes in the sector stay zero-filled
                            let end = out_offset as usize + block_run.compressed_length as usize;
                            self.reader.read_exact(&mut output[out_offset as usize..end])?;
                        }
                    }
                    BlockType::Zlib => {
                        self.reader.seek(SeekFrom::Start(
                            self.koly.data_fork_offset + block_run.compressed_offset,
                        ))?;
                        let mut compressed = vec![0u8; block_run.compressed_length as usize];
                        self.reader.read_exact(&mut compressed)?;

                        let mut decoder = flate2::read::ZlibDecoder::new(&compressed[..]);
                        let end = (out_offset + out_size) as usize;
                        let _ = decoder.read(&mut output[out_offset as usize..end])?;
                    }
                    BlockType::Bzip2 => {
                        self.reader.seek(SeekFrom::Start(
                            self.koly.data_fork_offset + block_run.compressed_offset,
                        ))?;
                        let mut compressed = vec![0u8; block_run.compressed_length as usize];
                        self.reader.read_exact(&mut compressed)?;

                        let mut decoder = bzip2::read::BzDecoder::new(&compressed[..]);
                        let end = (out_offset + out_size) as usize;
                        let _ = decoder.read(&mut output[out_offset as usize..end])?;
                    }
                    BlockType::Lzfse | BlockType::Lzvn => {
                        self.reader.seek(SeekFrom::Start(
                            self.koly.data_fork_offset + block_run.compressed_offset,
                        ))?;
                        let mut compressed = vec![0u8; block_run.compressed_length as usize];
                        self.reader.read_exact(&mut compressed)?;

                        // LZFSE decoder needs extra buffer space
                        let expected_size = out_size as usize;
                        let mut temp_buf = vec![0u8; expected_size * 2];
                        let decoded_size = lzfse::decode_buffer(&compressed, &mut temp_buf)
                            .map_err(|e| DppError::Decompression(format!("LZFSE/LZVN: {:?}", e)))?;

                        let copy_size = decoded_size.min(expected_size);
                        let end = out_offset as usize + copy_size;
                        output[out_offset as usize..end].copy_from_slice(&temp_buf[..copy_size]);
                    }
                    BlockType::Adc => {
                        return Err(DppError::Unsupported("ADC compression".into()));
                    }
                    BlockType::Comment | BlockType::End => {}
                }
            }
        }

        Ok(output)
    }

    /// Get info about block compression types used
    pub fn compression_info(&self) -> CompressionInfo {
        let mut info = CompressionInfo::default();

        for partition in &self.partitions {
            for block_run in &partition.block_map.block_runs {
                match block_run.block_type {
                    BlockType::ZeroFill => info.zero_fill_blocks += 1,
                    BlockType::Raw => info.raw_blocks += 1,
                    BlockType::Zlib => info.zlib_blocks += 1,
                    BlockType::Bzip2 => info.bzip2_blocks += 1,
                    BlockType::Lzfse => info.lzfse_blocks += 1,
                    BlockType::Lzvn => info.lzvn_blocks += 1,
                    BlockType::Adc => info.adc_blocks += 1,
                    _ => {}
                }
            }
        }

        info
    }
}

impl DmgReader<BufReader<File>> {
    /// Open a DMG file from a path with default options (checksum verification enabled)
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        Self::open_with_options(path, DmgReaderOptions::default())
    }

    /// Open a DMG file from a path with custom options
    pub fn open_with_options<P: AsRef<Path>>(path: P, options: DmgReaderOptions) -> Result<Self> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        Self::with_options(reader, options)
    }
}

/// Statistics about a DMG file
#[derive(Debug, Clone)]
pub struct DmgStats {
    /// DMG version
    pub version: u32,
    /// Total sector count
    pub sector_count: u64,
    /// Number of partitions
    pub partition_count: usize,
    /// Total uncompressed size in bytes
    pub total_uncompressed: u64,
    /// Total compressed size in bytes
    pub total_compressed: u64,
    /// Data fork length
    pub data_fork_length: u64,
}

impl DmgStats {
    /// Calculate compression ratio
    pub fn compression_ratio(&self) -> f64 {
        if self.total_uncompressed == 0 {
            return 1.0;
        }
        self.total_compressed as f64 / self.total_uncompressed as f64
    }

    /// Calculate space savings percentage
    pub fn space_savings(&self) -> f64 {
        (1.0 - self.compression_ratio()) * 100.0
    }
}

/// Information about compression methods used
#[derive(Debug, Clone, Default)]
pub struct CompressionInfo {
    pub zero_fill_blocks: u32,
    pub raw_blocks: u32,
    pub zlib_blocks: u32,
    pub bzip2_blocks: u32,
    pub lzfse_blocks: u32,
    pub lzvn_blocks: u32,
    pub adc_blocks: u32,
}

/// Parse the DMG plist to extract partition info
fn parse_plist(plist_data: &[u8]) -> Result<Vec<PartitionEntry>> {
    // Parse using plist crate
    let plist: plist::Value = plist::from_bytes(plist_data)
        .map_err(|e| DppError::InvalidPlist(format!("plist parse error: {}", e)))?;

    let dict = plist
        .as_dictionary()
        .ok_or_else(|| DppError::InvalidPlist("expected dictionary".into()))?;

    let resource_fork = dict
        .get("resource-fork")
        .and_then(|v| v.as_dictionary())
        .ok_or_else(|| DppError::InvalidPlist("missing resource-fork".into()))?;

    let blkx = resource_fork
        .get("blkx")
        .and_then(|v| v.as_array())
        .ok_or_else(|| DppError::InvalidPlist("missing blkx array".into()))?;

    let mut partitions = Vec::with_capacity(blkx.len());

    for entry in blkx {
        let entry_dict = entry
            .as_dictionary()
            .ok_or_else(|| DppError::InvalidPlist("blkx entry not a dictionary".into()))?;

        let name = entry_dict
            .get("Name")
            .and_then(|v| v.as_string())
            .unwrap_or("")
            .to_string();

        let id = entry_dict
            .get("ID")
            .and_then(|v| v.as_string())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        let attributes = entry_dict
            .get("Attributes")
            .and_then(|v| v.as_string())
            .and_then(|s| {
                if s.starts_with("0x") {
                    u32::from_str_radix(&s[2..], 16).ok()
                } else {
                    s.parse().ok()
                }
            })
            .unwrap_or(0);

        let data = entry_dict
            .get("Data")
            .and_then(|v| v.as_data())
            .ok_or_else(|| DppError::InvalidPlist("missing Data in blkx entry".into()))?;

        let block_map = MishHeader::from_bytes(data)?;

        partitions.push(PartitionEntry {
            name,
            id,
            attributes,
            block_map,
        });
    }

    Ok(partitions)
}

/// Convenience function to open a DMG file
pub fn open<P: AsRef<Path>>(path: P) -> Result<DmgReader<BufReader<File>>> {
    DmgReader::open(path)
}

/// Check if a file is a valid DMG
pub fn is_dmg<P: AsRef<Path>>(path: P) -> bool {
    File::open(path)
        .ok()
        .map(BufReader::new)
        .map(|mut r| crate::format::is_dmg(&mut r))
        .unwrap_or(false)
}
