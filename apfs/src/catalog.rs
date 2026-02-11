use byteorder::{LittleEndian, ReadBytesExt};
use std::io::{Cursor, Read, Seek};

use crate::btree;
use crate::error::{ApfsError, Result};
use crate::{DirEntry, EntryKind};

// Catalog record types (j_obj_types), stored in top 4 bits of key's obj_id_and_type
pub const J_TYPE_SNAP_METADATA: u8 = 1;
pub const J_TYPE_EXTENT: u8 = 2;
pub const J_TYPE_INODE: u8 = 3;
pub const J_TYPE_XATTR: u8 = 4;
pub const J_TYPE_SIBLING_LINK: u8 = 5;
pub const J_TYPE_DSTREAM_ID: u8 = 6;
pub const J_TYPE_CRYPTO_STATE: u8 = 7;
pub const J_TYPE_FILE_EXTENT: u8 = 8;
pub const J_TYPE_DIR_REC: u8 = 9;
pub const J_TYPE_DIR_STATS: u8 = 10;
pub const J_TYPE_SNAP_NAME: u8 = 11;
pub const J_TYPE_SIBLING_MAP: u8 = 12;

// Well-known OIDs
pub const ROOT_DIR_PARENT: u64 = 1;   // Parent OID of root directory
pub const ROOT_DIR_RECORD: u64 = 2;   // OID of the root directory inode

// Inode types (from BSD mode)
pub const INODE_DIR_TYPE: u16 = 0o040000;      // S_IFDIR
pub const INODE_FILE_TYPE: u16 = 0o100000;     // S_IFREG
pub const INODE_SYMLINK_TYPE: u16 = 0o120000;  // S_IFLNK

// Extended field types (INO_EXT_TYPE_*)
const INO_EXT_TYPE_DSTREAM: u8 = 8;

/// Parsed inode value from a catalog record.
#[derive(Debug, Clone)]
pub struct InodeVal {
    pub parent_id: u64,
    pub private_id: u64,
    pub create_time: i64,
    pub modify_time: i64,
    pub change_time: i64,
    pub access_time: i64,
    pub internal_flags: u64,
    pub nchildren_or_nlink: i32,
    pub default_protection_class: u32,
    pub write_generation_counter: u32,
    pub bsd_flags: u32,
    pub uid: u32,
    pub gid: u32,
    pub mode: u16,
    pub pad1: u16,
    pub uncompressed_size: u64,
    /// Logical file size from the dstream xfield (if present).
    pub dstream_size: Option<u64>,
}

impl InodeVal {
    /// Fixed size of j_inode_val_t before xfields
    const FIXED_SIZE: usize = 92;

    /// Parse from raw catalog value bytes.
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < Self::FIXED_SIZE {
            return Err(ApfsError::CorruptedData(
                format!("inode value too short: {} bytes", data.len()),
            ));
        }
        let mut cursor = Cursor::new(data);
        let parent_id = cursor.read_u64::<LittleEndian>()?;
        let private_id = cursor.read_u64::<LittleEndian>()?;
        let create_time = cursor.read_i64::<LittleEndian>()?;
        let modify_time = cursor.read_i64::<LittleEndian>()?;
        let change_time = cursor.read_i64::<LittleEndian>()?;
        let access_time = cursor.read_i64::<LittleEndian>()?;
        let internal_flags = cursor.read_u64::<LittleEndian>()?;
        let nchildren_or_nlink = cursor.read_i32::<LittleEndian>()?;
        let default_protection_class = cursor.read_u32::<LittleEndian>()?;
        let write_generation_counter = cursor.read_u32::<LittleEndian>()?;
        let bsd_flags = cursor.read_u32::<LittleEndian>()?;
        let uid = cursor.read_u32::<LittleEndian>()?;
        let gid = cursor.read_u32::<LittleEndian>()?;
        let mode = cursor.read_u16::<LittleEndian>()?;
        let pad1 = cursor.read_u16::<LittleEndian>()?;
        let uncompressed_size = cursor.read_u64::<LittleEndian>()?;

        // Parse xfields for dstream size
        let dstream_size = Self::parse_dstream_size(&data[Self::FIXED_SIZE..]);

