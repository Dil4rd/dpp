use byteorder::{LittleEndian, ReadBytesExt};
use std::io::{Cursor, Read, Seek};

use crate::error::{ApfsError, Result};
use crate::object::{self, ObjectHeader};
use crate::omap;

// B-tree node flags (from btn_flags)
pub const BTNODE_ROOT: u16 = 0x0001;
pub const BTNODE_LEAF: u16 = 0x0002;
pub const BTNODE_FIXED_KV_SIZE: u16 = 0x0004;

// BTreeInfo flags
pub const BTREE_PHYSICAL: u32 = 0x0001;

/// B-tree node header — 56 bytes after the object header.
#[derive(Debug, Clone)]
pub struct BTreeNodeHeader {
    pub btn_flags: u16,
    pub btn_level: u16,
    pub btn_nkeys: u32,
    pub btn_table_space_off: u16,
    pub btn_table_space_len: u16,
    pub btn_free_space_off: u16,
    pub btn_free_space_len: u16,
    pub btn_free_list_off: u16,
    pub btn_free_list_len: u16,
    pub btn_key_free_list_off: u16,
    pub btn_key_free_list_len: u16,
    pub btn_val_free_list_off: u16,
    pub btn_val_free_list_len: u16,
}

impl BTreeNodeHeader {
    pub const SIZE: usize = 24;

    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < Self::SIZE {
            return Err(ApfsError::InvalidBTree("btree node header too short".into()));
        }
        let mut cursor = Cursor::new(data);
        Ok(BTreeNodeHeader {
            btn_flags: cursor.read_u16::<LittleEndian>()?,
            btn_level: cursor.read_u16::<LittleEndian>()?,
            btn_nkeys: cursor.read_u32::<LittleEndian>()?,
            btn_table_space_off: cursor.read_u16::<LittleEndian>()?,
            btn_table_space_len: cursor.read_u16::<LittleEndian>()?,
            btn_free_space_off: cursor.read_u16::<LittleEndian>()?,
            btn_free_space_len: cursor.read_u16::<LittleEndian>()?,
            btn_free_list_off: cursor.read_u16::<LittleEndian>()?,
            btn_free_list_len: cursor.read_u16::<LittleEndian>()?,
            btn_key_free_list_off: cursor.read_u16::<LittleEndian>()?,
            btn_key_free_list_len: cursor.read_u16::<LittleEndian>()?,
            btn_val_free_list_off: cursor.read_u16::<LittleEndian>()?,
            btn_val_free_list_len: cursor.read_u16::<LittleEndian>()?,
        })
    }

    pub fn is_leaf(&self) -> bool {
        self.btn_flags & BTNODE_LEAF != 0
    }

    pub fn is_root(&self) -> bool {
        self.btn_flags & BTNODE_ROOT != 0
    }

    pub fn is_fixed_kv(&self) -> bool {
        self.btn_flags & BTNODE_FIXED_KV_SIZE != 0
    }
}

/// BTreeInfo — 40 bytes at the end of a root node (before the footer).
#[derive(Debug, Clone)]
pub struct BTreeInfo {
    pub bt_fixed: BTreeInfoFixed,
    pub bt_longest_key: u32,
    pub bt_longest_val: u32,
    pub bt_key_count: u64,
    pub bt_node_count: u64,
}

#[derive(Debug, Clone)]
pub struct BTreeInfoFixed {
    pub bt_flags: u32,
    pub bt_node_size: u32,
    pub bt_key_size: u32,
    pub bt_val_size: u32,
}

impl BTreeInfo {
    pub const SIZE: usize = 40;

    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < Self::SIZE {
            return Err(ApfsError::InvalidBTree("btree info too short".into()));
        }
        let mut cursor = Cursor::new(data);
        let bt_flags = cursor.read_u32::<LittleEndian>()?;
        let bt_node_size = cursor.read_u32::<LittleEndian>()?;
        let bt_key_size = cursor.read_u32::<LittleEndian>()?;
        let bt_val_size = cursor.read_u32::<LittleEndian>()?;
        let bt_longest_key = cursor.read_u32::<LittleEndian>()?;
        let bt_longest_val = cursor.read_u32::<LittleEndian>()?;
        let bt_key_count = cursor.read_u64::<LittleEndian>()?;
        let bt_node_count = cursor.read_u64::<LittleEndian>()?;

