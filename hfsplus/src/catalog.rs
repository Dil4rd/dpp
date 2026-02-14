use byteorder::{BigEndian, ReadBytesExt};
use std::io::{Cursor, Read, Seek};

use crate::btree::{self, BTreeHeaderRecord};
use crate::error::{HfsPlusError, Result};
use crate::unicode;
use crate::volume::{ExtentDescriptor, ForkData, VolumeHeader};
use crate::{DirEntry, EntryKind};

/// Well-known Catalog Node IDs
pub const CNID_ROOT_PARENT: u32 = 1;
pub const CNID_ROOT_FOLDER: u32 = 2;
pub const CNID_EXTENTS_FILE: u32 = 3;
pub const CNID_CATALOG_FILE: u32 = 4;
pub const CNID_BAD_BLOCKS_FILE: u32 = 5;
pub const CNID_ALLOCATION_FILE: u32 = 6;
pub const CNID_STARTUP_FILE: u32 = 7;
pub const CNID_ATTRIBUTES_FILE: u32 = 8;

/// Catalog record types
pub const RECORD_TYPE_FOLDER: u16 = 0x0001;
pub const RECORD_TYPE_FILE: u16 = 0x0002;
pub const RECORD_TYPE_FOLDER_THREAD: u16 = 0x0003;
pub const RECORD_TYPE_FILE_THREAD: u16 = 0x0004;

/// BSD permissions
#[derive(Debug, Clone)]
pub struct HfsPlusBsdInfo {
    pub owner_id: u32,
    pub group_id: u32,
    pub admin_flags: u8,
    pub owner_flags: u8,
    pub file_mode: u16,
    pub special: u32,
}

/// Catalog file record
#[derive(Debug, Clone)]
pub struct CatalogFile {
    pub file_id: u32,
    pub create_date: u32,
    pub content_mod_date: u32,
    pub attribute_mod_date: u32,
    pub access_date: u32,
    pub backup_date: u32,
    pub permissions: HfsPlusBsdInfo,
    pub data_fork: ForkData,
    pub resource_fork: ForkData,
    pub text_encoding: u32,
}

/// Catalog folder record
#[derive(Debug, Clone)]
pub struct CatalogFolder {
    pub folder_id: u32,
    pub create_date: u32,
    pub content_mod_date: u32,
    pub attribute_mod_date: u32,
    pub access_date: u32,
    pub backup_date: u32,
    pub permissions: HfsPlusBsdInfo,
    pub valence: u32,
    pub text_encoding: u32,
}

/// Catalog thread record (points back to the parent)
#[derive(Debug, Clone)]
pub struct CatalogThread {
    pub parent_id: u32,
    pub node_name: String,
}

/// Parsed catalog record
#[derive(Debug, Clone)]
pub enum CatalogRecord {
    Folder(CatalogFolder),
    File(CatalogFile),
    FolderThread(CatalogThread),
    FileThread(CatalogThread),
}

/// Catalog key: (parentID, nodeName)
#[derive(Debug, Clone)]
pub struct CatalogKey {
    pub parent_id: u32,
    pub node_name: Vec<u16>, // UTF-16 code points
}

/// Parse a catalog key from raw record data.
/// Returns (key, remaining_data_offset) where remaining_data_offset points to the record data after the key.
fn parse_catalog_key(data: &[u8]) -> Result<(CatalogKey, usize)> {
    if data.len() < 6 {
        return Err(HfsPlusError::InvalidBTree("catalog key too short".into()));
    }

    let key_length = u16::from_be_bytes([data[0], data[1]]) as usize;
    let parent_id = u32::from_be_bytes([data[2], data[3], data[4], data[5]]);
    let name_length = u16::from_be_bytes([data[6], data[7]]) as usize;

    let name_start = 8;
    let name_end = name_start + name_length * 2;
    if name_end > data.len() {
        return Err(HfsPlusError::InvalidBTree(
            format!("catalog key name extends beyond data: name_end={}, data_len={}", name_end, data.len()),
        ));
    }

    let node_name = unicode::utf16be_to_u16(&data[name_start..name_end]);

    // Record data starts after key_length + 2 bytes for the key_length field itself
    let record_offset = 2 + key_length;
    // Ensure even alignment
    let record_offset = if !record_offset.is_multiple_of(2) { record_offset + 1 } else { record_offset };

    Ok((
        CatalogKey {
            parent_id,
            node_name,
        },
        record_offset,
    ))
}

