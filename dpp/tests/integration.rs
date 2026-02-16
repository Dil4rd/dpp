use std::io::BufReader;
use udif::PartitionType;

// ---------------------------------------------------------------------------
// Synthetic test fixture builder (no external files needed)
// ---------------------------------------------------------------------------

/// Build a complete DMG containing an HFSX partition with a hello.txt and
/// a valid .pkg (XAR → PBZX → CPIO) file. Returns the raw DMG bytes.
fn build_test_dmg() -> Vec<u8> {
    // --- Layer 1: CPIO archive with a test file ---
    let mut cpio = pbzx::writer::CpioBuilder::new();
    cpio.add_directory("usr", 0o755);
    cpio.add_file("usr/hello.txt", b"Hello from CPIO!\n", 0o644);
    let cpio_data = cpio.finish();

    // --- Layer 2: PBZX wrapping the CPIO ---
    let pbzx_data = {
        let mut writer = pbzx::PbzxWriter::new(Vec::new())
            .chunk_size(64 * 1024)
            .compression_level(0); // no compression for speed
        writer.write_cpio(&cpio_data).unwrap();
        writer.finish().unwrap()
    };

    // --- Layer 3: XAR/PKG archive containing the PBZX as "Payload" ---
    let pkg_data = build_xar_pkg(&pbzx_data);

    // --- Layer 4: HFS+ image with hello.txt + test.pkg ---
    let hfs_image = {
        let mut builder = hfsplus::testutil::HfsPlusImageBuilder::new();
        builder
            .add_file("hello.txt", b"Hello, World!\n", 0o644)
            .add_file("test.pkg", &pkg_data, 0o644);
        builder.build()
    };

    // --- Layer 5: DMG wrapping the HFS+ partition ---
    let mut dmg_bytes = Vec::new();
    {
        let cursor = std::io::Cursor::new(&mut dmg_bytes);
        let mut writer = udif::DmgWriter::new(cursor)
            .compression(udif::CompressionMethod::Raw)
            .skip_checksums(true);
        writer.add_partition("Apple_HFSX", &hfs_image).unwrap();
        writer.finish().unwrap();
    }
    dmg_bytes
}

/// Build a minimal XAR archive (component .pkg) with the given data as "Payload".
fn build_xar_pkg(payload: &[u8]) -> Vec<u8> {
    use flate2::Compression;
    use flate2::write::ZlibEncoder;
    use std::io::Write;

    let toc_xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<xar>
  <toc>
    <file id="1">
      <name>Payload</name>
      <type>file</type>
      <data>
        <offset>0</offset>
        <length>{len}</length>
        <size>{len}</size>
        <encoding style="application/octet-stream"/>
      </data>
    </file>
  </toc>
</xar>"#,
        len = payload.len()
    );

    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(toc_xml.as_bytes()).unwrap();
    let compressed_toc = encoder.finish().unwrap();

    let mut buf = Vec::new();
    // XAR header (28 bytes, big-endian)
    buf.extend_from_slice(&0x78617221u32.to_be_bytes()); // magic "xar!"
    buf.extend_from_slice(&28u16.to_be_bytes()); // header_size
    buf.extend_from_slice(&1u16.to_be_bytes()); // version
    buf.extend_from_slice(&(compressed_toc.len() as u64).to_be_bytes());
    buf.extend_from_slice(&(toc_xml.len() as u64).to_be_bytes());
    buf.extend_from_slice(&0u32.to_be_bytes()); // checksum_algo = none

    // Compressed TOC
    buf.extend_from_slice(&compressed_toc);

    // Heap: raw payload data
    buf.extend_from_slice(payload);

    buf
}

/// Write the test DMG to a temp file and return the handle (keeps file alive).
fn write_test_dmg_to_file() -> tempfile::NamedTempFile {
    use std::io::Write;
    let data = build_test_dmg();
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    tmp.write_all(&data).unwrap();
    tmp
}