        Ok(BTreeInfo {
            bt_fixed: BTreeInfoFixed {
                bt_flags,
                bt_node_size,
                bt_key_size,
                bt_val_size,
            },
            bt_longest_key,
            bt_longest_val,
            bt_key_count,
            bt_node_count,
        })
    }
}

/// A Table of Contents entry (fixed-size KV: 4 bytes, variable-size: 8 bytes)
#[derive(Debug, Clone)]
pub struct TocEntry {
    pub key_off: u16,
    pub key_len: u16,   // 0 for fixed-size KV
    pub val_off: u16,
    pub val_len: u16,   // 0 for fixed-size KV
}

/// A parsed APFS B-tree node with extracted key-value pairs.
pub struct BTreeNode {
    pub header: ObjectHeader,
    pub node_header: BTreeNodeHeader,
    pub toc: Vec<TocEntry>,
    pub block_data: Vec<u8>,
    pub key_area_off: usize,   // Absolute offset within block_data where key area starts
    pub val_area_end: usize,   // Absolute offset within block_data where val area ends
    pub info: Option<BTreeInfo>,
}

impl BTreeNode {
    /// Parse a B-tree node from a raw block.
    pub fn parse(block: &[u8]) -> Result<Self> {
        let header = ObjectHeader::parse(block)?;
        let node_header = BTreeNodeHeader::parse(&block[ObjectHeader::SIZE..])?;

        let toc_start = ObjectHeader::SIZE + BTreeNodeHeader::SIZE + node_header.btn_table_space_off as usize;
        let fixed_kv = node_header.is_fixed_kv();

        // Key area starts right after the table space
        let key_area_off = ObjectHeader::SIZE + BTreeNodeHeader::SIZE
            + node_header.btn_table_space_off as usize
            + node_header.btn_table_space_len as usize;

        // Parse BTreeInfo if this is a root node (it's at the end of the value area)
        let info = if node_header.is_root() {
            let info_start = block.len() - BTreeInfo::SIZE;
            Some(BTreeInfo::parse(&block[info_start..])?)
        } else {
            None
        };

        // Value area end: for root nodes, it's before BTreeInfo; for non-root, it's end of block
        let val_area_end = if node_header.is_root() {
            block.len() - BTreeInfo::SIZE
        } else {
            block.len()
        };

        // Parse TOC entries
        let mut toc = Vec::with_capacity(node_header.btn_nkeys as usize);
        let mut cursor = Cursor::new(&block[toc_start..]);

        for _ in 0..node_header.btn_nkeys {
            if fixed_kv {
                let key_off = cursor.read_u16::<LittleEndian>()?;
                let val_off = cursor.read_u16::<LittleEndian>()?;
                toc.push(TocEntry {
                    key_off,
                    key_len: 0,
                    val_off,
                    val_len: 0,
                });
            } else {
                let key_off = cursor.read_u16::<LittleEndian>()?;
                let key_len = cursor.read_u16::<LittleEndian>()?;
                let val_off = cursor.read_u16::<LittleEndian>()?;
                let val_len = cursor.read_u16::<LittleEndian>()?;
                toc.push(TocEntry {
                    key_off,
                    key_len,
                    val_off,
                    val_len,
                });
            }
        }

        Ok(BTreeNode {
            header,
            node_header,
            toc,
            block_data: block.to_vec(),
            key_area_off,
            val_area_end,
            info,
        })
    }

    /// Get the key bytes for a given TOC index.
    pub fn key(&self, index: usize, fixed_key_size: u32) -> Result<&[u8]> {
        let entry = &self.toc[index];
        let start = self.key_area_off + entry.key_off as usize;
        let len = if self.node_header.is_fixed_kv() {
            fixed_key_size as usize
        } else {
            entry.key_len as usize
        };
        let end = start + len;
        if end > self.block_data.len() {
            return Err(ApfsError::InvalidBTree(
                format!("key out of bounds: start={}, len={}, block_size={}", start, len, self.block_data.len()),
            ));
        }
        Ok(&self.block_data[start..end])
    }

