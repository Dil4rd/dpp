use flate2::read::ZlibDecoder;
use std::io::{Read, Seek, SeekFrom, Write};

use crate::error::{XarError, Result};
use crate::toc::XarFile;

/// Read a file entry's data from the heap.
/// Returns number of bytes written to the writer.
pub fn read_entry<R: Read + Seek, W: Write>(
    reader: &mut R,
    heap_offset: u64,
    file: &XarFile,
    mut writer: W,
) -> Result<u64> {
    let data = match &file.data {
        Some(d) => d,
        None => return Ok(0), // Directory or entry with no data
    };

    // Seek to the entry in the heap
    let abs_offset = heap_offset + data.offset;
    reader.seek(SeekFrom::Start(abs_offset))?;

    // Read the compressed data
    let mut compressed = vec![0u8; data.length as usize];
    reader.read_exact(&mut compressed)?;

    // Decompress based on encoding
    match data.encoding.as_str() {
        "application/octet-stream" => {
            // Raw/uncompressed data
            writer.write_all(&compressed)?;
            Ok(data.length)
        }
        "application/x-gzip" => {
            let mut decoder = flate2::read::GzDecoder::new(&compressed[..]);
            let mut decompressed = Vec::with_capacity(data.size as usize);
            decoder.read_to_end(&mut decompressed)
                .map_err(|e| XarError::DecompressionFailed(format!("gzip: {}", e)))?;
            let len = decompressed.len() as u64;
            writer.write_all(&decompressed)?;
            Ok(len)
        }
        "application/x-bzip2" => {
            let decoded = bzip2_decode(&compressed)?;
            let len = decoded.len() as u64;
            writer.write_all(&decoded)?;
            Ok(len)
        }
        "application/zlib" | "application/x-zlib" => {
            let mut decoder = ZlibDecoder::new(&compressed[..]);
            let mut decompressed = Vec::with_capacity(data.size as usize);
            decoder.read_to_end(&mut decompressed)
                .map_err(|e| XarError::DecompressionFailed(format!("zlib: {}", e)))?;
            let len = decompressed.len() as u64;
            writer.write_all(&decompressed)?;
            Ok(len)
        }
        other => Err(XarError::UnsupportedEncoding(other.to_string())),
    }
}

/// Decompress bzip2 data (if bzip2 support is available)
fn bzip2_decode(_data: &[u8]) -> Result<Vec<u8>> {
    // Use flate2-style manual decompression approach
    // Since we don't want to add bzip2 dependency just for XAR,
    // return an error if bzip2 encoding is encountered
    Err(XarError::UnsupportedEncoding(
        "application/x-bzip2 (bzip2 not enabled)".to_string(),
    ))
}
