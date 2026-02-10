use byteorder::{BigEndian, ReadBytesExt};
use std::io::{Read, Seek, SeekFrom, Cursor};

use crate::error::{HfsPlusError, Result};
use crate::volume::ForkData;

/// B-tree node kinds
pub const NODE_KIND_LEAF: u8 = 0xFF;   // -1 as i8
pub const NODE_KIND_INDEX: u8 = 0x00;
pub const NODE_KIND_HEADER: u8 = 0x01;
pub const NODE_KIND_MAP: u8 = 0x02;

/// B-tree header record (from the header node, record 0)
#[derive(Debug, Clone)]
pub struct BTreeHeaderRecord {
    pub tree_depth: u16,
    pub root_node: u32,
    pub leaf_records: u32,
    pub first_leaf_node: u32,
    pub last_leaf_node: u32,
    pub node_size: u16,
    pub max_key_length: u16,
    pub total_nodes: u32,
    pub free_nodes: u32,
    pub key_compare_type: u32,
    /// Fork data for computing byte offsets into the B-tree file
    pub fork: ForkData,
    /// Block size of the volume
    pub block_size: u32,
}

/// A B-tree node descriptor (14 bytes at the start of each node)
#[derive(Debug, Clone)]
pub struct NodeDescriptor {
    pub forward_link: u32,
    pub backward_link: u32,
    pub kind: u8,
    pub height: u8,
    pub num_records: u16,
    pub reserved: u16,
}

/// A parsed B-tree node with its raw data
#[derive(Debug)]
pub struct BTreeNode {
    pub descriptor: NodeDescriptor,
    /// Raw node data (node_size bytes)
    pub data: Vec<u8>,
    /// Record offsets (from the offset table at the end of the node)
    pub record_offsets: Vec<u16>,
}

/// Read the B-tree header record from the first node of a fork
pub fn read_btree_header<R: Read + Seek>(
    reader: &mut R,
    fork: &ForkData,
    block_size: u32,
) -> Result<BTreeHeaderRecord> {
    // The header node is always node 0
    let node_data = read_raw_node(reader, fork, block_size, 0, 512)?;

    // First, we need to figure out the real node size from the header record.
    // The node descriptor is 14 bytes, then record 0 is the header record.
    let mut cursor = Cursor::new(&node_data);

    // Parse node descriptor
    let desc = parse_node_descriptor(&mut cursor)?;
    if desc.kind != NODE_KIND_HEADER {
        return Err(HfsPlusError::InvalidBTree(
            format!("expected header node, got kind {}", desc.kind),
        ));
    }

    // Header record starts at offset 14
    let tree_depth = cursor.read_u16::<BigEndian>()?;
    let root_node = cursor.read_u32::<BigEndian>()?;
    let leaf_records = cursor.read_u32::<BigEndian>()?;
    let first_leaf_node = cursor.read_u32::<BigEndian>()?;
    let last_leaf_node = cursor.read_u32::<BigEndian>()?;
    let node_size = cursor.read_u16::<BigEndian>()?;
    let max_key_length = cursor.read_u16::<BigEndian>()?;
    let total_nodes = cursor.read_u32::<BigEndian>()?;
    let free_nodes = cursor.read_u32::<BigEndian>()?;
    let _reserved = cursor.read_u16::<BigEndian>()?;
    let _clump_size = cursor.read_u32::<BigEndian>()?;
    let _btree_type = cursor.read_u8()?;
    let key_compare_type = cursor.read_u8()? as u32;
    let _attributes = cursor.read_u32::<BigEndian>()?;
    // Skip reserved bytes (16 * 4 = 64 bytes)

    Ok(BTreeHeaderRecord {
        tree_depth,
        root_node,
        leaf_records,
        first_leaf_node,
        last_leaf_node,
        node_size,
        max_key_length,
        total_nodes,
        free_nodes,
        key_compare_type,
        fork: fork.clone(),
        block_size,
    })
}

/// Read raw bytes for a node. We read `read_size` bytes at the node's offset.
/// If `read_size` is less than the actual node size, we read what we can
/// (used for initial header read where we don't know node_size yet).
fn read_raw_node<R: Read + Seek>(
    reader: &mut R,
    fork: &ForkData,
    block_size: u32,
    node_number: u32,
    read_size: u16,
) -> Result<Vec<u8>> {
    let byte_offset = compute_fork_offset(fork, block_size, node_number as u64 * read_size as u64)?;
    reader.seek(SeekFrom::Start(byte_offset))?;

    let mut buf = vec![0u8; read_size as usize];
    reader.read_exact(&mut buf)?;
    Ok(buf)
}

