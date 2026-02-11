//! PBZX archive reader implementation.

use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom, Write};
use std::path::Path;

use byteorder::{BigEndian, ReadBytesExt};
use xz2::read::XzDecoder;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use crate::error::{PbzxError, Result};
use crate::format::{ChunkHeader, PbzxHeader, CHUNK_HEADER_SIZE, HEADER_SIZE, PBZX_MAGIC};

/// A reader for PBZX archives.
///
/// This struct provides methods to read and decompress PBZX archives.
/// It supports streaming decompression to minimize memory usage.
///
/// # Example
///
/// ```no_run
/// use pbzx::PbzxReader;
/// use std::fs::File;
/// use std::io::BufReader;
///
/// let file = File::open("Payload").unwrap();
/// let reader = BufReader::new(file);
/// let mut pbzx = PbzxReader::new(reader).unwrap();
///
/// // Decompress to a file
/// let mut output = File::create("output.cpio").unwrap();
/// pbzx.decompress_to(&mut output).unwrap();
/// ```
pub struct PbzxReader<R> {
    reader: R,
    header: PbzxHeader,
    current_offset: u64,
    total_decompressed: u64,
}

impl<R: Read> PbzxReader<R> {
    /// Create a new PBZX reader from a Read source.
    ///
    /// This reads and validates the PBZX header.
    pub fn new(mut reader: R) -> Result<Self> {
        let header = Self::read_header(&mut reader)?;

        if !header.is_valid() {
            return Err(PbzxError::InvalidMagic(header.magic));
        }

        Ok(Self {
            reader,
            header,
            current_offset: HEADER_SIZE as u64,
            total_decompressed: 0,
        })
    }

    /// Get the PBZX header.
    pub fn header(&self) -> &PbzxHeader {
        &self.header
    }

    /// Get the flags value from the header.
    pub fn flags(&self) -> u64 {
        self.header.flags
    }

    /// Get the total bytes decompressed so far.
    pub fn total_decompressed(&self) -> u64 {
        self.total_decompressed
    }

    fn read_header(reader: &mut R) -> Result<PbzxHeader> {
        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic)?;

        let flags = reader.read_u64::<BigEndian>()?;

        Ok(PbzxHeader { magic, flags })
    }

    /// Read the next chunk header, if any.
    fn read_chunk_header(&mut self) -> Result<Option<ChunkHeader>> {
        let uncompressed_size = match self.reader.read_u64::<BigEndian>() {
            Ok(v) => v,
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(e.into()),
        };

        let compressed_size = self.reader.read_u64::<BigEndian>()?;

        self.current_offset += CHUNK_HEADER_SIZE as u64;

        // End marker check
        if uncompressed_size == 0 && compressed_size == 0 {
            return Ok(None);
        }

        Ok(Some(ChunkHeader {
            uncompressed_size,
            compressed_size,
        }))
    }

    /// Decompress the entire PBZX archive to a writer.
    ///
    /// Returns the total number of bytes written.
    pub fn decompress_to<W: Write>(&mut self, writer: &mut W) -> Result<u64> {
        let mut total_written = 0u64;

        while let Some(chunk) = self.read_chunk_header()? {
            let chunk_start = self.current_offset;

            // Read the compressed chunk data
            let mut chunk_data = vec![0u8; chunk.compressed_size as usize];
            self.reader.read_exact(&mut chunk_data)?;
            self.current_offset += chunk.compressed_size;

            // Decompress or copy directly
            if chunk.is_uncompressed() {
                writer.write_all(&chunk_data)?;
                total_written += chunk_data.len() as u64;
            } else {
                // Decompress using XZ
                let mut decoder = XzDecoder::new(&chunk_data[..]);
                let mut decompressed = Vec::with_capacity(chunk.uncompressed_size as usize);

                decoder.read_to_end(&mut decompressed).map_err(|e| {
                    PbzxError::Decompression(format!(
                        "Failed to decompress chunk at offset {}: {}",
                        chunk_start, e
                    ))
                })?;

                if decompressed.len() as u64 != chunk.uncompressed_size {
                    return Err(PbzxError::InvalidChunk {
                        offset: chunk_start,
                        message: format!(
                            "Decompressed size mismatch: expected {}, got {}",
                            chunk.uncompressed_size,
                            decompressed.len()
                        ),
                    });
                }

                writer.write_all(&decompressed)?;
                total_written += decompressed.len() as u64;
            }
        }

        self.total_decompressed = total_written;
        Ok(total_written)
    }

    /// Decompress to a Vec<u8>.
    ///
    /// Note: This loads the entire decompressed content into memory.
    /// For large archives, prefer `decompress_to` with a file writer.
    pub fn decompress(&mut self) -> Result<Vec<u8>> {
        let mut output = Vec::new();
        self.decompress_to(&mut output)?;
        Ok(output)
    }
}