// ---------------------------------------------------------------------------
// Non-ignored tests using synthetic fixtures
// ---------------------------------------------------------------------------

#[test]
fn test_pipeline_dmg_to_hfs() {
    let tmp = write_test_dmg_to_file();
    let mut pipeline = dpp::DmgPipeline::open(tmp.path()).unwrap();

    let partitions = pipeline.partitions();
    assert!(!partitions.is_empty(), "DMG should have partitions");
    assert!(
        partitions
            .iter()
            .any(|p| p.partition_type == PartitionType::Hfsx),
        "Should have an HFSX partition"
    );

    let mut hfs = pipeline
        .open_hfs_with_mode(dpp::ExtractMode::InMemory)
        .unwrap();

    let entries = hfs.list_directory("/").unwrap();
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, vec!["hello.txt", "test.pkg"]);

    let data = hfs.read_file("/hello.txt").unwrap();
    assert_eq!(data, b"Hello, World!\n");
}

#[test]
fn test_pipeline_dmg_to_pkg() {
    let tmp = write_test_dmg_to_file();
    let mut pipeline = dpp::DmgPipeline::open(tmp.path()).unwrap();
    let mut hfs = pipeline
        .open_hfs_with_mode(dpp::ExtractMode::InMemory)
        .unwrap();

    let pkg = hfs.open_pkg("/test.pkg").unwrap();
    let files = pkg.xar().files();
    assert!(!files.is_empty(), "PKG should contain files");

    let payload_entry = pkg
        .xar()
        .find("Payload")
        .expect("PKG should have a Payload entry");
    assert_eq!(payload_entry.name, "Payload");
    assert!(payload_entry.data.is_some());

    // It should be a component package
    let components = pkg.components();
    assert_eq!(components.len(), 1);
    assert_eq!(components[0], ""); // empty string = component package
}

#[test]
fn test_pipeline_full() {
    let tmp = write_test_dmg_to_file();
    let mut pipeline = dpp::DmgPipeline::open(tmp.path()).unwrap();
    let mut hfs = pipeline
        .open_hfs_with_mode(dpp::ExtractMode::InMemory)
        .unwrap();

    // Open .pkg
    let mut pkg = hfs.open_pkg("/test.pkg").unwrap();
    let components = pkg.components();
    assert_eq!(components.len(), 1);

    // Extract Payload (PBZX data)
    let payload_data = pkg.payload(&components[0]).unwrap();
    assert!(
        payload_data.len() >= 4,
        "Payload should be at least 4 bytes"
    );
    assert_eq!(
        &payload_data[..4],
        b"pbzx",
        "Payload should start with PBZX magic"
    );

    // Decompress PBZX → CPIO
    let archive = pbzx::Archive::from_reader(std::io::Cursor::new(payload_data)).unwrap();
    let entries = archive.list().unwrap();
    assert!(!entries.is_empty(), "PBZX should contain CPIO entries");

    // Verify the CPIO contains our test file
    let hello = entries.iter().find(|e| e.path == "usr/hello.txt");
    assert!(hello.is_some(), "Should find usr/hello.txt in CPIO");
}

#[test]
fn test_pipeline_find_packages() {
    let tmp = write_test_dmg_to_file();

    let pkgs = dpp::pipeline::find_packages(tmp.path()).unwrap();
    assert_eq!(pkgs.len(), 1);
    assert_eq!(pkgs[0], "/test.pkg");
}

#[test]
fn test_pipeline_filesystem_auto_detect() {
    let tmp = write_test_dmg_to_file();
    let mut pipeline = dpp::DmgPipeline::open(tmp.path()).unwrap();

    let mut fs = pipeline
        .open_filesystem_with_mode(dpp::ExtractMode::InMemory)
        .unwrap();
    assert!(fs.as_hfs().is_some(), "Should auto-detect as HFS+");

    let entries = fs.list_directory("/").unwrap();
    assert_eq!(entries.len(), 2);

    let walk = fs.walk().unwrap();
    assert_eq!(walk.len(), 2);

    assert!(fs.exists("/hello.txt").unwrap());
    assert!(fs.exists("/test.pkg").unwrap());
    assert!(!fs.exists("/missing").unwrap());
}