    /// Get the value bytes for a given TOC index.
    ///
    /// val_off is an offset from val_area_end to the START of the value data.
    /// i.e., value bytes are at block_data[val_area_end - val_off .. val_area_end - val_off + len].
    ///
    /// For internal (non-leaf) nodes, the value is always an oid_t (u64, 8 bytes).
    pub fn value(&self, index: usize, fixed_val_size: u32) -> Result<&[u8]> {
        let entry = &self.toc[index];
        let len = if !self.node_header.is_leaf() {
            // Internal node values are always an oid_t (8 bytes)
            8
        } else if self.node_header.is_fixed_kv() {
            fixed_val_size as usize
        } else {
            entry.val_len as usize
        };

        let val_off = entry.val_off as usize;
        let start = self.val_area_end - val_off;
        let end = start + len;
        if end > self.block_data.len() || start < self.key_area_off {
            return Err(ApfsError::InvalidBTree(
                format!("value out of bounds: start={}, len={}, val_area_end={}, block_size={}",
                    start, len, self.val_area_end, self.block_data.len()),
            ));
        }
        Ok(&self.block_data[start..end])
    }

    /// For index nodes, get the child OID at a given index.
    /// The value for index nodes is always an oid_t (u64, 8 bytes).
    pub fn child_oid(&self, index: usize) -> Result<u64> {
        let val = self.value(index, 8)?;
        if val.len() < 8 {
            return Err(ApfsError::InvalidBTree("child oid too short".into()));
        }
        Ok(u64::from_le_bytes([val[0], val[1], val[2], val[3], val[4], val[5], val[6], val[7]]))
    }
}

/// Resolve a child OID to a physical block number.
/// If `omap_root` is Some, the OID is virtual and needs OMAP resolution.
/// If `omap_root` is None, the OID is already a physical block number.
fn resolve_child_oid<R: Read + Seek>(
    reader: &mut R,
    child_oid: u64,
    block_size: u32,
    omap_root: Option<u64>,
) -> Result<u64> {
    match omap_root {
        Some(omap) => omap::omap_lookup(reader, omap, block_size, child_oid),
        None => Ok(child_oid),
    }
}

/// Look up a key in a B-tree.
///
/// `compare_fn` takes key bytes and returns Ordering of the node key relative to the search key:
/// - Less: node key < search key
/// - Equal: match
/// - Greater: node key > search key
///
/// `omap_root`: Some(block) for virtual B-trees (catalog), None for physical (OMAP).
///
/// Returns the raw value bytes if found.
pub fn btree_lookup<R: Read + Seek, F>(
    reader: &mut R,
    root_block: u64,
    block_size: u32,
    fixed_key_size: u32,
    fixed_val_size: u32,
    compare_fn: &F,
    omap_root: Option<u64>,
) -> Result<Option<Vec<u8>>>
where
    F: Fn(&[u8]) -> std::cmp::Ordering,
{
    let block_data = object::read_block(reader, root_block, block_size)?;
    let node = BTreeNode::parse(&block_data)?;

    // Get fixed sizes from BTreeInfo if available (root node)
    let (fks, fvs) = if let Some(ref info) = node.info {
        (
            if info.bt_fixed.bt_key_size > 0 { info.bt_fixed.bt_key_size } else { fixed_key_size },
            if info.bt_fixed.bt_val_size > 0 { info.bt_fixed.bt_val_size } else { fixed_val_size },
        )
    } else {
        (fixed_key_size, fixed_val_size)
    };

    btree_lookup_node(reader, &node, block_size, fks, fvs, compare_fn, omap_root)
}

