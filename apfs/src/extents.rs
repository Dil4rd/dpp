use std::io::{Read, Seek, SeekFrom, Write};

use crate::catalog::FileExtentRecord;
use crate::error::{ApfsError, Result};

/// Write `count` zero bytes to `writer`, reusing `scratch` as the source.
fn write_hole<W: Write>(writer: &mut W, mut count: u64, scratch: &[u8]) -> Result<()> {
    while count > 0 {
        let chunk = count.min(scratch.len() as u64) as usize;
        writer.write_all(&scratch[..chunk])?;
        count -= chunk as u64;
    }
    Ok(())
}

/// Read file data from extents, streaming to a writer.
/// Returns the number of bytes written.
///
/// Each extent is placed at the logical address from its own record rather
/// than at the running sum of the lengths before it, so a sparse file's
/// extents land where they belong. A gap between two extents is a hole, and a
/// hole reads as zeros: an extent exists beyond it, so the file genuinely
/// extends across the gap and simply has no backing store there.
pub fn read_file_data<R: Read + Seek, W: Write>(
    reader: &mut R,
    block_size: u32,
    extents: &[FileExtentRecord],
    logical_size: u64,
    writer: &mut W,
) -> Result<u64> {
    if logical_size == 0 {
        return Ok(0);
    }

    let block_size = block_size as u64;
    let mut bytes_written: u64 = 0;
    let mut buf = vec![0u8; block_size as usize];
    let zeros = vec![0u8; block_size as usize];

    // The catalog orders extent records by logical address, so this is a
    // no-op on a well-formed volume. Sorting anyway keeps the streaming walk
    // in order if it is not, rather than emitting bytes at the wrong offsets.
    let mut ordered: Vec<&FileExtentRecord> = extents.iter().collect();
    ordered.sort_by_key(|record| record.logical_addr);

    for record in ordered {
        if bytes_written >= logical_size {
            break;
        }

        let extent_length = record.value.length();
        if extent_length == 0 {
            continue;
        }

        // Hole before this extent.
        if record.logical_addr > bytes_written {
            let gap = (record.logical_addr - bytes_written).min(logical_size - bytes_written);
            write_hole(writer, gap, &zeros)?;
            bytes_written += gap;
            if bytes_written >= logical_size {
                break;
            }
        }

        // PROVISIONAL(anomaly-channel): overlapping extents. The bytes
        // already emitted win and the overlap is skipped, which contains the
        // damage to this file but tells the caller nothing about it.
        let skip = bytes_written.saturating_sub(record.logical_addr);
        if skip >= extent_length {
            continue;
        }

        let phys_start = record
            .value
            .phys_block_num
            .checked_mul(block_size)
            .ok_or_else(|| {
                ApfsError::CorruptedData(format!(
                    "extent physical block {} overflows the device address space",
                    record.value.phys_block_num
                ))
            })?;

        let mut extent_offset = skip;
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

    // PROVISIONAL(anomaly-channel): a gap after the last extent is left
    // unwritten and surfaces only as a short count. Unlike an interior hole
    // it is ambiguous — a genuine trailing hole and a lost tail of extent
    // records look identical here — so zero-filling would fabricate bytes
    // indistinguishable from recovered ones. Resolve with the anomaly
    // channel, not by symmetry with the interior case.
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
        extents: Vec<FileExtentRecord>,
        logical_size: u64,
    ) -> Self {
        let block_size = block_size as u64;
        let mut extent_map = Vec::new();

        for record in &extents {
            let length = record.value.length();
            if length == 0 {
                continue;
            }
            let physical_start = match record.value.phys_block_num.checked_mul(block_size) {
                Some(start) => start,
                // A block number that cannot be addressed cannot be read from
                // either. Drop the extent rather than seeking somewhere else.
                // PROVISIONAL(anomaly-channel): dropped silently, turning the
                // extent into a hole with no signal to the caller.
                None => continue,
            };
            // Placed at the address from the record's own key, not at the
            // running total of the lengths before it.
            extent_map.push((record.logical_addr, physical_start, length));
        }
        extent_map.sort_by_key(|&(logical_start, _, _)| logical_start);

        ApfsForkReader {
            reader,
            logical_size,
            extent_map,
            position: 0,
        }
    }

    /// Map a logical offset to backing storage, or to the hole it falls in.
    fn map_offset(&self, logical_offset: u64) -> Mapping {
        for &(log_start, phys_start, length) in &self.extent_map {
            if logical_offset >= log_start && logical_offset < log_start + length {
                return Mapping::Data {
                    physical: phys_start + (logical_offset - log_start),
                    available: (log_start + length) - logical_offset,
                };
            }
        }

        // Not backed by any extent: a hole running to the next extent, or to
        // the end of the file if none follows.
        let next_start = self
            .extent_map
            .iter()
            .map(|&(log_start, _, _)| log_start)
            .filter(|&log_start| log_start > logical_offset)
            .min()
            .unwrap_or(self.logical_size);
        Mapping::Hole {
            available: next_start.saturating_sub(logical_offset),
        }
    }
}

/// What backs a given logical offset in a fork.
enum Mapping {
    Data { physical: u64, available: u64 },
    Hole { available: u64 },
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
            let wanted = (to_read - total_read) as u64;

