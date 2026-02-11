use std::io::{Read, Seek, SeekFrom, Write};

use crate::catalog::FileExtentVal;
use crate::error::Result;

/// Read file data from extents, streaming to a writer.
/// Returns the number of bytes written.
pub fn read_file_data<R: Read + Seek, W: Write>(
    reader: &mut R,
    block_size: u32,
    extents: &[FileExtentVal],
    logical_size: u64,
    writer: &mut W,
) -> Result<u64> {
    if logical_size == 0 {
        return Ok(0);
    }

    let block_size = block_size as u64;
    let mut bytes_written: u64 = 0;
    let mut buf = vec![0u8; block_size as usize];

    for extent in extents {
        if bytes_written >= logical_size {
            break;
        }

        let extent_length = extent.length();
        let phys_start = extent.phys_block_num * block_size;

        let mut extent_offset = 0u64;
        while extent_offset < extent_length && bytes_written < logical_size {
            let remaining_in_file = logical_size - bytes_written;
            let remaining_in_extent = extent_length - extent_offset;
            let to_read = remaining_in_file.min(remaining_in_extent).min(block_size) as usize;

            reader.seek(SeekFrom::Start(phys_start + extent_offset))?;
            reader.read_exact(&mut buf[..to_read])?;
            writer.write_all(&buf[..to_read])?;

            bytes_written += to_read as u64;
            extent_offset += to_read as u64;
        }
    }

    Ok(bytes_written)
}

/// A reader that presents a file's extents as a contiguous Read + Seek stream.
pub struct ApfsForkReader<'a, R: Read + Seek> {
    reader: &'a mut R,
    logical_size: u64,
    /// (logical_start, physical_start, length_bytes)
    extent_map: Vec<(u64, u64, u64)>,
    position: u64,
}

impl<'a, R: Read + Seek> ApfsForkReader<'a, R> {
    pub fn new(
        reader: &'a mut R,
        block_size: u32,
        extents: Vec<FileExtentVal>,
        logical_size: u64,
    ) -> Self {
        let block_size = block_size as u64;
        let mut extent_map = Vec::new();
        let mut logical_offset = 0u64;

        for extent in &extents {
            let length = extent.length();
            if length == 0 {
                continue;
            }
            let physical_start = extent.phys_block_num * block_size;
            extent_map.push((logical_offset, physical_start, length));
            logical_offset += length;
        }

        ApfsForkReader {
            reader,
            logical_size,
            extent_map,
            position: 0,
        }
    }

    fn logical_to_physical(&self, logical_offset: u64) -> Option<u64> {
        for &(log_start, phys_start, length) in &self.extent_map {
            if logical_offset >= log_start && logical_offset < log_start + length {
                return Some(phys_start + (logical_offset - log_start));
            }
        }
        None
    }
}

impl<R: Read + Seek> Read for ApfsForkReader<'_, R> {
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

            let physical_pos = self.logical_to_physical(logical_pos)
                .ok_or_else(|| std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "logical offset beyond extent map",
                ))?;

            // Calculate contiguous bytes available in this extent
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

impl<R: Read + Seek> Seek for ApfsForkReader<'_, R> {
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

#[cfg(test)]
mod tests {
    /// Requires ../tests/appfs.raw fixture. Run with `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn test_read_file() {
        let file = std::fs::File::open("../tests/appfs.raw").unwrap();
        let reader = std::io::BufReader::new(file);
        let mut vol = crate::ApfsVolume::open(reader).unwrap();

        let walk = vol.walk().unwrap();
        let small_file = walk.iter()
            .find(|e| e.entry.kind == crate::EntryKind::File && e.entry.size > 0 && e.entry.size < 100_000);

        let entry = small_file.expect("Should find a small file in the test image");
        let data = vol.read_file(&entry.path).unwrap();
        assert!(!data.is_empty(), "File data should not be empty");
        assert_eq!(data.len() as u64, entry.entry.size);
    }
}