        Ok(InodeVal {
            parent_id,
            private_id,
            create_time,
            modify_time,
            change_time,
            access_time,
            internal_flags,
            nchildren_or_nlink,
            default_protection_class,
            write_generation_counter,
            bsd_flags,
            uid,
            gid,
            mode,
            pad1,
            uncompressed_size,
            dstream_size,
        })
    }

    /// Parse xfields to extract dstream size.
    /// Layout: xf_blob_t { xf_num_exts: u16, xf_used_data: u16 }
    /// followed by x_field_t[xf_num_exts] { x_type: u8, x_flags: u8, x_size: u16 }
    /// followed by the actual field data values (each padded to 8-byte alignment).
    fn parse_dstream_size(xfield_data: &[u8]) -> Option<u64> {
        if xfield_data.len() < 4 {
            return None;
        }
        let xf_num_exts = u16::from_le_bytes([xfield_data[0], xfield_data[1]]) as usize;
        if xf_num_exts == 0 {
            return None;
        }

        // x_field_t entries start at offset 4
        let entries_start = 4;
        let entries_end = entries_start + xf_num_exts * 4;
        if entries_end > xfield_data.len() {
            return None;
        }

        // Data values start immediately after the x_field_t array
        let mut data_offset = entries_end;

        for i in 0..xf_num_exts {
            let entry_off = entries_start + i * 4;
            let x_type = xfield_data[entry_off];
            let x_size = u16::from_le_bytes([xfield_data[entry_off + 2], xfield_data[entry_off + 3]]) as usize;

            if x_type == INO_EXT_TYPE_DSTREAM && x_size >= 8 && data_offset + 8 <= xfield_data.len() {
                // j_dstream_t.size is the first u64
                let size = u64::from_le_bytes([
                    xfield_data[data_offset],
                    xfield_data[data_offset + 1],
                    xfield_data[data_offset + 2],
                    xfield_data[data_offset + 3],
                    xfield_data[data_offset + 4],
                    xfield_data[data_offset + 5],
                    xfield_data[data_offset + 6],
                    xfield_data[data_offset + 7],
                ]);
                return Some(size);
            }

            // Advance past this field's data, padded to 8-byte boundary
            let padded_size = (x_size + 7) & !7;
            data_offset += padded_size;
        }

        None
    }

    /// Get the file type from the mode field
    pub fn kind(&self) -> u16 {
        self.mode & 0o170000
    }

    /// Get the logical file size.
    /// Prefers dstream size from xfields; falls back to uncompressed_size.
    pub fn size(&self) -> u64 {
        self.dstream_size.unwrap_or(self.uncompressed_size)
    }

    pub fn nlink(&self) -> u32 {
        self.nchildren_or_nlink as u32
    }
}

/// Directory record value (j_drec_val_t)
#[derive(Debug, Clone)]
pub struct DrecVal {
    pub file_id: u64,
    pub date_added: i64,
    pub flags: u16,
}

impl DrecVal {
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < 18 {
            return Err(ApfsError::CorruptedData(
                format!("drec value too short: {} bytes", data.len()),
            ));
        }
        let mut cursor = Cursor::new(data);
        let file_id = cursor.read_u64::<LittleEndian>()?;
        let date_added = cursor.read_i64::<LittleEndian>()?;
        let flags = cursor.read_u16::<LittleEndian>()?;

        Ok(DrecVal {
            file_id,
            date_added,
            flags,
        })
    }

    /// Get the file type from the flags field (DT_* from dirent.h)
    pub fn file_type(&self) -> u16 {
        self.flags & 0x000F
    }
}

// DT_* constants for directory entry types
pub const DT_REG: u16 = 8;    // Regular file
pub const DT_DIR: u16 = 4;    // Directory
pub const DT_LNK: u16 = 10;   // Symbolic link

/// File extent value (j_file_extent_val_t)
#[derive(Debug, Clone)]
pub struct FileExtentVal {
    pub flags_and_length: u64,
    pub phys_block_num: u64,
    pub crypto_id: u64,
}

impl FileExtentVal {
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < 24 {
            return Err(ApfsError::CorruptedData(
                format!("file extent value too short: {} bytes", data.len()),
            ));
        }
        let mut cursor = Cursor::new(data);
        let flags_and_length = cursor.read_u64::<LittleEndian>()?;
        let phys_block_num = cursor.read_u64::<LittleEndian>()?;
        let crypto_id = cursor.read_u64::<LittleEndian>()?;

        Ok(FileExtentVal {
            flags_and_length,
            phys_block_num,
            crypto_id,
        })
    }

    /// Get the logical length in bytes (lower 56 bits)
    pub fn length(&self) -> u64 {
        self.flags_and_length & 0x00FFFFFFFFFFFFFF
    }
}