            let chunk_size = match self.map_offset(logical_pos) {
                Mapping::Data {
                    physical,
                    available,
                } => {
                    let chunk_size = wanted.min(available) as usize;
                    self.reader.seek(SeekFrom::Start(physical))?;
                    self.reader
                        .read_exact(&mut buf[total_read..total_read + chunk_size])?;
                    chunk_size
                }
                // A hole is genuinely zeros on disk, not missing data.
                Mapping::Hole { available } => {
                    let chunk_size = wanted.min(available) as usize;
                    buf[total_read..total_read + chunk_size].fill(0);
                    chunk_size
                }
            };

            if chunk_size == 0 {
                // Nothing backs this offset and nothing follows it, so the
                // map cannot advance. Stop rather than spin.
                break;
            }
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
    use super::*;
    use crate::catalog::{FileExtentRecord, FileExtentVal};

    #[test]
    fn test_extent_block_number_beyond_the_address_space_is_rejected() {
        // phys_block_num comes straight off disk. Multiplied by the block size
        // it used to wrap, sending the read to an unrelated offset that looks
        // like recovered data.
        let extents = vec![FileExtentRecord {
            logical_addr: 0,
            value: FileExtentVal {
                flags_and_length: 4096,
                phys_block_num: u64::MAX,
                crypto_id: 0,
            },
        }];

        let mut reader = std::io::Cursor::new(vec![0u8; 8192]);
        let mut out = Vec::new();
        let err = read_file_data(&mut reader, 4096, &extents, 4096, &mut out).unwrap_err();

        assert!(
            matches!(err, ApfsError::CorruptedData(_)),
            "expected CorruptedData, got {err:?}"
        );
    }

    /// A 12 KiB file: 4 KiB of data, a 4 KiB hole, then 4 KiB of data. The
    /// second extent's records says it lives at 8192; summing the lengths
    /// before it would place it at 4096.
    fn sparse_fork() -> (std::io::Cursor<Vec<u8>>, Vec<FileExtentRecord>, u64) {
        let mut disk = vec![0u8; 4096 * 3];
        disk[4096..8192].fill(b'A');
        disk[8192..12288].fill(b'B');

        let extents = vec![
            FileExtentRecord {
                logical_addr: 0,
                value: FileExtentVal {
                    flags_and_length: 4096,
                    phys_block_num: 1,
                    crypto_id: 0,
                },
            },
            FileExtentRecord {
                logical_addr: 8192,
                value: FileExtentVal {
                    flags_and_length: 4096,
                    phys_block_num: 2,
                    crypto_id: 0,
                },
            },
        ];
        (std::io::Cursor::new(disk), extents, 12288)
    }

    fn expected_sparse_contents() -> Vec<u8> {
        let mut expected = vec![b'A'; 4096];
        expected.extend(std::iter::repeat_n(0u8, 4096));
        expected.extend(std::iter::repeat_n(b'B', 4096));
        expected
    }

    #[test]
    fn test_read_file_data_places_extents_at_their_logical_address() {
        let (mut reader, extents, logical_size) = sparse_fork();
        let mut out = Vec::new();
        let written = read_file_data(&mut reader, 4096, &extents, logical_size, &mut out).unwrap();

        assert_eq!(written, logical_size);
        assert_eq!(out, expected_sparse_contents());
    }

    #[test]
    fn test_read_file_data_is_independent_of_record_order() {
        let (mut reader, mut extents, logical_size) = sparse_fork();
        extents.reverse();
        let mut out = Vec::new();
        read_file_data(&mut reader, 4096, &extents, logical_size, &mut out).unwrap();

        assert_eq!(out, expected_sparse_contents());
    }

    #[test]
    fn test_fork_reader_reads_holes_as_zeros() {
        let (mut cursor, extents, logical_size) = sparse_fork();
        let mut fork = ApfsForkReader::new(&mut cursor, 4096, extents, logical_size);

        let mut out = Vec::new();
        fork.read_to_end(&mut out).unwrap();
        assert_eq!(out, expected_sparse_contents());
    }

    #[test]
    fn test_fork_reader_seeks_past_a_hole() {
        // Previously this returned UnexpectedEof: the map placed the second
        // extent at 4096, so nothing covered offset 9000.
        let (mut cursor, extents, logical_size) = sparse_fork();
        let mut fork = ApfsForkReader::new(&mut cursor, 4096, extents, logical_size);

        fork.seek(SeekFrom::Start(9000)).unwrap();
        let mut out = vec![0u8; 8];
        fork.read_exact(&mut out).unwrap();
        assert_eq!(out, [b'B'; 8]);

        // And a read landing inside the hole yields zeros.
        fork.seek(SeekFrom::Start(5000)).unwrap();
        let mut out = vec![0xFFu8; 8];
        fork.read_exact(&mut out).unwrap();
        assert_eq!(out, [0u8; 8]);
    }

    /// Requires ../tests/appfs.raw fixture. Run with `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn test_read_file() {
        let file = std::fs::File::open("../tests/appfs.raw").unwrap();
        let reader = std::io::BufReader::new(file);
        let mut vol = crate::ApfsVolume::open(reader).unwrap();

        let walk = vol.walk().unwrap();
        let small_file = walk.iter().find(|e| {
            e.entry.kind == crate::EntryKind::File && e.entry.size > 0 && e.entry.size < 100_000
        });

        let entry = small_file.expect("Should find a small file in the test image");
        let data = vol.read_file(&entry.path).unwrap();
        assert!(!data.is_empty(), "File data should not be empty");
        assert_eq!(data.len() as u64, entry.entry.size);
    }
}
