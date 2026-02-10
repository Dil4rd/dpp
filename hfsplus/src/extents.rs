use byteorder::{BigEndian, ReadBytesExt};
use std::io::{Cursor, Read, Seek, SeekFrom, Write};

use crate::btree::{self, BTreeHeaderRecord};
use crate::error::{HfsPlusError, Result};
use crate::volume::{ExtentDescriptor, ForkData, VolumeHeader};

/// A reader that presents a file's data fork as a contiguous `Read + Seek` stream.
/// Translates logical file offsets to physical disk offsets through the extent map.
pub struct ForkReader<'a, R: Read + Seek> {
    reader: &'a mut R,
    logical_size: u64,
    /// Flattened list of (logical_start_byte, physical_start_byte, length_bytes)
    extent_map: Vec<(u64, u64, u64)>,
    position: u64,
}

impl<'a, R: Read + Seek> ForkReader<'a, R> {
    /// Create a ForkReader from a fork's inline extents.
    /// For files with overflow extents, call `with_overflow_extents` instead.
    pub fn new(
        reader: &'a mut R,
        fork: &ForkData,
        block_size: u32,
    ) -> Self {
        let block_size = block_size as u64;
        let mut extent_map = Vec::new();
        let mut logical_offset = 0u64;

        for extent in &fork.extents {
            if extent.block_count == 0 {
                break;
            }
            let physical_start = extent.start_block as u64 * block_size;
            let length = extent.block_count as u64 * block_size;
            extent_map.push((logical_offset, physical_start, length));
            logical_offset += length;
        }

        ForkReader {
            reader,
            logical_size: fork.logical_size,
            extent_map,
            position: 0,
        }
    }

    /// Translate a logical offset to a physical offset
    fn logical_to_physical(&self, logical_offset: u64) -> Option<u64> {
        for &(log_start, phys_start, length) in &self.extent_map {
            if logical_offset >= log_start && logical_offset < log_start + length {
                return Some(phys_start + (logical_offset - log_start));
            }
        }
        None
    }
}

impl<R: Read + Seek> Read for ForkReader<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.position >= self.logical_size {
            return Ok(0);
        }

        let remaining = (self.logical_size - self.position) as usize;
        let to_read = buf.len().min(remaining);
        if to_read == 0 {
            return Ok(0);
        }

        let mut total_read = 0;
        while total_read < to_read {
            let logical_pos = self.position + total_read as u64;

            // Find which extent this position falls in
            let physical_pos = self.logical_to_physical(logical_pos)
                .ok_or_else(|| std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "logical offset beyond extent map",
                ))?;

            // Calculate how many contiguous bytes we can read from this extent
            let mut extent_remaining = 0u64;
            for &(log_start, _, length) in &self.extent_map {
                if logical_pos >= log_start && logical_pos < log_start + length {
                    extent_remaining = (log_start + length) - logical_pos;
                    break;
                }
            }

            let chunk_size = ((to_read - total_read) as u64).min(extent_remaining) as usize;

            self.reader.seek(SeekFrom::Start(physical_pos))?;
            self.reader.read_exact(&mut buf[total_read..total_read + chunk_size])?;

            total_read += chunk_size;
        }

        self.position += total_read as u64;
        Ok(total_read)
    }
}

impl<R: Read + Seek> Seek for ForkReader<'_, R> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let new_pos = match pos {
            SeekFrom::Start(offset) => offset as i64,
            SeekFrom::Current(offset) => self.position as i64 + offset,
            SeekFrom::End(offset) => self.logical_size as i64 + offset,
        };

        if new_pos < 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "seek before start of file",
            ));
        }

        self.position = new_pos as u64;
        Ok(self.position)
    }
}

/// Fork types
pub const FORK_TYPE_DATA: u8 = 0x00;
pub const FORK_TYPE_RESOURCE: u8 = 0xFF;