/// Decode a catalog key: extract obj_id and type from the combined j_key_t.
fn decode_catalog_key(key_bytes: &[u8]) -> Result<(u64, u8)> {
    if key_bytes.len() < 8 {
        return Err(ApfsError::InvalidBTree("catalog key too short".into()));
    }
    let obj_id_and_type = u64::from_le_bytes([
        key_bytes[0], key_bytes[1], key_bytes[2], key_bytes[3],
        key_bytes[4], key_bytes[5], key_bytes[6], key_bytes[7],
    ]);

    let obj_id = obj_id_and_type & 0x0FFFFFFFFFFFFFFF;
    let j_type = ((obj_id_and_type >> 60) & 0xF) as u8;

    Ok((obj_id, j_type))
}

/// Extract the name from a directory record key (j_drec_hashed_key_t or j_drec_key_t).
/// After the 8-byte obj_id_and_type, there's a 4-byte name_len_and_hash (for hashed keys)
/// followed by the UTF-8 name.
fn decode_drec_name(key_bytes: &[u8]) -> Result<String> {
    if key_bytes.len() < 12 {
        return Err(ApfsError::InvalidBTree("drec key too short for name".into()));
    }

    // key[8..12]: name_len_and_hash (u32 LE)
    // name_len = lower 10 bits
    let name_len_and_hash = u32::from_le_bytes([key_bytes[8], key_bytes[9], key_bytes[10], key_bytes[11]]);
    let name_len = (name_len_and_hash & 0x000003FF) as usize;

    let name_start = 12;
    let name_end = name_start + name_len;

    if name_end > key_bytes.len() {
        return Err(ApfsError::InvalidBTree(
            format!("drec name extends beyond key: name_end={}, key_len={}", name_end, key_bytes.len()),
        ));
    }

    // Name is null-terminated UTF-8
    let name_bytes = &key_bytes[name_start..name_end];
    let nul_pos = name_bytes.iter().position(|&b| b == 0).unwrap_or(name_bytes.len());
    Ok(String::from_utf8_lossy(&name_bytes[..nul_pos]).to_string())
}

/// List directory entries for a given parent OID.
///
/// Scans the catalog B-tree for all J_TYPE_DIR_REC entries whose obj_id matches
/// the parent directory OID. For each, looks up the inode to get size/timestamps.
pub fn list_directory<R: Read + Seek>(
    reader: &mut R,
    catalog_root: u64,
    omap_root: u64,
    block_size: u32,
    parent_oid: u64,
) -> Result<Vec<DirEntry>> {
    // Catalog keys are sorted by OID first, then type within the same OID.
    let range_fn = |key: &[u8]| -> Option<bool> {
        match decode_catalog_key(key) {
            Ok((oid, j_type)) => {
                match compare_catalog_keys(oid, j_type, parent_oid, J_TYPE_DIR_REC) {
                    std::cmp::Ordering::Less => Some(false), // before target, keep scanning
                    std::cmp::Ordering::Equal => Some(true),  // match (DIR_REC entries have extra name data but oid+type matches)
                    std::cmp::Ordering::Greater => {
                        // For DIR_REC matching: same OID with type > DIR_REC, or higher OID
                        if oid == parent_oid && j_type == J_TYPE_DIR_REC {
                            Some(true) // shouldn't happen, but include
                        } else {
                            None // past our target, stop
                        }
                    }
                }
            }
            Err(_) => Some(false),
        }
    };

    let entries = btree::btree_scan(
        reader,
        catalog_root,
        block_size,
        0, 0, // variable-size keys and values
        &range_fn,
        Some(omap_root),
    )?;

    let mut dir_entries = Vec::new();
    for (key, val) in &entries {
        let name = match decode_drec_name(key) {
            Ok(n) => n,
            Err(_) => continue,
        };

        let drec = match DrecVal::parse(val) {
            Ok(d) => d,
            Err(_) => continue,
        };

        let kind = match drec.file_type() {
            DT_DIR => EntryKind::Directory,
            DT_LNK => EntryKind::Symlink,
            _ => EntryKind::File,
        };

        // Look up the inode for size/timestamps
        let (size, create_time, modify_time) = match lookup_inode(
            reader, catalog_root, omap_root, block_size, drec.file_id,
        ) {
            Ok(inode) => (inode.size(), inode.create_time, inode.modify_time),
            Err(_) => (0, 0, 0),
        };

        dir_entries.push(DirEntry {
            name,
            oid: drec.file_id,
            kind,
            size,
            create_time,
            modify_time,
        });
    }

    Ok(dir_entries)
}

