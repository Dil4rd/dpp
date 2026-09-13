//! Unit tests for the parent module, split out of `cpio.rs` for size.

use super::*;

#[test]
fn test_sanitize_path() {
    assert!(sanitize_path("normal/path/file.txt").is_ok());
    assert!(sanitize_path("/absolute/path").is_ok());
    assert!(sanitize_path("../traversal").is_err());
    assert!(sanitize_path("path/../traversal").is_err());
}

#[test]
fn test_normalize_cpio_path() {
    assert_eq!(normalize_cpio_path("./usr/bin/foo"), "usr/bin/foo");
    assert_eq!(normalize_cpio_path("/usr/bin/foo"), "usr/bin/foo");
    assert_eq!(normalize_cpio_path("usr/bin/foo"), "usr/bin/foo");
    assert_eq!(normalize_cpio_path("."), "");
    assert_eq!(normalize_cpio_path("./"), "");
    assert_eq!(normalize_cpio_path("/"), "");
    assert_eq!(normalize_cpio_path(""), "");
}

#[test]
fn test_extract_path_with_filter() {
    use crate::writer::CpioBuilder;

    let mut builder = CpioBuilder::new();
    builder.add_directory("usr", 0o755);
    builder.add_directory("usr/bin", 0o755);
    builder.add_file("usr/bin/hello", b"hello world", 0o755);
    builder.add_directory("usr/lib", 0o755);
    builder.add_file("usr/lib/libfoo.so", b"lib content", 0o644);
    builder.add_directory("etc", 0o755);
    builder.add_file("etc/config.txt", b"config", 0o644);
    let cpio_data = builder.finish();

    let tmp = tempfile::tempdir().unwrap();

    // Extract only usr/bin — base prefix is stripped from output
    let mut reader = CpioReader::new(std::io::Cursor::new(&cpio_data));
    let stats = reader.extract_path("usr/bin", tmp.path()).unwrap();
    assert_eq!(stats.files, 1);
    assert_eq!(stats.dirs, 1); // usr/bin dir itself (base)
    assert!(tmp.path().join("hello").exists());
    assert!(!tmp.path().join("usr").exists());
    assert!(!tmp.path().join("etc").exists());
}

#[test]
fn test_odc_header_keeps_values_wider_than_32_bits() {
    // odc stores filesize and mtime as 11 octal digits, a 33-bit range, so
    // values above u32::MAX are representable on the wire. Narrowing the
    // filesize wraps the byte count the reader then consumes, which
    // corrupts every entry after it.
    let filesize: u64 = 5_000_000_000;
    let mtime: u64 = 5_000_000_001;
    assert!(filesize > u64::from(u32::MAX));

    let fields: [(usize, u64); 10] = [
        (6, 0),        // dev
        (6, 1),        // ino
        (6, 0o100644), // mode
        (6, 0),        // uid
        (6, 0),        // gid
        (6, 1),        // nlink
        (6, 0),        // rdev
        (11, mtime),
        (6, 4), // namesize, including the NUL
        (11, filesize),
    ];

    let mut header = b"070707".to_vec();
    for (width, value) in fields {
        header.extend_from_slice(format!("{value:0width$o}").as_bytes());
    }
    header.extend_from_slice(b"big\0");

    let mut reader = CpioReader::new(std::io::Cursor::new(header));
    let parsed = reader.read_header().unwrap().unwrap();

    assert_eq!(parsed.filesize, filesize);
    assert_eq!(parsed.mtime, mtime);
    assert_eq!(parsed.name, "big");
}

#[test]
fn test_entries_iterator_honours_newc_padding() {
    use crate::writer::CpioBuilder;

    // 7 bytes of content needs 1 byte of newc padding. An entry read that
    // does not consume that pad leaves the stream a byte short, so every
    // later header is parsed from the wrong offset.
    let mut builder = CpioBuilder::new();
    builder.add_file("first.txt", b"content", 0o644);
    builder.add_file("second.txt", b"more", 0o644);
    let cpio_data = builder.finish();

    let mut reader = CpioReader::new(std::io::Cursor::new(&cpio_data));
    let entries: Vec<_> = reader
        .entries()
        .unwrap()
        .collect::<Result<Vec<_>>>()
        .unwrap();

    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].path, "first.txt");
    assert_eq!(entries[0].data.as_deref(), Some(&b"content"[..]));
    assert_eq!(entries[1].path, "second.txt");
    assert_eq!(entries[1].data.as_deref(), Some(&b"more"[..]));
}

#[test]
fn test_extract_all_returns_stats() {
    use crate::writer::CpioBuilder;

    let mut builder = CpioBuilder::new();
    builder.add_directory("dir", 0o755);
    builder.add_file("dir/file.txt", b"content", 0o644);
    let cpio_data = builder.finish();

    let tmp = tempfile::tempdir().unwrap();
    let mut reader = CpioReader::new(std::io::Cursor::new(&cpio_data));
    let stats = reader.extract_all(tmp.path()).unwrap();

    assert_eq!(stats.files, 1);
    assert_eq!(stats.dirs, 1);
    assert_eq!(stats.bytes, 7); // "content".len()
    assert_eq!(stats.symlinks_skipped, 0);
}

#[test]
fn test_extract_path_skips_symlinks() {
    use crate::writer::CpioBuilder;

    let mut builder = CpioBuilder::new();
    builder.add_directory("usr", 0o755);
    builder.add_file("usr/real.txt", b"real file", 0o644);
    builder.add_symlink("usr/link.txt", "real.txt", 0o777);
    let cpio_data = builder.finish();

    let tmp = tempfile::tempdir().unwrap();
    let mut reader = CpioReader::new(std::io::Cursor::new(&cpio_data));
    let stats = reader.extract_all(tmp.path()).unwrap();

    assert_eq!(stats.files, 1);
    assert_eq!(stats.symlinks_skipped, 1);
    assert!(tmp.path().join("usr/real.txt").exists());
    // Symlink is NOT created
    assert!(!tmp.path().join("usr/link.txt").exists());
}

#[test]
fn test_extract_path_rejects_traversal() {
    assert!(sanitize_path("../etc/passwd").is_err());
    assert!(sanitize_path("usr/../../etc/passwd").is_err());
}