fn parse_bsd_info(cursor: &mut Cursor<&[u8]>) -> Result<HfsPlusBsdInfo> {
    Ok(HfsPlusBsdInfo {
        owner_id: cursor.read_u32::<BigEndian>()?,
        group_id: cursor.read_u32::<BigEndian>()?,
        admin_flags: cursor.read_u8()?,
        owner_flags: cursor.read_u8()?,
        file_mode: cursor.read_u16::<BigEndian>()?,
        special: cursor.read_u32::<BigEndian>()?,
    })
}

fn read_extent_descriptor(cursor: &mut Cursor<&[u8]>) -> Result<ExtentDescriptor> {
    Ok(ExtentDescriptor {
        start_block: cursor.read_u32::<BigEndian>()?,
        block_count: cursor.read_u32::<BigEndian>()?,
    })
}

fn parse_fork_data(cursor: &mut Cursor<&[u8]>) -> Result<ForkData> {
    let logical_size = cursor.read_u64::<BigEndian>()?;
    let clump_size = cursor.read_u32::<BigEndian>()?;
    let total_blocks = cursor.read_u32::<BigEndian>()?;
    let mut extents = [ExtentDescriptor::default(); 8];
    for extent in &mut extents {
        *extent = read_extent_descriptor(cursor)?;
    }
    Ok(ForkData {
        logical_size,
        clump_size,
        total_blocks,
        extents,
    })
}

/// Parse a catalog record from raw data (after the key)
fn parse_catalog_record(data: &[u8]) -> Result<CatalogRecord> {
    if data.len() < 2 {
        return Err(HfsPlusError::InvalidBTree("catalog record too short".into()));
    }

    let record_type = u16::from_be_bytes([data[0], data[1]]);
    let mut cursor = Cursor::new(data);
    cursor.set_position(2);

    match record_type {
        RECORD_TYPE_FOLDER => {
            let _flags = cursor.read_u16::<BigEndian>()?;
            let valence = cursor.read_u32::<BigEndian>()?;
            let folder_id = cursor.read_u32::<BigEndian>()?;
            let create_date = cursor.read_u32::<BigEndian>()?;
            let content_mod_date = cursor.read_u32::<BigEndian>()?;
            let attribute_mod_date = cursor.read_u32::<BigEndian>()?;
            let access_date = cursor.read_u32::<BigEndian>()?;
            let backup_date = cursor.read_u32::<BigEndian>()?;
            let permissions = parse_bsd_info(&mut cursor)?;
            // Skip user info (16 bytes) and finder info (16 bytes)
            let mut _skip = [0u8; 32];
            cursor.read_exact(&mut _skip)?;
            let text_encoding = cursor.read_u32::<BigEndian>()?;

            Ok(CatalogRecord::Folder(CatalogFolder {
                folder_id,
                create_date,
                content_mod_date,
                attribute_mod_date,
                access_date,
                backup_date,
                permissions,
                valence,
                text_encoding,
            }))
        }
        RECORD_TYPE_FILE => {
            let _flags = cursor.read_u16::<BigEndian>()?;
            let _reserved = cursor.read_u32::<BigEndian>()?;
            let file_id = cursor.read_u32::<BigEndian>()?;
            let create_date = cursor.read_u32::<BigEndian>()?;
            let content_mod_date = cursor.read_u32::<BigEndian>()?;
            let attribute_mod_date = cursor.read_u32::<BigEndian>()?;
            let access_date = cursor.read_u32::<BigEndian>()?;
            let backup_date = cursor.read_u32::<BigEndian>()?;
            let permissions = parse_bsd_info(&mut cursor)?;
            // Skip user info (16 bytes) and finder info (16 bytes)
            let mut _skip = [0u8; 32];
            cursor.read_exact(&mut _skip)?;
            let text_encoding = cursor.read_u32::<BigEndian>()?;
            let _reserved2 = cursor.read_u32::<BigEndian>()?;
            let data_fork = parse_fork_data(&mut cursor)?;
            let resource_fork = parse_fork_data(&mut cursor)?;

            Ok(CatalogRecord::File(CatalogFile {
                file_id,
                create_date,
                content_mod_date,
                attribute_mod_date,
                access_date,
                backup_date,
                permissions,
                data_fork,
                resource_fork,
                text_encoding,
            }))
        }
        RECORD_TYPE_FOLDER_THREAD | RECORD_TYPE_FILE_THREAD => {
            let _reserved = cursor.read_u16::<BigEndian>()?;
            let parent_id = cursor.read_u32::<BigEndian>()?;
            let name_length = cursor.read_u16::<BigEndian>()? as usize;
            let mut name_buf = vec![0u8; name_length * 2];
            cursor.read_exact(&mut name_buf)?;
            let name_u16 = unicode::utf16be_to_u16(&name_buf);
            let node_name = unicode::utf16_to_string(&name_u16);

            let record = CatalogThread {
                parent_id,
                node_name,
            };

            if record_type == RECORD_TYPE_FOLDER_THREAD {
                Ok(CatalogRecord::FolderThread(record))
            } else {
                Ok(CatalogRecord::FileThread(record))
            }
        }
        other => Err(HfsPlusError::InvalidBTree(
            format!("unknown catalog record type: 0x{:04X}", other),
        )),
    }
}