/// Look up an inode record in the catalog B-tree.
pub fn lookup_inode<R: Read + Seek>(
    reader: &mut R,
    catalog_root: u64,
    omap_root: u64,
    block_size: u32,
    oid: u64,
) -> Result<InodeVal> {
    let compare_fn = |key: &[u8]| -> std::cmp::Ordering {
        match decode_catalog_key(key) {
            Ok((key_oid, key_type)) => {
                let search_oid = oid;
                let search_type = J_TYPE_INODE;
                match key_oid.cmp(&search_oid) {
                    std::cmp::Ordering::Equal => (key_type).cmp(&search_type),
                    ord => ord,
                }
            }
            Err(_) => std::cmp::Ordering::Less,
        }
    };

    let val = btree::btree_lookup(
        reader,
        catalog_root,
        block_size,
        0, 0,
        &compare_fn,
        Some(omap_root),
    )?;

    match val {
        Some(data) => InodeVal::parse(&data),
        None => Err(ApfsError::FileNotFound(format!("inode OID {}", oid))),
    }
}

/// Look up all file extent records for a given file OID (private_id).
pub fn lookup_extents<R: Read + Seek>(
    reader: &mut R,
    catalog_root: u64,
    omap_root: u64,
    block_size: u32,
    file_oid: u64,
) -> Result<Vec<FileExtentVal>> {
    let range_fn = |key: &[u8]| -> Option<bool> {
        match decode_catalog_key(key) {
            Ok((oid, j_type)) => {
                if oid == file_oid && j_type == J_TYPE_FILE_EXTENT {
                    Some(true) // match
                } else {
                    match compare_catalog_keys(oid, j_type, file_oid, J_TYPE_FILE_EXTENT) {
                        std::cmp::Ordering::Less => Some(false), // before target, skip
                        std::cmp::Ordering::Greater => None,     // past target, stop
                        std::cmp::Ordering::Equal => Some(true), // shouldn't reach here
                    }
                }
            }
            Err(_) => Some(false),
        }
    };

    let entries = btree::btree_scan(
        reader,
        catalog_root,
        block_size,
        0, 0,
        &range_fn,
        Some(omap_root),
    )?;

    let mut extents = Vec::new();
    for (_key, val) in &entries {
        extents.push(FileExtentVal::parse(val)?);
    }

    Ok(extents)
}

/// Resolve a path like "/Applications/Upscayl.app/Contents/Info.plist" to its (OID, InodeVal).
pub fn resolve_path<R: Read + Seek>(
    reader: &mut R,
    catalog_root: u64,
    omap_root: u64,
    block_size: u32,
    path: &str,
) -> Result<(u64, InodeVal)> {
    let path = path.trim_matches('/');

    if path.is_empty() {
        // Root directory
        let inode = lookup_inode(reader, catalog_root, omap_root, block_size, ROOT_DIR_RECORD)?;
        return Ok((ROOT_DIR_RECORD, inode));
    }

    let components: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let mut current_parent = ROOT_DIR_RECORD;

    for (i, component) in components.iter().enumerate() {
        // Look up the directory record for this component under current_parent
        let drec = lookup_drec(reader, omap_root, catalog_root, block_size, current_parent, component)?;

        if i == components.len() - 1 {
            // Final component — look up its inode
            let inode = lookup_inode(reader, catalog_root, omap_root, block_size, drec.file_id)?;
            return Ok((drec.file_id, inode));
        }

        // Not the final component — it must be a directory
        if drec.file_type() != DT_DIR {
            return Err(ApfsError::NotADirectory(
                components[..=i].join("/"),
            ));
        }

        current_parent = drec.file_id;
    }

    unreachable!()
}

/// Look up a specific directory record by name under a parent OID.
fn lookup_drec<R: Read + Seek>(
    reader: &mut R,
    omap_root: u64,
    catalog_root: u64,
    block_size: u32,
    parent_oid: u64,
    name: &str,
) -> Result<DrecVal> {
    // Scan all DRECs for this parent and find the one with matching name
    let range_fn = |key: &[u8]| -> Option<bool> {
        match decode_catalog_key(key) {
            Ok((oid, j_type)) => {
                if oid == parent_oid && j_type == J_TYPE_DIR_REC {
                    Some(true)
                } else {
                    match compare_catalog_keys(oid, j_type, parent_oid, J_TYPE_DIR_REC) {
                        std::cmp::Ordering::Less => Some(false),
                        std::cmp::Ordering::Greater => None,
                        std::cmp::Ordering::Equal => Some(true),
                    }
                }
            }
            Err(_) => Some(false),
        }
    };

    let entries = btree::btree_scan(
        reader,
        catalog_root,
        block_size,
        0, 0,
        &range_fn,
        Some(omap_root),
    )?;

    for (key, val) in &entries {
        if let Ok(entry_name) = decode_drec_name(key) {
            if entry_name == name {
                return DrecVal::parse(val);
            }
        }
    }

    Err(ApfsError::FileNotFound(name.to_string()))
}

