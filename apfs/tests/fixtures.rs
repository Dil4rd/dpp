//! Fixture tests: the only coverage that runs the reader against images macOS
//! actually wrote.
//!
//! Every test here is `#[ignore]`d, so neither `cargo test` nor CI runs it —
//! the fixtures in `tests/` are gitignored and exist only on a maintainer's
//! machine. Run them with `cargo test -p apfs -- --ignored` before calling
//! parser work finished. They `.unwrap()` on `File::open`, so they panic
//! rather than skip when the fixtures are absent.

use apfs::catalog::*;
use apfs::fletcher::*;
use apfs::omap::*;
use apfs::superblock::*;
use apfs::*;
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::{BufReader, Cursor, Seek, SeekFrom};

/// Requires ../tests/appfs.raw fixture. Run with `cargo test -- --ignored`.
fn open_appfs() -> BufReader<std::fs::File> {
    let file = std::fs::File::open("../tests/appfs.raw").unwrap();
    BufReader::new(file)
}

fn open_volume() -> (BufReader<std::fs::File>, u64, u64, u32) {
    let file = std::fs::File::open("../tests/appfs.raw").unwrap();
    let mut reader = BufReader::new(file);

    let nxsb = apfs::superblock::read_nxsb(&mut reader).unwrap();
    let latest = apfs::superblock::find_latest_nxsb(&mut reader, &nxsb).unwrap();
    let block_size = latest.block_size;

    let container_omap_root =
        apfs::omap::read_omap_tree_root(&mut reader, latest.omap_oid, block_size).unwrap();

    let vol_oid = latest.fs_oids.iter().find(|&&o| o != 0).copied().unwrap();
    let vol_block =
        apfs::omap::omap_lookup(&mut reader, container_omap_root, block_size, vol_oid).unwrap();

    let vol_data = apfs::object::read_block(&mut reader, vol_block, block_size).unwrap();
    let vol_sb = apfs::superblock::ApfsSuperblock::parse(&vol_data).unwrap();

    let vol_omap_root =
        apfs::omap::read_omap_tree_root(&mut reader, vol_sb.omap_oid, block_size).unwrap();
    let catalog_root =
        apfs::omap::omap_lookup(&mut reader, vol_omap_root, block_size, vol_sb.root_tree_oid)
            .unwrap();

    (reader, catalog_root, vol_omap_root, block_size)
}