/// Read and parse a B-tree node
pub fn read_node<R: Read + Seek>(
    reader: &mut R,
    btree_header: &BTreeHeaderRecord,
    node_number: u32,
) -> Result<BTreeNode> {
    let node_size = btree_header.node_size;
    let byte_offset_in_fork = node_number as u64 * node_size as u64;
    let byte_offset = compute_fork_offset(
        &btree_header.fork,
        btree_header.block_size,
        byte_offset_in_fork,
    )?;

    reader.seek(SeekFrom::Start(byte_offset))?;
    let mut data = vec![0u8; node_size as usize];
    reader.read_exact(&mut data)?;

    // Parse the node descriptor
    let mut cursor = Cursor::new(&data);
    let descriptor = parse_node_descriptor(&mut cursor)?;

    // Read the record offset table from the end of the node
    // Offsets are stored as u16 values at the very end, growing backwards
    let num_offsets = descriptor.num_records as usize + 1; // +1 for the free space offset
    let mut record_offsets = Vec::with_capacity(num_offsets);

    for i in 0..num_offsets {
        let offset_pos = node_size as usize - (i + 1) * 2;
        if offset_pos + 1 >= data.len() {
            return Err(HfsPlusError::InvalidBTree("offset table out of bounds".into()));
        }
        let offset = u16::from_be_bytes([data[offset_pos], data[offset_pos + 1]]);
        record_offsets.push(offset);
    }

    Ok(BTreeNode {
        descriptor,
        data,
        record_offsets,
    })
}

impl BTreeNode {
    /// Get the raw bytes for record `index` in this node
    pub fn record_data(&self, index: usize) -> Result<&[u8]> {
        if index >= self.descriptor.num_records as usize {
            return Err(HfsPlusError::InvalidBTree(
                format!("record index {} >= num_records {}", index, self.descriptor.num_records),
            ));
        }
        let start = self.record_offsets[index] as usize;
        let end = self.record_offsets[index + 1] as usize;
        if start > end || end > self.data.len() {
            return Err(HfsPlusError::InvalidBTree(
                format!("invalid record offsets: start={}, end={}, len={}", start, end, self.data.len()),
            ));
        }
        Ok(&self.data[start..end])
    }
}

fn parse_node_descriptor<R: Read>(reader: &mut R) -> Result<NodeDescriptor> {
    Ok(NodeDescriptor {
        forward_link: reader.read_u32::<BigEndian>()?,
        backward_link: reader.read_u32::<BigEndian>()?,
        kind: reader.read_u8()?,
        height: reader.read_u8()?,
        num_records: reader.read_u16::<BigEndian>()?,
        reserved: reader.read_u16::<BigEndian>()?,
    })
}

/// Compute the absolute byte offset in the volume for a given byte offset within a fork.
/// Walks through the fork's extent descriptors to find the right allocation block.
pub fn compute_fork_offset(
    fork: &ForkData,
    block_size: u32,
    offset_in_fork: u64,
) -> Result<u64> {
    let block_size = block_size as u64;
    let mut remaining = offset_in_fork;

    for extent in &fork.extents {
        if extent.block_count == 0 {
            break;
        }
        let extent_bytes = extent.block_count as u64 * block_size;
        if remaining < extent_bytes {
            let block_within_extent = remaining / block_size;
            let offset_within_block = remaining % block_size;
            let absolute_block = extent.start_block as u64 + block_within_extent;
            return Ok(absolute_block * block_size + offset_within_block);
        }
        remaining -= extent_bytes;
    }

    Err(HfsPlusError::InvalidBTree(
        format!("fork offset {} exceeds extent capacity", offset_in_fork),
    ))
}

/// Search a B-tree for a key. Returns the leaf node and record index where the key
/// was found, or where it would be inserted.
///
/// `compare_key` takes raw record bytes and returns Ordering relative to the search key:
/// - Less means the record key is less than the search key
/// - Greater means the record key is greater than the search key
/// - Equal means exact match
pub fn search_btree<R, F>(
    reader: &mut R,
    btree_header: &BTreeHeaderRecord,
    compare_key: &F,
) -> Result<Option<(BTreeNode, usize)>>
where
    R: Read + Seek,
    F: Fn(&[u8]) -> std::cmp::Ordering,
{
    if btree_header.root_node == 0 {
        return Ok(None);
    }

    let mut current_node_num = btree_header.root_node;

    loop {
        let node = read_node(reader, btree_header, current_node_num)?;

        match node.descriptor.kind {
            NODE_KIND_LEAF => {
                // Search through leaf records for an exact match
                for i in 0..node.descriptor.num_records as usize {
                    let record_data = node.record_data(i)?;
                    match compare_key(record_data) {
                        std::cmp::Ordering::Equal => return Ok(Some((node, i))),
                        std::cmp::Ordering::Greater => return Ok(None),
                        std::cmp::Ordering::Less => continue,
                    }
                }
                return Ok(None);
            }
            NODE_KIND_INDEX => {
                // In index nodes, find the last record whose key is <= search key.
                // Each record's data (after the key) is a u32 child node number.
                let mut child_node = 0u32;
                let mut found = false;

                for i in 0..node.descriptor.num_records as usize {
                    let record_data = node.record_data(i)?;
                    match compare_key(record_data) {
                        std::cmp::Ordering::Less | std::cmp::Ordering::Equal => {
                            // Extract child node pointer from the end of the record
                            child_node = extract_index_child(record_data)?;
                            found = true;
                        }
                        std::cmp::Ordering::Greater => {
                            break;
                        }
                    }
                }

                if !found {
                    return Ok(None);
                }

                current_node_num = child_node;
            }
            other => {
                return Err(HfsPlusError::InvalidBTree(
                    format!("unexpected node kind {} during search", other),
                ));
            }
        }
    }
}