/// Compare a catalog key in a B-tree record against a target (parent_id, name).
/// For HFSX: binary name comparison. For HFS+: case-insensitive.
fn make_catalog_comparator(
    target_parent_id: u32,
    target_name: &[u16],
    is_hfsx: bool,
) -> impl Fn(&[u8]) -> std::cmp::Ordering + '_ {
    move |record_data: &[u8]| {
        let (key, _) = match parse_catalog_key(record_data) {
            Ok(k) => k,
            Err(_) => return std::cmp::Ordering::Less,
        };

        match key.parent_id.cmp(&target_parent_id) {
            std::cmp::Ordering::Equal => {}
            ord => return ord,
        }

        if is_hfsx {
            unicode::compare_binary(&key.node_name, target_name)
        } else {
            unicode::compare_case_insensitive(&key.node_name, target_name)
        }
    }
}

/// Look up a catalog record by (parent_id, name)
pub fn lookup_catalog<R: Read + Seek>(
    reader: &mut R,
    vol: &VolumeHeader,
    btree_header: &BTreeHeaderRecord,
    parent_id: u32,
    name: &str,
) -> Result<Option<CatalogRecord>> {
    let name_u16 = unicode::string_to_utf16(name);
    let comparator = make_catalog_comparator(parent_id, &name_u16, vol.is_hfsx);

    match btree::search_btree(reader, btree_header, &comparator)? {
        Some((node, record_idx)) => {
            let record_data = node.record_data(record_idx)?;
            let (_, record_offset) = parse_catalog_key(record_data)?;
            if record_offset >= record_data.len() {
                return Err(HfsPlusError::InvalidBTree("record data missing after key".into()));
            }
            let record = parse_catalog_record(&record_data[record_offset..])?;
            Ok(Some(record))
        }
        None => Ok(None),
    }
}