fn btree_lookup_node<R: Read + Seek, F>(
    reader: &mut R,
    node: &BTreeNode,
    block_size: u32,
    fixed_key_size: u32,
    fixed_val_size: u32,
    compare_fn: &F,
    omap_root: Option<u64>,
) -> Result<Option<Vec<u8>>>
where
    F: Fn(&[u8]) -> std::cmp::Ordering,
{
    if node.node_header.is_leaf() {
        // Search leaf for exact match
        for i in 0..node.node_header.btn_nkeys as usize {
            let key = node.key(i, fixed_key_size)?;
            match compare_fn(key) {
                std::cmp::Ordering::Equal => {
                    let val = node.value(i, fixed_val_size)?;
                    return Ok(Some(val.to_vec()));
                }
                std::cmp::Ordering::Greater => return Ok(None),
                std::cmp::Ordering::Less => continue,
            }
        }
        Ok(None)
    } else {
        // Internal node: find the last key <= search key, follow child pointer
        let mut child_idx: Option<usize> = None;

        for i in 0..node.node_header.btn_nkeys as usize {
            let key = node.key(i, fixed_key_size)?;
            match compare_fn(key) {
                std::cmp::Ordering::Less | std::cmp::Ordering::Equal => {
                    child_idx = Some(i);
                }
                std::cmp::Ordering::Greater => break,
            }
        }

        let child_idx = match child_idx {
            Some(i) => i,
            None => return Ok(None),
        };

        let child_oid = node.child_oid(child_idx)?;
        let child_block = resolve_child_oid(reader, child_oid, block_size, omap_root)?;

        let child_data = object::read_block(reader, child_block, block_size)?;
        let child_node = BTreeNode::parse(&child_data)?;

        btree_lookup_node(reader, &child_node, block_size, fixed_key_size, fixed_val_size, compare_fn, omap_root)
    }
}

/// Scan a B-tree, collecting all key-value pairs where `range_fn` returns true.
///
/// `range_fn` takes key bytes and returns:
/// - Some(true): include this entry
/// - Some(false): skip this entry, keep scanning
/// - None: stop scanning
///
/// `omap_root`: Some(block) for virtual B-trees, None for physical.
pub fn btree_scan<R: Read + Seek, F>(
    reader: &mut R,
    root_block: u64,
    block_size: u32,
    fixed_key_size: u32,
    fixed_val_size: u32,
    range_fn: &F,
    omap_root: Option<u64>,
) -> Result<Vec<(Vec<u8>, Vec<u8>)>>
where
    F: Fn(&[u8]) -> Option<bool>,
{
    let block_data = object::read_block(reader, root_block, block_size)?;
    let node = BTreeNode::parse(&block_data)?;

    let (fks, fvs) = if let Some(ref info) = node.info {
        (
            if info.bt_fixed.bt_key_size > 0 { info.bt_fixed.bt_key_size } else { fixed_key_size },
            if info.bt_fixed.bt_val_size > 0 { info.bt_fixed.bt_val_size } else { fixed_val_size },
        )
    } else {
        (fixed_key_size, fixed_val_size)
    };

    let mut results = Vec::new();
    btree_scan_node(reader, &node, block_size, fks, fvs, range_fn, &mut results, omap_root)?;
    Ok(results)
}

fn btree_scan_node<R: Read + Seek, F>(
    reader: &mut R,
    node: &BTreeNode,
    block_size: u32,
    fixed_key_size: u32,
    fixed_val_size: u32,
    range_fn: &F,
    results: &mut Vec<(Vec<u8>, Vec<u8>)>,
    omap_root: Option<u64>,
) -> Result<bool>  // returns false if scanning should stop
where
    F: Fn(&[u8]) -> Option<bool>,
{
    if node.node_header.is_leaf() {
        for i in 0..node.node_header.btn_nkeys as usize {
            let key = node.key(i, fixed_key_size)?;
            match range_fn(key) {
                Some(true) => {
                    let val = node.value(i, fixed_val_size)?;
                    results.push((key.to_vec(), val.to_vec()));
                }
                Some(false) => continue,
                None => return Ok(false),
            }
        }
        Ok(true)
    } else {
        // For non-leaf nodes, visit each child subtree
        // The number of children is btn_nkeys (each key has an associated child pointer)
        // Plus there may be one more child than keys (rightmost child).
        // In APFS B-trees, btn_nkeys IS the number of children for internal nodes.
        for i in 0..node.node_header.btn_nkeys as usize {
            let child_oid = node.child_oid(i)?;
            let child_block = resolve_child_oid(reader, child_oid, block_size, omap_root)?;
            let child_data = object::read_block(reader, child_block, block_size)?;
            let child_node = BTreeNode::parse(&child_data)?;

            if !btree_scan_node(reader, &child_node, block_size, fixed_key_size, fixed_val_size, range_fn, results, omap_root)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
}