/// Scan leaf nodes to find all records matching a predicate.
/// Starts from the first leaf and scans forward.
/// `match_fn` returns:
/// - Some(true) to include this record
/// - Some(false) to skip
/// - None to stop scanning (all subsequent records will also not match)
pub fn scan_leaves<R, F, T, P>(
    reader: &mut R,
    btree_header: &BTreeHeaderRecord,
    start_node: u32,
    match_fn: &F,
    parse_fn: &P,
) -> Result<Vec<T>>
where
    R: Read + Seek,
    F: Fn(&[u8]) -> Option<bool>,
    P: Fn(&[u8]) -> Result<T>,
{
    let mut results = Vec::new();
    let mut current_node_num = start_node;

    while current_node_num != 0 {
        let node = read_node(reader, btree_header, current_node_num)?;

        if node.descriptor.kind != NODE_KIND_LEAF {
            return Err(HfsPlusError::InvalidBTree(
                format!("expected leaf node, got kind {}", node.descriptor.kind),
            ));
        }

        for i in 0..node.descriptor.num_records as usize {
            let record_data = node.record_data(i)?;
            match match_fn(record_data) {
                Some(true) => {
                    results.push(parse_fn(record_data)?);
                }
                Some(false) => continue,
                None => return Ok(results), // stop scanning
            }
        }

        current_node_num = node.descriptor.forward_link;
    }

    Ok(results)
}

/// Extract the child node number from an index node record (public alias).
pub fn extract_index_child_pub(record_data: &[u8]) -> Result<u32> {
    extract_index_child(record_data)
}

/// Extract the child node number from an index node record.
/// In HFS+ B-trees, the key is followed by a u32 node number.
fn extract_index_child(record_data: &[u8]) -> Result<u32> {
    // Record format: [key_length: u16] [key_data: key_length bytes] [child_node: u32]
    if record_data.len() < 2 {
        return Err(HfsPlusError::InvalidBTree("index record too short".into()));
    }
    let key_length = u16::from_be_bytes([record_data[0], record_data[1]]) as usize;
    let child_offset = 2 + key_length;
    if child_offset + 4 > record_data.len() {
        return Err(HfsPlusError::InvalidBTree(
            format!("index record too short for child pointer: key_len={}, record_len={}", key_length, record_data.len()),
        ));
    }
    Ok(u32::from_be_bytes([
        record_data[child_offset],
        record_data[child_offset + 1],
        record_data[child_offset + 2],
        record_data[child_offset + 3],
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_btree_header_from_kdk() {
        let path = std::path::Path::new("../tests/kdk.raw");
        if !path.exists() {
            eprintln!("Skipping test - kdk.raw not found");
            return;
        }

        let file = std::fs::File::open(path).unwrap();
        let mut reader = std::io::BufReader::new(file);
        let vol = crate::volume::VolumeHeader::parse(&mut reader).unwrap();

        let catalog_header = read_btree_header(&mut reader, &vol.catalog_file, vol.block_size).unwrap();

        eprintln!("Catalog B-tree header:");
        eprintln!("  tree_depth: {}", catalog_header.tree_depth);
        eprintln!("  root_node: {}", catalog_header.root_node);
        eprintln!("  leaf_records: {}", catalog_header.leaf_records);
        eprintln!("  first_leaf_node: {}", catalog_header.first_leaf_node);
        eprintln!("  last_leaf_node: {}", catalog_header.last_leaf_node);
        eprintln!("  node_size: {}", catalog_header.node_size);
        eprintln!("  max_key_length: {}", catalog_header.max_key_length);
        eprintln!("  total_nodes: {}", catalog_header.total_nodes);
        eprintln!("  free_nodes: {}", catalog_header.free_nodes);
        eprintln!("  key_compare_type: {}", catalog_header.key_compare_type);

        assert!(catalog_header.node_size > 0);
        assert!(catalog_header.root_node > 0);
        assert!(catalog_header.leaf_records > 0);
    }
}