// ---------------------------------------------------------------------------
// Existing fixture-based tests (require external files, run with --ignored)
// ---------------------------------------------------------------------------

/// Requires ../tests/hfsp.raw fixture. Run with `cargo test -- --ignored`.
#[test]
#[ignore]
fn test_hfsplus_to_xar_to_pbzx() {
    let file = std::fs::File::open("../tests/hfsp.raw").unwrap();
    let reader = BufReader::new(file);
    let mut volume = hfsplus::HfsVolume::open(reader).unwrap();

    let entries = volume.list_directory("/").unwrap();
    let pkg_entry = entries
        .iter()
        .find(|e| e.name.ends_with(".pkg"))
        .expect("Should find a .pkg in root directory");

    let pkg_path = format!("/{}", pkg_entry.name);
    let mut fork_reader = volume.open_file(&pkg_path).unwrap();
    let mut xar = xara::XarArchive::open(&mut fork_reader).unwrap();

    assert!(!xar.files().is_empty(), "XAR should contain files");

    let payload = {
        let payloads: Vec<_> = xar
            .files()
            .iter()
            .filter(|f| f.name == "Payload" && f.data.is_some())
            .collect();
        assert!(
            !payloads.is_empty(),
            "PKG should contain at least one Payload entry"
        );

        let smallest = payloads
            .iter()
            .min_by_key(|f| f.data.as_ref().unwrap().length)
            .unwrap();
        (*smallest).clone()
    };

    let mut payload_bytes = Vec::new();
    let payload_size = xar.read_file_to(&payload, &mut payload_bytes).unwrap();
    assert!(payload_size >= 4, "Payload should be at least 4 bytes");

    if &payload_bytes[..4] == b"pbzx" {
        let cursor = std::io::Cursor::new(payload_bytes);
        let archive = pbzx::Archive::from_reader(cursor).unwrap();
        let file_entries = archive.list().unwrap();
        assert!(!file_entries.is_empty(), "PBZX should contain CPIO entries");
    }
}

/// Requires ../tests/kdk.dmg fixture.
/// Run with `cargo test -- --ignored`.
#[test]
#[ignore]
fn test_dmg_pipeline() {
    let path = "../tests/kdk.dmg";

    let mut pipeline = dpp::DmgPipeline::open(path).unwrap();
    let partitions = pipeline.partitions();
    assert!(!partitions.is_empty());

    let mut hfs = pipeline
        .open_hfs_with_mode(dpp::ExtractMode::TempFile)
        .unwrap();

    let entries = hfs.list_directory("/").unwrap();
    let pkg_entry = entries
        .iter()
        .find(|e| e.name.ends_with(".pkg"))
        .expect("Should find a .pkg in root directory");

    let pkg_path = format!("/{}", pkg_entry.name);
    let mut pkg = hfs.open_pkg_streaming(&pkg_path).unwrap();

    let components = pkg.components();
    let smallest_component = components
        .iter()
        .min_by_key(|c| {
            let p = if c.is_empty() {
                "Payload".to_string()
            } else {
                format!("{}/Payload", c)
            };
            pkg.xar()
                .find(&p)
                .and_then(|f| f.data.as_ref())
                .map(|d| d.size)
                .unwrap_or(u64::MAX)
        })
        .unwrap()
        .clone();

    let payload_data = pkg.payload(&smallest_component).unwrap();
    assert!(
        payload_data.len() >= 4,
        "Payload should be at least 4 bytes"
    );

    if &payload_data[..4] == b"pbzx" {
        let archive = pbzx::Archive::from_reader(std::io::Cursor::new(payload_data)).unwrap();
        let entries = archive.list().unwrap();
        assert!(!entries.is_empty(), "PBZX should contain files");
    }
}

