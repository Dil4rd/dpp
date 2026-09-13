//! Fixture tests: the only coverage that runs the reader against images macOS
//! actually wrote.
//!
//! Every test here is `#[ignore]`d, so neither `cargo test` nor CI runs it —
//! the fixtures in `tests/` are gitignored and exist only on a maintainer's
//! machine. Run them with `cargo test -p udif -- --ignored` before calling
//! parser work finished. They `.unwrap()` on `File::open`, so they panic
//! rather than skip when the fixtures are absent.

use udif::*;

/// Requires ../tests/kdk.dmg fixture.
/// Run with `cargo test -- --ignored`.
#[test]
#[ignore]
fn test_real_dmg_if_available() {
    let test_dmg = "../tests/kdk.dmg";

    let archive = DmgArchive::open(test_dmg).unwrap();
    let stats = archive.stats();

    assert_eq!(stats.version, 4);
    assert!(stats.partition_count > 0);
    assert!(stats.total_uncompressed > stats.total_compressed);

    let partitions = archive.partitions();
    assert!(!partitions.is_empty());

    let hfsx = partitions.iter().find(|p| p.name.contains("HFSX"));
    assert!(hfsx.is_some(), "Should have HFSX partition");
}
/// Requires ../tests/kdk.dmg fixture.
/// Run with `cargo test -- --ignored`.
#[test]
#[ignore]
fn test_real_dmg_decompress() {
    let test_dmg = "../tests/kdk.dmg";

    let mut archive = DmgArchive::open(test_dmg).unwrap();
    let data = archive.extract_main_partition().unwrap();
    assert_eq!(&data[1024..1026], &[0x48, 0x58], "Should be HFSX signature");

    let mut archive2 = DmgArchive::open(test_dmg).unwrap();
    let mut buf = Vec::new();
    let _n = archive2.extract_main_partition_to(&mut buf).unwrap();
    assert_eq!(&buf[1024..1026], &[0x48, 0x58], "Should be HFSX signature");

    assert_eq!(data.len(), buf.len());
    assert_eq!(
        data, buf,
        "Buffered and streaming should produce identical output"
    );
}
/// Requires ../tests/googlechrome.dmg fixture (XZ-compressed DMG).
/// Run with `cargo test -- --ignored`.
#[test]
#[ignore]
fn test_xz_dmg_googlechrome() {
    let test_dmg = "../tests/googlechrome.dmg";

    let archive = DmgArchive::open(test_dmg).unwrap();
    let comp_info = archive.compression_info();

    // googlechrome.dmg uses XZ (block type 0x80000008)
    assert!(comp_info.xz_blocks > 0, "Should have XZ blocks");

    // Decompress main partition and verify non-zero data
    let mut archive = DmgArchive::open(test_dmg).unwrap();
    let data = archive.extract_main_partition().unwrap();
    assert!(
        !data.iter().all(|&b| b == 0),
        "Decompressed data should not be all zeros"
    );

    // Check for HFS+ signature (0x482B at offset 1024) or HFSX (0x4858)
    if data.len() > 1026 {
        let sig = &data[1024..1026];
        assert!(
            sig == [0x48, 0x2B] || sig == [0x48, 0x58],
            "Should have HFS+/HFSX signature, got {:02X}{:02X}",
            sig[0],
            sig[1]
        );
    }
}