/// Read file data from a fork, streaming to a writer.
/// Returns the number of bytes written.
pub fn read_fork_data<R: Read + Seek, W: Write>(
    reader: &mut R,
    vol: &VolumeHeader,
    extents_btree: &BTreeHeaderRecord,
    fork: &ForkData,
    file_id: u32,
    writer: &mut W,
) -> Result<u64> {
    let block_size = vol.block_size as u64;
    let total_bytes = fork.logical_size;

    if total_bytes == 0 {
        return Ok(0);
    }

    let mut bytes_written: u64 = 0;
    let mut buf = vec![0u8; vol.block_size as usize];

    // First, read from the inline extents (up to 8)
    for extent in &fork.extents {
        if extent.block_count == 0 || bytes_written >= total_bytes {
            break;
        }
        bytes_written += read_extent(
            reader, extent, block_size, total_bytes - bytes_written, &mut buf, writer,
        )?;
    }

    // If we've read all the data, we're done
    if bytes_written >= total_bytes {
        return Ok(bytes_written);
    }

    // Otherwise, look up overflow extents from the Extents B-tree
    let mut start_block = fork.extents.iter()
        .map(|e| e.block_count)
        .sum::<u32>();

    loop {
        if bytes_written >= total_bytes {
            break;
        }

        // Look up the next extent record in the overflow B-tree
        let overflow_extents = lookup_overflow_extents(
            reader,
            extents_btree,
            file_id,
            FORK_TYPE_DATA,
            start_block,
        )?;

        if overflow_extents.is_empty() {
            break;
        }

        for extent in &overflow_extents {
            if extent.block_count == 0 || bytes_written >= total_bytes {
                break;
            }
            bytes_written += read_extent(
                reader, extent, block_size, total_bytes - bytes_written, &mut buf, writer,
            )?;
            start_block += extent.block_count;
        }
    }

    Ok(bytes_written)
}

/// Read data from a single extent, writing to the output.
/// Returns bytes written.
fn read_extent<R: Read + Seek, W: Write>(
    reader: &mut R,
    extent: &ExtentDescriptor,
    block_size: u64,
    remaining: u64,
    buf: &mut Vec<u8>,
    writer: &mut W,
) -> Result<u64> {
    let mut written = 0u64;
    let start_offset = extent.start_block as u64 * block_size;

    for block_idx in 0..extent.block_count as u64 {
        if written >= remaining {
            break;
        }

        let offset = start_offset + block_idx * block_size;
        reader.seek(SeekFrom::Start(offset))?;

        let to_read = std::cmp::min(block_size, remaining - written) as usize;
        reader.read_exact(&mut buf[..to_read])?;
        writer.write_all(&buf[..to_read])?;
        written += to_read as u64;
    }

    Ok(written)
}

