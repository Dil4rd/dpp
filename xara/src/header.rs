use byteorder::{BigEndian, ReadBytesExt};
use std::io::Read;

use crate::error::{XarError, Result};

/// XAR magic number: "xar!" (0x78617221)
pub const XAR_MAGIC: u32 = 0x78617221;

/// Checksum algorithms
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChecksumAlgo {
    None,
    Sha1,
    Md5,
    Sha256,
    Unknown(u32),
}

impl From<u32> for ChecksumAlgo {
    fn from(v: u32) -> Self {
        match v {
            0 => ChecksumAlgo::None,
            1 => ChecksumAlgo::Sha1,
            2 => ChecksumAlgo::Md5,
            3 => ChecksumAlgo::Sha256,
            other => ChecksumAlgo::Unknown(other),
        }
    }
}

/// XAR archive header (28 bytes)
#[derive(Debug, Clone)]
pub struct XarHeader {
    pub magic: u32,
    pub header_size: u16,
    pub version: u16,
    pub toc_compressed_len: u64,
    pub toc_uncompressed_len: u64,
    pub checksum_algo: ChecksumAlgo,
}

/// Parse the XAR header from a reader
pub fn parse_header<R: Read>(reader: &mut R) -> Result<XarHeader> {
    let magic = reader.read_u32::<BigEndian>()?;
    if magic != XAR_MAGIC {
        return Err(XarError::InvalidMagic(magic));
    }

    let header_size = reader.read_u16::<BigEndian>()?;
    let version = reader.read_u16::<BigEndian>()?;
    let toc_compressed_len = reader.read_u64::<BigEndian>()?;
    let toc_uncompressed_len = reader.read_u64::<BigEndian>()?;
    let checksum_algo = ChecksumAlgo::from(reader.read_u32::<BigEndian>()?);

    // Skip any extra header bytes beyond the 28 we read
    if header_size > 28 {
        let extra = header_size as usize - 28;
        let mut skip_buf = vec![0u8; extra];
        reader.read_exact(&mut skip_buf)?;
    }

    Ok(XarHeader {
        magic,
        header_size,
        version,
        toc_compressed_len,
        toc_uncompressed_len,
        checksum_algo,
    })
}
