//! Fixture tests: the only coverage that runs the reader against images macOS
//! actually wrote.
//!
//! Every test here is `#[ignore]`d, so neither `cargo test` nor CI runs it —
//! the fixtures in `tests/` are gitignored and exist only on a maintainer's
//! machine. Run them with `cargo test -p hfsplus -- --ignored` before calling
//! parser work finished. They `.unwrap()` on `File::open`, so they panic
//! rather than skip when the fixtures are absent.

use hfsplus::btree::*;
use hfsplus::catalog::*;
use hfsplus::extents::*;
use hfsplus::volume::*;
use hfsplus::*;
use std::io::{BufReader, Read, Seek, SeekFrom};

fn open_hfsp() -> (BufReader<std::fs::File>, VolumeHeader, BTreeHeaderRecord) {
    let file = std::fs::File::open("../tests/hfsp.raw").unwrap();
    let mut reader = BufReader::new(file);
    let vol = VolumeHeader::parse(&mut reader).unwrap();
    let catalog_header =
        btree::read_btree_header(&mut reader, &vol.catalog_file, vol.block_size).unwrap();
    (reader, vol, catalog_header)
}

/// Requires ../tests/hfsp.raw fixture. Run with `cargo test -- --ignored`.
///
/// Covers catalog lookup, extent-to-physical mapping and `ForkReader`
/// against a real volume. Previously looked for `KernelDebugKit.pkg`,
/// which is not on `hfsp.raw` — that is the Google Chrome volume and has
/// no `.pkg` at all.
///
/// `.VolumeIcon.icns` is a better oracle than the pkg was: ICNS stores its
/// own total length big-endian at offset 4, so the fork can be
/// cross-checked against a value carried inside the file data rather than
/// against a magic alone. At 72059 bytes over 2048-byte blocks it spans
/// ~36 blocks, so the mapping is genuinely exercised.
#[test]
#[ignore]
fn test_read_file_header_through_fork_reader() {
    const ICNS_MAGIC: &[u8; 4] = b"icns";

    let file = std::fs::File::open("../tests/hfsp.raw").unwrap();
    let mut reader = std::io::BufReader::new(file);
    let vol = hfsplus::volume::VolumeHeader::parse(&mut reader).unwrap();
    let catalog_header =
        btree::read_btree_header(&mut reader, &vol.catalog_file, vol.block_size).unwrap();
    let _extents_header =
        btree::read_btree_header(&mut reader, &vol.extents_file, vol.block_size).unwrap();

    let record = hfsplus::catalog::lookup_catalog(
        &mut reader,
        &vol,
        &catalog_header,
        hfsplus::catalog::CNID_ROOT_FOLDER,
        ".VolumeIcon.icns",
    )
    .unwrap();

    let file_rec = match record {
        Some(hfsplus::catalog::CatalogRecord::File(f)) => f,
        other => panic!(
            "Expected File record, got {:?}",
            other.map(|r| format!("{:?}", r))
        ),
    };

    // Raw read at the first extent's physical offset.
    let offset = file_rec.data_fork.extents[0].start_block as u64 * vol.block_size as u64;
    reader.seek(std::io::SeekFrom::Start(offset)).unwrap();
    let mut magic = [0u8; 4];
    reader.read_exact(&mut magic).unwrap();
    assert_eq!(
        &magic, ICNS_MAGIC,
        "first extent should hold the ICNS magic"
    );

    let mut fork_reader = ForkReader::new(&mut reader, &file_rec.data_fork, vol.block_size);

    let mut header = [0u8; 8];
    fork_reader.read_exact(&mut header).unwrap();
    assert_eq!(&header[..4], ICNS_MAGIC, "ForkReader should read the magic");

    // ICNS carries its own length; it must agree with the catalog.
    let declared = u32::from_be_bytes(header[4..8].try_into().unwrap()) as u64;
    assert_eq!(
        declared, file_rec.data_fork.logical_size,
        "ICNS internal length should match the fork's logical size"
    );

    fork_reader.seek(SeekFrom::Start(0)).unwrap();
    let mut magic2 = [0u8; 4];
    fork_reader.read_exact(&mut magic2).unwrap();
    assert_eq!(&magic2, ICNS_MAGIC, "ForkReader seek+read should work");

    let end = fork_reader.seek(SeekFrom::End(0)).unwrap();
    assert_eq!(
        end, file_rec.data_fork.logical_size,
        "SeekFrom::End should match file size"
    );
}
/// Requires ../tests/hfsp.raw fixture. Run with `cargo test -- --ignored`.
#[test]
#[ignore]
fn test_list_root_directory() {
    let (mut reader, vol, catalog_header) = open_hfsp();

    let entries = list_directory(&mut reader, &vol, &catalog_header, CNID_ROOT_FOLDER).unwrap();
    assert!(!entries.is_empty(), "Root directory should not be empty");
}
/// Requires ../tests/hfsp.raw fixture. Run with `cargo test -- --ignored`.
#[test]
#[ignore]
fn test_resolve_root_path() {
    let (mut reader, vol, catalog_header) = open_hfsp();

    let entries = list_directory(&mut reader, &vol, &catalog_header, CNID_ROOT_FOLDER).unwrap();
    let first = entries.first().expect("Root should have entries");
    let path = format!("/{}", first.name);
    let (_record, _name) = resolve_path(&mut reader, &vol, &catalog_header, &path).unwrap();
}
/// Requires ../tests/hfsp.raw fixture. Run with `cargo test -- --ignored`.
/// Requires ../tests/hfsp.raw fixture. Run with `cargo test -- --ignored`.
///
/// `hfsp.raw` is the Google Chrome volume: plain HFS+ (0x482B, version 4),
/// not HFSX. It was previously asserted to be HFSX, which it has never
/// been.
#[test]
#[ignore]
fn test_parse_hfs_plus_volume_header() {
    let file = std::fs::File::open("../tests/hfsp.raw").unwrap();
    let mut reader = std::io::BufReader::new(file);
    let header = VolumeHeader::parse(&mut reader).unwrap();

    assert!(!header.is_hfsx, "hfsp.raw is HFS+, not HFSX");
    assert_eq!(header.signature, HFS_PLUS_SIGNATURE);
    assert_eq!(header.version, HFS_PLUS_VERSION);
    assert!(header.block_size > 0);
    assert!(header.total_blocks > 0);
    assert!(header.file_count > 0);
    assert!(header.folder_count > 0);
    assert!(header.catalog_file.logical_size > 0);
}
/// Requires ../tests/hfsp.raw fixture. Run with `cargo test -- --ignored`.
#[test]
#[ignore]
fn test_read_btree_header_from_real_volume() {
    let file = std::fs::File::open("../tests/hfsp.raw").unwrap();
    let mut reader = std::io::BufReader::new(file);
    let vol = hfsplus::volume::VolumeHeader::parse(&mut reader).unwrap();

    let catalog_header = read_btree_header(&mut reader, &vol.catalog_file, vol.block_size).unwrap();

    assert!(catalog_header.node_size > 0);
    assert!(catalog_header.root_node > 0);
    assert!(catalog_header.leaf_records > 0);
}
