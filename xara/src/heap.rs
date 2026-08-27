use flate2::read::ZlibDecoder;
use std::io::{self, Read, Seek, SeekFrom, Write};

use crate::error::{Result, XarError};
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

    // Seek to the entry in the heap.
    let abs_offset = heap_offset.checked_add(data.offset).ok_or_else(|| {
        XarError::InvalidToc(
            "XAR heap entry offset overflows the archive address space".to_string(),
        )
    })?;
    reader.seek(SeekFrom::Start(abs_offset))?;

    // Decompress based on encoding
    match data.encoding.as_str() {
        "application/octet-stream" => {
            if data.length != data.size {
                return Err(XarError::InvalidToc(format!(
                    "XAR raw entry declares compressed length {} but uncompressed size {}",
                    data.length, data.size
                )));
            }
            let written = io::copy(&mut reader.take(data.length), &mut writer)?;
            if written != data.size {
                return Err(XarError::InvalidToc(format!(
                    "XAR raw entry ended after {written} bytes, expected {}",
                    data.size
                )));
            }
            Ok(written)
        }
        "application/x-gzip" => {
            let decoder = flate2::read::GzDecoder::new(reader.take(data.length));
            copy_decoded(decoder, &mut writer, data.size, "gzip")
        }
        "application/x-bzip2" => bzip2_decode(),
        "application/zlib" | "application/x-zlib" => {
            let decoder = ZlibDecoder::new(reader.take(data.length));
            copy_decoded(decoder, &mut writer, data.size, "zlib")
        }
        other => Err(XarError::UnsupportedEncoding(other.to_string())),
    }
}

fn copy_decoded<R: Read, W: Write>(
    mut decoder: R,
    writer: &mut W,
    expected: u64,
    format: &str,
) -> Result<u64> {
    const BUFFER_SIZE: usize = 8 * 1024;

    let mut written = 0_u64;
    let mut buffer = [0_u8; BUFFER_SIZE];
    while written < expected {
        let remaining = expected - written;
        let read_limit = buffer
            .len()
            .min(usize::try_from(remaining).unwrap_or(usize::MAX));
        let read = decoder
            .read(&mut buffer[..read_limit])
            .map_err(|e| XarError::DecompressionFailed(format!("{format}: {e}")))?;
        if read == 0 {
            break;
        }
        writer.write_all(&buffer[..read])?;
        let read = u64::try_from(read).map_err(|_| {
            XarError::InvalidToc("decoded XAR byte count does not fit u64".to_string())
        })?;
        written = written.checked_add(read).ok_or_else(|| {
            XarError::InvalidToc("decoded XAR byte count overflows u64".to_string())
        })?;
    }
    if written != expected {
        return Err(XarError::DecompressionFailed(format!(
            "{format} decoded {written} bytes, expected {expected}"
        )));
    }

    let mut extra = [0_u8; 1];
    let extra = decoder
        .read(&mut extra)
        .map_err(|e| XarError::DecompressionFailed(format!("{format}: {e}")))?;
    if extra != 0 {
        return Err(XarError::DecompressionFailed(format!(
            "{format} decoded more than the expected {expected} bytes"
        )));
    }
    Ok(written)
}

/// Decompress bzip2 data (if bzip2 support is available)
fn bzip2_decode() -> Result<u64> {
    // Use flate2-style manual decompression approach
    // Since we don't want to add bzip2 dependency just for XAR,
    // return an error if bzip2 encoding is encountered
    Err(XarError::UnsupportedEncoding(
        "application/x-bzip2 (bzip2 not enabled)".to_string(),
    ))
}