/// List all entries in a directory (by parent CNID)
pub fn list_directory<R: Read + Seek>(
    reader: &mut R,
    vol: &VolumeHeader,
    btree_header: &BTreeHeaderRecord,
    parent_cnid: u32,
) -> Result<Vec<DirEntry>> {
    // We need to find the first leaf node that could contain records for parent_cnid.
    // Strategy: search for (parent_cnid, "") which will land at the first record
    // for this parent, then scan forward collecting all records with this parent.
    let empty_name: Vec<u16> = vec![];
    let comparator = make_catalog_comparator(parent_cnid, &empty_name, vol.is_hfsx);

    // Find starting position: search for the parent_cnid with empty name
    // This should land us at or before the first record for this parent
    let start_node = find_leaf_for_parent(reader, btree_header, parent_cnid, &comparator)?;

    if start_node == 0 {
        return Ok(Vec::new());
    }

    // Scan through leaf nodes collecting all records with matching parent_id
    let match_fn = |record_data: &[u8]| -> Option<bool> {
        match parse_catalog_key(record_data) {
            Ok((key, _)) => {
                if key.parent_id < parent_cnid {
                    Some(false) // skip, keep scanning
                } else if key.parent_id == parent_cnid {
                    Some(true) // match
                } else {
                    None // past our parent, stop
                }
            }
            Err(_) => Some(false),
        }
    };

    let parse_fn = |record_data: &[u8]| -> Result<Option<DirEntry>> {
        let (key, record_offset) = parse_catalog_key(record_data)?;
        if record_offset >= record_data.len() {
            return Ok(None);
        }
        let record = parse_catalog_record(&record_data[record_offset..])?;
        let name = unicode::utf16_to_string(&key.node_name);

        match record {
            CatalogRecord::Folder(f) => Ok(Some(DirEntry {
                name,
                cnid: f.folder_id,
                kind: EntryKind::Directory,
                size: 0,
                create_date: f.create_date,
                modify_date: f.content_mod_date,
            })),
            CatalogRecord::File(f) => {
                let kind = if f.permissions.file_mode & 0o170000 == 0o120000 {
                    EntryKind::Symlink
                } else {
                    EntryKind::File
                };
                Ok(Some(DirEntry {
                    name,
                    cnid: f.file_id,
                    kind,
                    size: f.data_fork.logical_size,
                    create_date: f.create_date,
                    modify_date: f.content_mod_date,
                }))
            }
            // Skip thread records
            CatalogRecord::FolderThread(_) | CatalogRecord::FileThread(_) => Ok(None),
        }
    };

    // Collect entries, filtering out thread records
    let mut entries = Vec::new();
    let mut current_node_num = start_node;

    while current_node_num != 0 {
        let node = btree::read_node(reader, btree_header, current_node_num)?;

        if node.descriptor.kind != btree::NODE_KIND_LEAF {
            break;
        }

        for i in 0..node.descriptor.num_records as usize {
            let record_data = node.record_data(i)?;
            match match_fn(record_data) {
                Some(true) => {
                    if let Some(entry) = parse_fn(record_data)? {
                        entries.push(entry);
                    }
                }
                Some(false) => continue,
                None => return Ok(entries),
            }
        }

        current_node_num = node.descriptor.forward_link;
    }

    Ok(entries)
}

/// Find the leaf node that should contain (or precede) records for a given parent CNID
fn find_leaf_for_parent<R: Read + Seek>(
    reader: &mut R,
    btree_header: &BTreeHeaderRecord,
    _parent_cnid: u32,
    comparator: &dyn Fn(&[u8]) -> std::cmp::Ordering,
) -> Result<u32> {
    if btree_header.root_node == 0 {
        return Ok(0);
    }

    let mut current_node_num = btree_header.root_node;

    loop {
        let node = btree::read_node(reader, btree_header, current_node_num)?;

        match node.descriptor.kind {
            btree::NODE_KIND_LEAF => {
                return Ok(current_node_num);
            }
            btree::NODE_KIND_INDEX => {
                let mut child_node = 0u32;
                let mut found = false;

                for i in 0..node.descriptor.num_records as usize {
                    let record_data = node.record_data(i)?;
                    match comparator(record_data) {
                        std::cmp::Ordering::Less | std::cmp::Ordering::Equal => {
                            child_node = btree::extract_index_child_pub(record_data)?;
                            found = true;
                        }
                        std::cmp::Ordering::Greater => {
                            break;
                        }
                    }
                }

                if !found {
                    // All records are greater than our key — go to the first child
                    if node.descriptor.num_records > 0 {
                        let record_data = node.record_data(0)?;
                        child_node = btree::extract_index_child_pub(record_data)?;
                    } else {
                        return Ok(0);
                    }
                }

                current_node_num = child_node;
            }
            other => {
                return Err(HfsPlusError::InvalidBTree(
                    format!("unexpected node kind {} during leaf search", other),
                ));
            }
        }
    }
}