/// Requires ../tests/payload.bin fixture. Run with `cargo test -- --ignored`.
#[test]
#[ignore]
fn test_pbzx_standalone() {
    let archive = pbzx::Archive::open("../tests/payload.bin").unwrap();
    let entries = archive.list().unwrap();
    assert!(!entries.is_empty());
}

/// Requires ../tests/upscayl.dmg fixture. Run with `cargo test -- --ignored`.
#[test]
#[ignore]
fn test_apfs_dmg_no_crash() {
    let path = "../tests/upscayl.dmg";

    let mut pipeline = dpp::DmgPipeline::open(path).unwrap();
    let partitions = pipeline.partitions();

    assert!(
        partitions
            .iter()
            .any(|p| p.partition_type == PartitionType::Apfs),
        "Expected APFS partition in this DMG"
    );

    assert!(
        !partitions
            .iter()
            .any(|p| p.partition_type.is_hfs_compatible()),
        "Did not expect HFS partition in this DMG"
    );

    // HFS+ should fail
    let result = pipeline.open_hfs();
    assert!(
        matches!(result, Err(dpp::DppError::NoHfsPartition)),
        "Expected NoHfsPartition error, got: {:?}",
        result.as_ref().map(|_| "(ok)").unwrap_or("other error")
    );

    // APFS should succeed
    let mut apfs = pipeline
        .open_apfs()
        .expect("APFS partition should open successfully");
    let vi = apfs.volume_info();
    assert!(!vi.name.is_empty(), "Volume name should not be empty");

    let entries = apfs.list_directory("/").unwrap();
    assert!(!entries.is_empty(), "Root directory should have entries");
}

/// Requires ../tests/upscayl.dmg fixture. Run with `cargo test -- --ignored`.
/// Full APFS DMG pipeline test: open DMG, extract APFS partition, list root, walk filesystem.
#[test]
#[ignore]
fn test_apfs_dmg_pipeline() {
    let path = "../tests/upscayl.dmg";

    let mut pipeline = dpp::DmgPipeline::open(path).unwrap();

    // open_filesystem should auto-detect APFS
    let mut fs = pipeline.open_filesystem().unwrap();
    assert!(fs.as_apfs().is_some(), "Expected APFS filesystem handle");

    let root = fs.list_directory("/").unwrap();
    assert!(!root.is_empty(), "Root should have entries");

    let walk = fs.walk().unwrap();
    assert!(!walk.is_empty(), "Walk should return entries");

    let file_count = walk
        .iter()
        .filter(|e| e.entry.kind == dpp::FsEntryKind::File)
        .count();
    assert!(file_count > 0, "Should have at least one file");
}

/// Requires ../tests/appfs.raw fixture. Run with `cargo test -- --ignored`.
/// Test opening a raw APFS image directly (not through DMG pipeline).
#[test]
#[ignore]
fn test_apfs_open_and_read() {
    let file = std::fs::File::open("../tests/appfs.raw").unwrap();
    let reader = BufReader::new(file);

    let mut vol = apfs::ApfsVolume::open(reader).unwrap();
    let info = vol.volume_info();

    assert!(!info.name.is_empty(), "Volume name should not be empty");
    assert_eq!(info.block_size, 4096);

    let entries = vol.list_directory("/").unwrap();
    assert!(!entries.is_empty(), "Root directory should have entries");

    let walk = vol.walk().unwrap();
    assert!(!walk.is_empty(), "Walk should return entries");

    // Find and read a small file
    let small_file = walk.iter().find(|e| {
        e.entry.kind == apfs::EntryKind::File && e.entry.size > 0 && e.entry.size < 1_000_000
    });

    if let Some(entry) = small_file {
        let data = vol.read_file(&entry.path).unwrap();
        assert_eq!(
            data.len() as u64,
            entry.entry.size,
            "Read size should match stat size"
        );

        let stat = vol.stat(&entry.path).unwrap();
        assert_eq!(stat.size, entry.entry.size);
        assert!(vol.exists(&entry.path).unwrap());
    }
}