/// Look up overflow extent records from the Extents B-tree.
/// Returns up to 8 extent descriptors.
fn lookup_overflow_extents<R: Read + Seek>(
    reader: &mut R,
    extents_btree: &BTreeHeaderRecord,
    file_id: u32,
    fork_type: u8,
    start_block: u32,
) -> Result<Vec<ExtentDescriptor>> {
    let comparator = move |record_data: &[u8]| -> std::cmp::Ordering {
        if record_data.len() < 12 {
            return std::cmp::Ordering::Less;
        }
        // Extent key: key_length(2) + fork_type(1) + pad(1) + file_id(4) + start_block(4)
        let _key_length = u16::from_be_bytes([record_data[0], record_data[1]]);
        let rec_fork_type = record_data[2];
        let rec_file_id = u32::from_be_bytes([
            record_data[4], record_data[5], record_data[6], record_data[7],
        ]);
        let rec_start_block = u32::from_be_bytes([
            record_data[8], record_data[9], record_data[10], record_data[11],
        ]);

        match rec_file_id.cmp(&file_id) {
            std::cmp::Ordering::Equal => {}
            ord => return ord,
        }
        match rec_fork_type.cmp(&fork_type) {
            std::cmp::Ordering::Equal => {}
            ord => return ord,
        }
        rec_start_block.cmp(&start_block)
    };

    match btree::search_btree(reader, extents_btree, &comparator)? {
        Some((node, record_idx)) => {
            let record_data = node.record_data(record_idx)?;
            // Key is 12 bytes (2 key_length + 1 fork_type + 1 pad + 4 file_id + 4 start_block)
            let key_length = u16::from_be_bytes([record_data[0], record_data[1]]) as usize;
            let data_start = 2 + key_length;
            if data_start + 64 > record_data.len() {
                return Err(HfsPlusError::InvalidBTree("extent record too short".into()));
            }

            let mut cursor = Cursor::new(&record_data[data_start..]);
            let mut extents = Vec::with_capacity(8);
            for _ in 0..8 {
                let start = cursor.read_u32::<BigEndian>()?;
                let count = cursor.read_u32::<BigEndian>()?;
                extents.push(ExtentDescriptor {
                    start_block: start,
                    block_count: count,
                });
            }
            Ok(extents)
        }
        None => Ok(Vec::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_pkg_header_from_kdk() {
        let path = std::path::Path::new("../tests/kdk.raw");
        if !path.exists() {
            eprintln!("Skipping test - kdk.raw not found");
            return;
        }

        let file = std::fs::File::open(path).unwrap();
        let mut reader = std::io::BufReader::new(file);
        let vol = crate::volume::VolumeHeader::parse(&mut reader).unwrap();
        let catalog_header = btree::read_btree_header(
            &mut reader, &vol.catalog_file, vol.block_size,
        ).unwrap();
        let extents_header = btree::read_btree_header(
            &mut reader, &vol.extents_file, vol.block_size,
        ).unwrap();

        // Look up KernelDebugKit.pkg
        let record = crate::catalog::lookup_catalog(
            &mut reader, &vol, &catalog_header,
            crate::catalog::CNID_ROOT_FOLDER, "KernelDebugKit.pkg",
        ).unwrap();

        let file_rec = match record {
            Some(crate::catalog::CatalogRecord::File(f)) => f,
            other => panic!("Expected File record, got {:?}", other.map(|r| format!("{:?}", r))),
        };

        eprintln!("KernelDebugKit.pkg: cnid={}, size={}", file_rec.file_id, file_rec.data_fork.logical_size);
        eprintln!("  First extent: start_block={}, block_count={}",
            file_rec.data_fork.extents[0].start_block,
            file_rec.data_fork.extents[0].block_count);

        // Read just the first block to check magic
        let offset = file_rec.data_fork.extents[0].start_block as u64 * vol.block_size as u64;
        reader.seek(std::io::SeekFrom::Start(offset)).unwrap();
        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic).unwrap();

        eprintln!("PKG magic bytes: {:02x} {:02x} {:02x} {:02x}", magic[0], magic[1], magic[2], magic[3]);
        assert_eq!(&magic, b"xar!", "PKG file should start with XAR magic 'xar!'");
        eprintln!("Confirmed: KernelDebugKit.pkg is a valid XAR archive!");

        // Test ForkReader: read XAR header (28 bytes) via streaming Read+Seek
        let mut fork_reader = ForkReader::new(&mut reader, &file_rec.data_fork, vol.block_size);

        let mut xar_header = [0u8; 28];
        fork_reader.read_exact(&mut xar_header).unwrap();
        assert_eq!(&xar_header[..4], b"xar!", "ForkReader should read XAR magic");

        // Test seek: jump to position 0 and re-read magic
        fork_reader.seek(SeekFrom::Start(0)).unwrap();
        let mut magic2 = [0u8; 4];
        fork_reader.read_exact(&mut magic2).unwrap();
        assert_eq!(&magic2, b"xar!", "ForkReader seek+read should work");

        // Test seek to end
        let end = fork_reader.seek(SeekFrom::End(0)).unwrap();
        assert_eq!(end, file_rec.data_fork.logical_size, "SeekFrom::End should match file size");

        eprintln!("ForkReader tests passed! Read XAR header via streaming fork access.");
    }
}
