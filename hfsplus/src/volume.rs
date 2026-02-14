use byteorder::{BigEndian, ReadBytesExt};
use std::io::{Read, Seek, SeekFrom};

use crate::error::{HfsPlusError, Result};

/// HFS+ volume header offset from the start of the partition
pub const VOLUME_HEADER_OFFSET: u64 = 1024;

/// HFS+ signature: "H+" (0x482B)
pub const HFS_PLUS_SIGNATURE: u16 = 0x482B;

/// HFSX signature: "HX" (0x4858) — case-sensitive variant
pub const HFSX_SIGNATURE: u16 = 0x4858;

/// HFS+ volume header version
pub const HFS_PLUS_VERSION: u16 = 4;
pub const HFSX_VERSION: u16 = 5;

/// An extent descriptor: contiguous range of allocation blocks
#[derive(Debug, Clone, Copy, Default)]
pub struct ExtentDescriptor {
    pub start_block: u32,
    pub block_count: u32,
}

/// Fork data: describes a data or resource fork
#[derive(Debug, Clone)]
#[derive(Default)]
pub struct ForkData {
    pub logical_size: u64,
    pub clump_size: u32,
    pub total_blocks: u32,
    pub extents: [ExtentDescriptor; 8],
}


/// The HFS+ Volume Header (512 bytes at offset 1024)
#[derive(Debug, Clone)]
pub struct VolumeHeader {
    pub signature: u16,
    pub version: u16,
    pub attributes: u32,
    pub last_mounted_version: u32,
    pub journal_info_block: u32,
    pub create_date: u32,
    pub modify_date: u32,
    pub backup_date: u32,
    pub checked_date: u32,
    pub file_count: u32,
    pub folder_count: u32,
    pub block_size: u32,
    pub total_blocks: u32,
    pub free_blocks: u32,
    pub next_allocation: u32,
    pub rsrc_clump_size: u32,
    pub data_clump_size: u32,
    pub next_catalog_id: u32,
    pub write_count: u32,
    pub encoding_bitmap: u64,
    pub finder_info: [u32; 8],
    pub allocation_file: ForkData,
    pub extents_file: ForkData,
    pub catalog_file: ForkData,
    pub attributes_file: ForkData,
    pub startup_file: ForkData,
    /// true if this is HFSX (case-sensitive)
    pub is_hfsx: bool,
}

fn read_extent_descriptor<R: Read>(reader: &mut R) -> Result<ExtentDescriptor> {
    Ok(ExtentDescriptor {
        start_block: reader.read_u32::<BigEndian>()?,
        block_count: reader.read_u32::<BigEndian>()?,
    })
}

fn read_fork_data<R: Read>(reader: &mut R) -> Result<ForkData> {
    let logical_size = reader.read_u64::<BigEndian>()?;
    let clump_size = reader.read_u32::<BigEndian>()?;
    let total_blocks = reader.read_u32::<BigEndian>()?;
    let mut extents = [ExtentDescriptor::default(); 8];
    for extent in &mut extents {
        *extent = read_extent_descriptor(reader)?;
    }
    Ok(ForkData {
        logical_size,
        clump_size,
        total_blocks,
        extents,
    })
}

impl VolumeHeader {
    /// Parse the volume header from a reader positioned at the start of the partition
    pub fn parse<R: Read + Seek>(reader: &mut R) -> Result<Self> {
        reader.seek(SeekFrom::Start(VOLUME_HEADER_OFFSET))?;

        let signature = reader.read_u16::<BigEndian>()?;
        let is_hfsx = match signature {
            HFS_PLUS_SIGNATURE => false,
            HFSX_SIGNATURE => true,
            _ => return Err(HfsPlusError::InvalidSignature(signature)),
        };

        let version = reader.read_u16::<BigEndian>()?;
        match version {
            HFS_PLUS_VERSION | HFSX_VERSION => {}
            _ => return Err(HfsPlusError::UnsupportedVersion(version)),
        }

        let attributes = reader.read_u32::<BigEndian>()?;
        let last_mounted_version = reader.read_u32::<BigEndian>()?;
        let journal_info_block = reader.read_u32::<BigEndian>()?;
        let create_date = reader.read_u32::<BigEndian>()?;
        let modify_date = reader.read_u32::<BigEndian>()?;
        let backup_date = reader.read_u32::<BigEndian>()?;
        let checked_date = reader.read_u32::<BigEndian>()?;
        let file_count = reader.read_u32::<BigEndian>()?;
        let folder_count = reader.read_u32::<BigEndian>()?;
        let block_size = reader.read_u32::<BigEndian>()?;
        let total_blocks = reader.read_u32::<BigEndian>()?;
        let free_blocks = reader.read_u32::<BigEndian>()?;
        let next_allocation = reader.read_u32::<BigEndian>()?;
        let rsrc_clump_size = reader.read_u32::<BigEndian>()?;
        let data_clump_size = reader.read_u32::<BigEndian>()?;
        let next_catalog_id = reader.read_u32::<BigEndian>()?;
        let write_count = reader.read_u32::<BigEndian>()?;
        let encoding_bitmap = reader.read_u64::<BigEndian>()?;

        let mut finder_info = [0u32; 8];
        for fi in &mut finder_info {
            *fi = reader.read_u32::<BigEndian>()?;
        }

        let allocation_file = read_fork_data(reader)?;
        let extents_file = read_fork_data(reader)?;
        let catalog_file = read_fork_data(reader)?;
        let attributes_file = read_fork_data(reader)?;
        let startup_file = read_fork_data(reader)?;

        Ok(VolumeHeader {
            signature,
            version,
            attributes,
            last_mounted_version,
            journal_info_block,
            create_date,
            modify_date,
            backup_date,
            checked_date,
            file_count,
            folder_count,
            block_size,
            total_blocks,
            free_blocks,
            next_allocation,
            rsrc_clump_size,
            data_clump_size,
            next_catalog_id,
            write_count,
            encoding_bitmap,
            finder_info,
            allocation_file,
            extents_file,
            catalog_file,
            attributes_file,
            startup_file,
            is_hfsx,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Requires ../tests/kdk.raw fixture. Run with `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn test_parse_kdk_volume_header() {
        let file = std::fs::File::open("../tests/kdk.raw").unwrap();
        let mut reader = std::io::BufReader::new(file);
        let header = VolumeHeader::parse(&mut reader).unwrap();

        assert!(header.is_hfsx, "kdk.raw should be HFSX");
        assert_eq!(header.signature, HFSX_SIGNATURE);
        assert_eq!(header.version, HFSX_VERSION);
        assert!(header.block_size > 0);
        assert!(header.total_blocks > 0);
        assert!(header.file_count > 0);
        assert!(header.folder_count > 0);
        assert!(header.catalog_file.logical_size > 0);
    }
}