/// Resolve a path like "/Library/Extensions/foo.kext" to its catalog record.
/// Returns (CatalogRecord, name_of_final_component).
pub fn resolve_path<R: Read + Seek>(
    reader: &mut R,
    vol: &VolumeHeader,
    btree_header: &BTreeHeaderRecord,
    path: &str,
) -> Result<(CatalogRecord, String)> {
    let path = path.trim_matches('/');

    // Root directory
    if path.is_empty() {
        match lookup_catalog(reader, vol, btree_header, CNID_ROOT_PARENT, "")? {
            Some(record) => return Ok((record, String::new())),
            None => {
                // Try looking up the root folder thread record
                // The root folder has CNID 2, and its thread record has parent_id = 1
                // Instead, let's just construct a minimal folder record by looking up
                // the root folder's thread
                return lookup_root_folder(reader, vol, btree_header);
            }
        }
    }

    let components: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let mut current_parent = CNID_ROOT_FOLDER;

    for (i, component) in components.iter().enumerate() {
        match lookup_catalog(reader, vol, btree_header, current_parent, component)? {
            Some(record) => {
                if i == components.len() - 1 {
                    return Ok((record, component.to_string()));
                }
                match &record {
                    CatalogRecord::Folder(f) => {
                        current_parent = f.folder_id;
                    }
                    CatalogRecord::File(_) => {
                        if i < components.len() - 1 {
                            return Err(HfsPlusError::NotADirectory(
                                components[..=i].join("/"),
                            ));
                        }
                        return Ok((record, component.to_string()));
                    }
                    _ => {
                        return Err(HfsPlusError::CorruptedData(
                            "unexpected thread record in path resolution".into(),
                        ));
                    }
                }
            }
            None => {
                return Err(HfsPlusError::FileNotFound(
                    components[..=i].join("/"),
                ));
            }
        }
    }

    unreachable!()
}

/// Look up the root folder by finding it in the catalog
fn lookup_root_folder<R: Read + Seek>(
    reader: &mut R,
    vol: &VolumeHeader,
    btree_header: &BTreeHeaderRecord,
) -> Result<(CatalogRecord, String)> {
    // The root folder's thread record is keyed by (CNID_ROOT_FOLDER, "")
    // which points to (parent_id=1, name="")
    // We need to list parent_id=1 to find the root folder record
    let entries = list_directory(reader, vol, btree_header, CNID_ROOT_PARENT)?;
    if let Some(entry) = entries.first() {
        // Look up the root folder properly
        match lookup_catalog(reader, vol, btree_header, CNID_ROOT_PARENT, &entry.name)? {
            Some(record) => Ok((record, entry.name.clone())),
            None => Err(HfsPlusError::FileNotFound("root folder".into())),
        }
    } else {
        Err(HfsPlusError::FileNotFound("root folder".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufReader;

    fn open_kdk() -> (BufReader<std::fs::File>, VolumeHeader, BTreeHeaderRecord) {
        let file = std::fs::File::open("../tests/kdk.raw").unwrap();
        let mut reader = BufReader::new(file);
        let vol = VolumeHeader::parse(&mut reader).unwrap();
        let catalog_header = btree::read_btree_header(&mut reader, &vol.catalog_file, vol.block_size).unwrap();
        (reader, vol, catalog_header)
    }

    /// Requires ../tests/kdk.raw fixture. Run with `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn test_list_root_directory() {
        let (mut reader, vol, catalog_header) = open_kdk();

        let entries = list_directory(&mut reader, &vol, &catalog_header, CNID_ROOT_FOLDER).unwrap();
        assert!(!entries.is_empty(), "Root directory should not be empty");
    }

    /// Requires ../tests/kdk.raw fixture. Run with `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn test_resolve_root_path() {
        let (mut reader, vol, catalog_header) = open_kdk();

        let entries = list_directory(&mut reader, &vol, &catalog_header, CNID_ROOT_FOLDER).unwrap();
        let first = entries.first().expect("Root should have entries");
        let path = format!("/{}", first.name);
        let (_record, _name) = resolve_path(&mut reader, &vol, &catalog_header, &path).unwrap();
    }
}
