use byteorder::{LittleEndian, ReadBytesExt};
use std::io::{Cursor, Read, Seek};

use crate::btree;
use crate::error::{ApfsError, Result};
use crate::object;

/// OMAP key: (oid: u64, xid: u64) — 16 bytes, fixed-size.
/// OMAP value: (flags: u32, size: u32, paddr: u64) — 16 bytes, fixed-size.
const OMAP_KEY_SIZE: u32 = 16;
const OMAP_VAL_SIZE: u32 = 16;

/// Read the OMAP structure at a given physical block and return the
/// physical block number of the OMAP B-tree root.
pub fn read_omap_tree_root<R: Read + Seek>(
    reader: &mut R,
    omap_block: u64,
    block_size: u32,
) -> Result<u64> {
    let block_data = object::read_block(reader, omap_block, block_size)?;

    // omap_phys_t layout after obj_phys_t (32 bytes):
    //   om_flags: u32 (4)
    //   om_snap_count: u32 (4)
    //   om_tree_type: u32 (4)
    //   om_snapshot_tree_type: u32 (4)
    //   om_tree_oid: u64 (8)  <- B-tree root physical block
    let mut cursor = Cursor::new(&block_data[object::ObjectHeader::SIZE..]);
    let _om_flags = cursor.read_u32::<LittleEndian>()?;
    let _om_snap_count = cursor.read_u32::<LittleEndian>()?;
    let _om_tree_type = cursor.read_u32::<LittleEndian>()?;
    let _om_snap_tree_type = cursor.read_u32::<LittleEndian>()?;
    let om_tree_oid = cursor.read_u64::<LittleEndian>()?;

    Ok(om_tree_oid)
}

/// Look up a virtual OID in an OMAP B-tree and return the physical block address.
///
/// The OMAP B-tree uses fixed-size keys (oid: u64, xid: u64) and fixed-size
/// values (flags: u32, size: u32, paddr: u64). We search for the entry with
/// the matching OID and the highest xid that is <= the current transaction.
///
/// Since we want the most recent mapping, we search for the target_oid and
/// accept any xid (effectively finding the latest mapping).
pub fn omap_lookup<R: Read + Seek>(
    reader: &mut R,
    omap_tree_root: u64,
    block_size: u32,
    target_oid: u64,
) -> Result<u64> {
    // For the OMAP lookup, we need to find the entry with matching OID.
    // OMAP keys are sorted by (oid, xid). We want the highest xid for our oid.
    //
    // Strategy: use btree_scan to find all entries for this OID, then pick the
    // one with the highest xid. This is simpler than trying to do a range query.

    let compare_fn = |key: &[u8]| -> std::cmp::Ordering {
        if key.len() < 16 {
            return std::cmp::Ordering::Less;
        }
        let key_oid = u64::from_le_bytes([key[0], key[1], key[2], key[3], key[4], key[5], key[6], key[7]]);
        // Compare only by OID. For equal OIDs, we consider the key "equal" to let
        // btree_lookup find the first match, then we'll use scan for the latest xid.
        key_oid.cmp(&target_oid)
    };

    // First try a direct lookup — this finds the first entry with matching OID
    // OMAP B-trees are physical, so omap_root = None
    let result = btree::btree_lookup(
        reader,
        omap_tree_root,
        block_size,
        OMAP_KEY_SIZE,
        OMAP_VAL_SIZE,
        &compare_fn,
        None,
    )?;

    if let Some(val) = result {
        return parse_omap_val(&val);
    }

    // If direct lookup fails, try scanning for the OID with any xid
    let range_fn = |key: &[u8]| -> Option<bool> {
        if key.len() < 16 {
            return Some(false);
        }
        let key_oid = u64::from_le_bytes([key[0], key[1], key[2], key[3], key[4], key[5], key[6], key[7]]);
        if key_oid < target_oid {
            Some(false) // skip, keep scanning
        } else if key_oid == target_oid {
            Some(true)  // match
        } else {
            None // past our OID, stop
        }
    };

    let entries = btree::btree_scan(
        reader,
        omap_tree_root,
        block_size,
        OMAP_KEY_SIZE,
        OMAP_VAL_SIZE,
        &range_fn,
        None,
    )?;

    if entries.is_empty() {
        return Err(ApfsError::CorruptedData(
            format!("OMAP lookup failed: OID {} not found", target_oid),
        ));
    }

    // Pick the entry with the highest xid
    let mut best_xid: u64 = 0;
    let mut best_paddr: u64 = 0;

    for (key, val) in &entries {
        if key.len() >= 16 {
            let xid = u64::from_le_bytes([key[8], key[9], key[10], key[11], key[12], key[13], key[14], key[15]]);
            if xid >= best_xid {
                best_xid = xid;
                best_paddr = parse_omap_val(val)?;
            }
        }
    }

    if best_paddr == 0 {
        return Err(ApfsError::CorruptedData(
            format!("OMAP lookup: OID {} resolved to paddr 0", target_oid),
        ));
    }

    Ok(best_paddr)
}

/// Parse an OMAP value: (flags: u32, size: u32, paddr: u64)
fn parse_omap_val(val: &[u8]) -> Result<u64> {
    if val.len() < 16 {
        return Err(ApfsError::InvalidBTree("omap value too short".into()));
    }
    let paddr = u64::from_le_bytes([val[8], val[9], val[10], val[11], val[12], val[13], val[14], val[15]]);
    Ok(paddr)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::superblock;
    use std::io::BufReader;

    /// Requires ../tests/appfs.raw fixture. Run with `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn test_omap_lookup() {
        let file = std::fs::File::open("../tests/appfs.raw").unwrap();
        let mut reader = BufReader::new(file);

        let nxsb = superblock::read_nxsb(&mut reader).unwrap();
        let latest = superblock::find_latest_nxsb(&mut reader, &nxsb).unwrap();

        let omap_root = read_omap_tree_root(&mut reader, latest.omap_oid, latest.block_size).unwrap();

        let vol_oid = latest.fs_oids.iter().find(|&&o| o != 0).copied().unwrap();

        let vol_block = omap_lookup(&mut reader, omap_root, latest.block_size, vol_oid).unwrap();
        assert!(vol_block > 0 && vol_block < latest.block_count,
            "Physical block {} should be within container", vol_block);

        let vol_data = object::read_block(&mut reader, vol_block, latest.block_size).unwrap();
        let vol_sb = superblock::ApfsSuperblock::parse(&vol_data).unwrap();
        assert_eq!(vol_sb.magic, superblock::APSB_MAGIC);
    }
}