#[test]
#[ignore]
fn test_read_file() {
    let file = std::fs::File::open("../tests/appfs.raw").unwrap();
    let reader = std::io::BufReader::new(file);
    let mut vol = apfs::ApfsVolume::open(reader).unwrap();

    let walk = vol.walk().unwrap();
    let small_file = walk.iter().find(|e| {
        e.entry.kind == apfs::EntryKind::File && e.entry.size > 0 && e.entry.size < 100_000
    });

    let entry = small_file.expect("Should find a small file in the test image");
    let data = vol.read_file(&entry.path).unwrap();
    assert!(!data.is_empty(), "File data should not be empty");
    assert_eq!(data.len() as u64, entry.entry.size);
}
/// Requires ../tests/appfs.raw fixture. Run with `cargo test -- --ignored`.
#[test]
#[ignore]
fn test_parse_nxsb() {
    let mut reader = open_appfs();

    let nxsb = read_nxsb(&mut reader).unwrap();
    assert_eq!(nxsb.magic, NX_MAGIC);
    assert_eq!(nxsb.block_size, 4096);
    assert!(nxsb.block_count > 0);

    let file_size = reader.seek(SeekFrom::End(0)).unwrap();
    let expected_size = nxsb.block_count * nxsb.block_size as u64;
    assert_eq!(
        file_size, expected_size,
        "File size {} should match block_count({}) * block_size({})",
        file_size, nxsb.block_count, nxsb.block_size
    );
}
/// Requires ../tests/appfs.raw fixture. Run with `cargo test -- --ignored`.
#[test]
#[ignore]
fn test_checkpoint_scan() {
    let mut reader = open_appfs();

    let nxsb = read_nxsb(&mut reader).unwrap();
    let latest = find_latest_nxsb(&mut reader, &nxsb).unwrap();

    assert!(
        latest.header.xid >= nxsb.header.xid,
        "Latest xid {} should be >= block 0 xid {}",
        latest.header.xid,
        nxsb.header.xid
    );
}
/// Requires ../tests/appfs.raw fixture. Run with `cargo test -- --ignored`.
#[test]
#[ignore]
fn test_volume_superblock() {
    let mut reader = open_appfs();

    let nxsb = read_nxsb(&mut reader).unwrap();
    let latest = find_latest_nxsb(&mut reader, &nxsb).unwrap();

    assert!(
        latest.fs_oids.iter().any(|&o| o != 0),
        "Should have at least one volume"
    );

    let omap_block =
        apfs::object::read_block(&mut reader, latest.omap_oid, latest.block_size).unwrap();
    let omap_header = apfs::object::ObjectHeader::parse(&omap_block).unwrap();
    assert_ne!(omap_header.object_type(), 0);

    let mut cursor = Cursor::new(&omap_block[32..]);
    let _om_flags = cursor.read_u32::<LittleEndian>().unwrap();
    let _om_snap_count = cursor.read_u32::<LittleEndian>().unwrap();
    let _om_tree_type = cursor.read_u32::<LittleEndian>().unwrap();
    let _om_snap_tree_type = cursor.read_u32::<LittleEndian>().unwrap();
    let om_tree_oid = cursor.read_u64::<LittleEndian>().unwrap();
    assert!(om_tree_oid > 0);
}
/// Requires ../tests/appfs.raw fixture. Run with `cargo test -- --ignored`.
#[test]
#[ignore]
fn test_list_root() {
    let (mut reader, catalog_root, omap_root, block_size) = open_volume();

    let entries = list_directory(
        &mut reader,
        catalog_root,
        omap_root,
        block_size,
        ROOT_DIR_RECORD,
    )
    .unwrap();
    assert!(!entries.is_empty(), "Root directory should have entries");
}
/// Requires ../tests/appfs.raw fixture. Run with `cargo test -- --ignored`.
#[test]
#[ignore]
fn test_resolve_path() {
    let (mut reader, catalog_root, omap_root, block_size) = open_volume();

    let entries = list_directory(
        &mut reader,
        catalog_root,
        omap_root,
        block_size,
        ROOT_DIR_RECORD,
    )
    .unwrap();
    let first = entries.first().expect("Root should have entries");
    let path = format!("/{}", first.name);
    let (oid, inode) =
        resolve_path(&mut reader, catalog_root, omap_root, block_size, &path).unwrap();
    assert!(oid > 0);
    assert!(inode.kind() != 0);
}
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
    assert!(
        vol_block > 0 && vol_block < latest.block_count,
        "Physical block {} should be within container",
        vol_block
    );

    let vol_data = object::read_block(&mut reader, vol_block, latest.block_size).unwrap();
    let vol_sb = superblock::ApfsSuperblock::parse(&vol_data).unwrap();
    assert_eq!(vol_sb.magic, superblock::APSB_MAGIC);
}
/// Requires ../tests/appfs.raw fixture. Run with `cargo test -- --ignored`.
#[test]
#[ignore]
fn test_fletcher64_known() {
    let mut file = std::fs::File::open("../tests/appfs.raw").unwrap();
    use std::io::Read;
    let mut block = vec![0u8; 4096];
    file.read_exact(&mut block).unwrap();

    assert!(verify_object(&block), "Block 0 checksum should be valid");

    let stored = u64::from_le_bytes([
        block[0], block[1], block[2], block[3], block[4], block[5], block[6], block[7],
    ]);
    let computed = fletcher64(&block[8..]);
    assert_eq!(
        stored, computed,
        "Stored checksum 0x{:016X} should match computed 0x{:016X}",
        stored, computed
    );
}
/// Requires ../tests/appfs.raw fixture. Run with `cargo test -- --ignored`.
#[test]
#[ignore]
fn test_volume_open() {
    let file = std::fs::File::open("../tests/appfs.raw").unwrap();
    let reader = BufReader::new(file);

    let mut vol = ApfsVolume::open(reader).unwrap();
    let info = vol.volume_info();

    assert!(!info.name.is_empty(), "Volume name should not be empty");
    assert_eq!(info.block_size, 4096);

    let entries = vol.list_directory("/").unwrap();
    assert!(!entries.is_empty(), "Root directory should have entries");

    let walk_entries = vol.walk().unwrap();
    assert!(!walk_entries.is_empty());
}
/// Requires ../tests/appfs.raw fixture. Run with `cargo test -- --ignored`.
#[test]
#[ignore]
fn test_read_file_data() {
    let file = std::fs::File::open("../tests/appfs.raw").unwrap();
    let reader = BufReader::new(file);

    let mut vol = ApfsVolume::open(reader).unwrap();

    let walk = vol.walk().unwrap();
    let small_file = walk
        .iter()
        .find(|e| e.entry.kind == EntryKind::File && e.entry.size > 0 && e.entry.size < 1_000_000);

    let entry = small_file.expect("Should find a small file in the test image");
    let data = vol.read_file(&entry.path).unwrap();
    assert_eq!(
        data.len() as u64,
        entry.entry.size,
        "Read size should match stat size"
    );

    let stat = vol.stat(&entry.path).unwrap();
    assert_eq!(stat.size, entry.entry.size);
}
/// Requires ../tests/appfs.raw fixture. Run with `cargo test -- --ignored`.
#[test]
#[ignore]
fn test_read_symlink_targets() {
    let file = std::fs::File::open("../tests/appfs.raw").unwrap();
    let reader = BufReader::new(file);

    let mut vol = ApfsVolume::open(reader).unwrap();

    let walk = vol.walk().unwrap();
    let symlinks: Vec<_> = walk
        .iter()
        .filter(|e| e.entry.kind == EntryKind::Symlink)
        .map(|e| e.path.clone())
        .collect();
    assert!(
        !symlinks.is_empty(),
        "Test image should contain symlinks to read"
    );

    for path in &symlinks {
        let target = vol.read_file(path).unwrap();
        assert!(
            !target.is_empty(),
            "Symlink {path} should resolve to a non-empty target"
        );
        assert!(
            !target.contains(&0),
            "Symlink target for {path} should have its trailing NUL stripped"
        );

        // stat() reports the target length, not the inode's zero size.
        let stat = vol.stat(path).unwrap();
        assert_eq!(stat.kind, EntryKind::Symlink);
        assert_eq!(
            stat.size,
            target.len() as u64,
            "stat size should match the target length for {path}"
        );
    }
}
/// Requires ../tests/appfs.raw. Run with `cargo test -- --ignored`.
///
/// Walks every file on the fixture, lists its extended attributes and
/// fetches each one back. Measured Sep 2026: 129 files, 126 carrying
/// attributes, 610 values across seven names — `com.apple.provenance`
/// (298), the four `com.apple.cs.*` code-signing attributes (74 each),
/// `com.apple.fs.symlink` (15) and `com.apple.FinderInfo` (1). All 610
/// resolve, so the key comparator and both storage forms are exercised
/// against a real volume rather than only synthetically.
///
/// The fixture has no decmpfs-compressed file, so it cannot cover
/// decompression; `cmpfs` and the synthetic hfsplus tests do that.
#[test]
#[ignore]
fn lists_and_reads_every_xattr_on_the_fixture() {
    let file = std::fs::File::open("../tests/appfs.raw").unwrap();
    let mut vol = ApfsVolume::open(std::io::BufReader::new(file)).unwrap();

    let entries = vol.walk().unwrap();
    let mut listed = 0usize;
    let mut fetched = 0usize;

    for entry in &entries {
        for attr in vol.list_xattrs(&entry.path).unwrap() {
            listed += 1;
            let value = vol.get_xattr(&entry.path, &attr.name).unwrap();
            assert!(
                value.is_some(),
                "{} listed {} but it could not be read back",
                entry.path,
                attr.name
            );
            // The fixture holds no compressed file, so every attribute on
            // it is user data. A future fixture with one would trip this,
            // which is the point.
            assert_eq!(attr.kind, XattrKind::User, "{} {}", entry.path, attr.name);
            fetched += 1;
        }
    }

    assert_eq!(listed, fetched);
    assert!(
        listed > 500,
        "expected hundreds of attributes, found {listed}"
    );
}