/// A chunk read from the archive, ready for decompression.
#[cfg(feature = "parallel")]
struct ReadChunk {
    /// The chunk header
    header: ChunkHeader,
    /// Offset of this chunk in the file (for error reporting)
    offset: u64,
    /// The raw (possibly compressed) data
    data: Vec<u8>,
}

#[cfg(feature = "parallel")]
impl<R: Read> PbzxReader<R> {
    /// Read all chunks into memory sequentially.
    fn read_all_chunks(&mut self) -> Result<Vec<ReadChunk>> {
        let mut chunks = Vec::new();

        while let Some(header) = self.read_chunk_header()? {
            let offset = self.current_offset;
            let mut data = vec![0u8; header.compressed_size as usize];
            self.reader.read_exact(&mut data)?;
            self.current_offset += header.compressed_size;

            chunks.push(ReadChunk {
                header,
                offset,
                data,
            });
        }

        Ok(chunks)
    }

    /// Decompress the entire PBZX archive using parallel decompression.
    ///
    /// Uses rayon to decompress all XZ chunks in parallel across multiple
    /// threads, then concatenates results in chunk order.
    ///
    /// Returns the total number of bytes written.
    pub fn decompress_parallel_to<W: Write>(&mut self, writer: &mut W) -> Result<u64> {
        let output = self.decompress_parallel()?;
        let len = output.len() as u64;
        writer.write_all(&output)?;
        Ok(len)
    }

    /// Decompress the entire PBZX archive using parallel decompression.
    ///
    /// Uses rayon to decompress all XZ chunks in parallel across multiple
    /// threads, then concatenates results in chunk order.
    ///
    /// Returns the decompressed data as a `Vec<u8>`.
    pub fn decompress_parallel(&mut self) -> Result<Vec<u8>> {
        let chunks = self.read_all_chunks()?;

        // Parallel decompress each chunk
        let results: Vec<Result<Vec<u8>>> = chunks
            .into_par_iter()
            .map(|chunk| decompress_chunk(chunk))
            .collect();

        // Calculate total size for pre-allocation
        let mut total_size = 0usize;
        for result in &results {
            match result {
                Ok(v) => total_size += v.len(),
                Err(_) => break,
            }
        }

        // Concatenate in order, propagating first error
        let mut output = Vec::with_capacity(total_size);
        for result in results {
            output.extend_from_slice(&result?);
        }

        self.total_decompressed = output.len() as u64;
        Ok(output)
    }
}

/// Decompress a single chunk (used by parallel decompression).
#[cfg(feature = "parallel")]
fn decompress_chunk(chunk: ReadChunk) -> Result<Vec<u8>> {
    if chunk.header.is_uncompressed() {
        return Ok(chunk.data);
    }

    let mut decoder = XzDecoder::new(&chunk.data[..]);
    let mut decompressed = Vec::with_capacity(chunk.header.uncompressed_size as usize);

    decoder.read_to_end(&mut decompressed).map_err(|e| {
        PbzxError::Decompression(format!(
            "Failed to decompress chunk at offset {}: {}",
            chunk.offset, e
        ))
    })?;

    if decompressed.len() as u64 != chunk.header.uncompressed_size {
        return Err(PbzxError::InvalidChunk {
            offset: chunk.offset,
            message: format!(
                "Decompressed size mismatch: expected {}, got {}",
                chunk.header.uncompressed_size,
                decompressed.len()
            ),
        });
    }

    Ok(decompressed)
}

impl<R: Read + Seek> PbzxReader<R> {
    /// Reset the reader to the beginning of the chunks.
    pub fn reset(&mut self) -> Result<()> {
        self.reader.seek(SeekFrom::Start(HEADER_SIZE as u64))?;
        self.current_offset = HEADER_SIZE as u64;
        self.total_decompressed = 0;
        Ok(())
    }

    /// Get information about all chunks without decompressing.
    pub fn chunk_info(&mut self) -> Result<Vec<ChunkInfo>> {
        self.reset()?;
        let mut chunks = Vec::new();
        let mut index = 0;

        while let Some(header) = self.read_chunk_header()? {
            let offset = self.current_offset;

            chunks.push(ChunkInfo {
                index,
                offset,
                compressed_size: header.compressed_size,
                uncompressed_size: header.uncompressed_size,
                is_compressed: !header.is_uncompressed(),
            });

            // Skip the chunk data
            self.reader
                .seek(SeekFrom::Current(header.compressed_size as i64))?;
            self.current_offset += header.compressed_size;
            index += 1;
        }

        self.reset()?;
        Ok(chunks)
    }
}

/// Information about a single chunk in the archive.
#[derive(Debug, Clone)]
pub struct ChunkInfo {
    /// Chunk index (0-based)
    pub index: usize,
    /// Offset of chunk data in the file
    pub offset: u64,
    /// Compressed size in bytes
    pub compressed_size: u64,
    /// Uncompressed size in bytes
    pub uncompressed_size: u64,
    /// Whether this chunk is compressed
    pub is_compressed: bool,
}