/// Compare two catalog keys in APFS sort order: OID first, then type.
/// Returns the ordering of (oid_a, type_a) vs (oid_b, type_b).
fn compare_catalog_keys(oid_a: u64, type_a: u8, oid_b: u64, type_b: u8) -> std::cmp::Ordering {
    match oid_a.cmp(&oid_b) {
        std::cmp::Ordering::Equal => type_a.cmp(&type_b),
        ord => ord,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::superblock;
    use crate::omap as omap_mod;
    use std::io::BufReader;

    fn open_volume() -> (BufReader<std::fs::File>, u64, u64, u32) {
        let file = std::fs::File::open("../tests/appfs.raw").unwrap();
        let mut reader = BufReader::new(file);

        let nxsb = superblock::read_nxsb(&mut reader).unwrap();
        let latest = superblock::find_latest_nxsb(&mut reader, &nxsb).unwrap();
        let block_size = latest.block_size;

        let container_omap_root = omap_mod::read_omap_tree_root(&mut reader, latest.omap_oid, block_size).unwrap();

        let vol_oid = latest.fs_oids.iter().find(|&&o| o != 0).copied().unwrap();
        let vol_block = omap_mod::omap_lookup(&mut reader, container_omap_root, block_size, vol_oid).unwrap();

        let vol_data = crate::object::read_block(&mut reader, vol_block, block_size).unwrap();
        let vol_sb = superblock::ApfsSuperblock::parse(&vol_data).unwrap();

        let vol_omap_root = omap_mod::read_omap_tree_root(&mut reader, vol_sb.omap_oid, block_size).unwrap();
        let catalog_root = omap_mod::omap_lookup(&mut reader, vol_omap_root, block_size, vol_sb.root_tree_oid).unwrap();

        (reader, catalog_root, vol_omap_root, block_size)
    }

    /// Requires ../tests/appfs.raw fixture. Run with `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn test_list_root() {
        let (mut reader, catalog_root, omap_root, block_size) = open_volume();

        let entries = list_directory(&mut reader, catalog_root, omap_root, block_size, ROOT_DIR_RECORD).unwrap();
        assert!(!entries.is_empty(), "Root directory should have entries");
    }

    /// Requires ../tests/appfs.raw fixture. Run with `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn test_resolve_path() {
        let (mut reader, catalog_root, omap_root, block_size) = open_volume();

        let entries = list_directory(&mut reader, catalog_root, omap_root, block_size, ROOT_DIR_RECORD).unwrap();
        let first = entries.first().expect("Root should have entries");
        let path = format!("/{}", first.name);
        let (oid, inode) = resolve_path(&mut reader, catalog_root, omap_root, block_size, &path).unwrap();
        assert!(oid > 0);
        assert!(inode.kind() != 0);
    }

    #[test]
    fn test_drec_val_parse() {
        // Construct DrecVal bytes: file_id(u64) + date_added(i64) + flags(u16)
        let mut data = Vec::new();
        data.extend_from_slice(&42u64.to_le_bytes());       // file_id = 42
        data.extend_from_slice(&1000i64.to_le_bytes());     // date_added = 1000
        data.extend_from_slice(&(DT_DIR as u16).to_le_bytes()); // flags = DT_DIR (4)

        let drec = DrecVal::parse(&data).unwrap();
        assert_eq!(drec.file_id, 42);
        assert_eq!(drec.date_added, 1000);
        assert_eq!(drec.file_type(), DT_DIR);
    }

    #[test]
    fn test_file_extent_val_parse() {
        // Construct FileExtentVal bytes: flags_and_length(u64) + phys_block_num(u64) + crypto_id(u64)
        // length() masks with lower 56 bits (0x00FFFFFFFFFFFFFF)
        let flags_and_length: u64 = 0xAB00_0000_0000_1000; // upper byte = flags 0xAB, lower 56 = 0x1000
        let mut data = Vec::new();
        data.extend_from_slice(&flags_and_length.to_le_bytes());
        data.extend_from_slice(&100u64.to_le_bytes());  // phys_block_num = 100
        data.extend_from_slice(&0u64.to_le_bytes());    // crypto_id = 0

        let extent = FileExtentVal::parse(&data).unwrap();
        assert_eq!(extent.length(), 0x1000);
        assert_eq!(extent.phys_block_num, 100);
        assert_eq!(extent.crypto_id, 0);
    }
}