impl ChunkInfo {
    /// Get the compression ratio for this chunk.
    pub fn compression_ratio(&self) -> f64 {
        if self.uncompressed_size == 0 {
            1.0
        } else {
            self.compressed_size as f64 / self.uncompressed_size as f64
        }
    }
}

/// Open a PBZX file for reading.
///
/// This is a convenience function that opens a file and creates a PbzxReader.
///
/// # Example
///
/// ```no_run
/// use pbzx::open;
///
/// let mut reader = pbzx::open("Payload").unwrap();
/// let data = reader.decompress().unwrap();
/// ```
pub fn open<P: AsRef<Path>>(path: P) -> Result<PbzxReader<BufReader<File>>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    PbzxReader::new(reader)
}

/// Check if a file is a valid PBZX archive.
///
/// This only checks the magic bytes without reading the entire file.
pub fn is_pbzx<P: AsRef<Path>>(path: P) -> Result<bool> {
    let mut file = File::open(path)?;
    let mut magic = [0u8; 4];

    match file.read_exact(&mut magic) {
        Ok(()) => Ok(magic == PBZX_MAGIC),
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => Ok(false),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn create_minimal_pbzx() -> Vec<u8> {
        let mut data = Vec::new();
        // Magic
        data.extend_from_slice(&PBZX_MAGIC);
        // Flags (8 bytes, big-endian)
        data.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 1]);
        data
    }

    #[test]
    fn test_header_parsing() {
        let data = create_minimal_pbzx();
        let cursor = Cursor::new(data);
        let reader = PbzxReader::new(cursor).unwrap();

        assert!(reader.header().is_valid());
        assert_eq!(reader.flags(), 1);
    }

    #[test]
    fn test_invalid_magic() {
        let data = vec![0x00, 0x00, 0x00, 0x00, 0, 0, 0, 0, 0, 0, 0, 0];
        let cursor = Cursor::new(data);
        let result = PbzxReader::new(cursor);

        assert!(matches!(result, Err(PbzxError::InvalidMagic(_))));
    }
}

#[cfg(test)]
#[cfg(feature = "parallel")]
mod parallel_tests {
    use super::*;
    use std::io::Cursor;

    /// Create a multi-chunk PBZX archive for testing.
    fn create_multi_chunk_pbzx(chunk_size: usize) -> (Vec<u8>, Vec<u8>) {
        use crate::writer::{CpioBuilder, PbzxWriter};

        let mut cpio_builder = CpioBuilder::new();
        for i in 0..10 {
            let content = format!(
                "File {} content with enough data to generate multiple chunks: {}",
                i,
                "abcdefghijklmnopqrstuvwxyz ".repeat(20)
            );
            cpio_builder.add_file(&format!("file_{}.txt", i), content.as_bytes(), 0o644);
        }
        let cpio_data = cpio_builder.finish();

        let mut pbzx_data = Vec::new();
        let mut writer = PbzxWriter::new(&mut pbzx_data)
            .chunk_size(chunk_size)
            .compression_level(1);
        writer.write_cpio(&cpio_data).unwrap();
        writer.finish().unwrap();

        (pbzx_data, cpio_data)
    }

    #[test]
    fn test_parallel_matches_sequential() {
        let (pbzx_data, _) = create_multi_chunk_pbzx(256);

        // Sequential decompress
        let mut reader1 = PbzxReader::new(Cursor::new(&pbzx_data)).unwrap();
        let sequential = reader1.decompress().unwrap();

        // Parallel decompress
        let mut reader2 = PbzxReader::new(Cursor::new(&pbzx_data)).unwrap();
        let parallel = reader2.decompress_parallel().unwrap();

        assert_eq!(sequential, parallel);
    }

    #[test]
    fn test_parallel_single_chunk() {
        let (pbzx_data, cpio_data) = create_multi_chunk_pbzx(1024 * 1024);

        let mut reader = PbzxReader::new(Cursor::new(&pbzx_data)).unwrap();
        let result = reader.decompress_parallel().unwrap();

        assert_eq!(result, cpio_data);
    }

    #[test]
    fn test_parallel_empty_archive() {
        let mut data = Vec::new();
        data.extend_from_slice(&PBZX_MAGIC);
        data.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 1]); // flags

        let mut reader = PbzxReader::new(Cursor::new(data)).unwrap();
        let result = reader.decompress_parallel().unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_parallel_to_writer() {
        let (pbzx_data, _) = create_multi_chunk_pbzx(256);

        // Sequential
        let mut reader1 = PbzxReader::new(Cursor::new(&pbzx_data)).unwrap();
        let sequential = reader1.decompress().unwrap();

        // Parallel via decompress_parallel_to
        let mut reader2 = PbzxReader::new(Cursor::new(&pbzx_data)).unwrap();
        let mut output = Vec::new();
        reader2.decompress_parallel_to(&mut output).unwrap();

        assert_eq!(sequential, output);
    }
}
