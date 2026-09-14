//! Decode payloads that macOS itself wrote.
//!
//! Requires `../tests/decmpfs`, generated on a Mac: see the layout below.
//! Ignored like the other fixture tests, so neither `cargo test` nor CI runs
//! it:
//!
//! ```text
//! cargo test -p cmpfs -- --ignored
//! ```
//!
//! This is the only coverage that checks `cmpfs` against bytes Apple produced
//! rather than bytes reconstructed from a reference implementation. Types 8, 10
//! and 12 rest on a single reader that marks two of them assumptions, so they
//! are the reason this exists.
//!
//! # Fixture layout
//!
//! Per case, taken from a file macOS has compressed — `ditto --hfsCompression`
//! produces one:
//!
//! - `<name>.decmpfs` — the `com.apple.decmpfs` attribute, verbatim
//! - `<name>.rsrc` — the resource fork, verbatim, for the types that use one
//! - `<name>.expected` — the original uncompressed bytes
//!
//! plus `manifest.tsv`, whose rows are `name`, compression type, declared
//! size, actual size, attribute length, resource-fork length, sha256 and a
//! description. Lines beginning `#` are comments.
//!
//! Capturing the first two needs `getxattr` with `XATTR_SHOWCOMPRESSION`
//! (`bsd/sys/xattr.h`): the kernel hides both on a compressed file, so
//! `xattr(1)` reports neither. See `docs/FORMATS.md`.

use std::path::{Path, PathBuf};

const DIR: &str = "../tests/decmpfs";

struct Fixture {
    name: String,
    compression_type: u32,
    declared_size: u64,
    actual_size: u64,
    purpose: String,
}

/// Parse the manifest the minting script writes. Tab-separated so `cmpfs`
/// needs no dev-dependency to read it.
fn manifest() -> Vec<Fixture> {
    let path = PathBuf::from(DIR).join("manifest.tsv");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "{}: {e}\ngenerate the fixtures on a Mac; see this file's header",
            path.display()
        )
    });

    text.lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
        .map(|line| {
            let f: Vec<&str> = line.split('\t').collect();
            assert!(f.len() >= 8, "malformed manifest row: {line:?}");
            Fixture {
                name: f[0].to_string(),
                compression_type: f[1].parse().unwrap(),
                declared_size: f[2].parse().unwrap(),
                actual_size: f[3].parse().unwrap(),
                purpose: f[7].to_string(),
            }
        })
        .collect()
}

fn read(name: &str) -> Option<Vec<u8>> {
    let path = Path::new(DIR).join(name);
    std::fs::read(&path).ok()
}

#[test]
#[ignore]
fn decodes_every_payload_macos_wrote() {
    let fixtures = manifest();
    assert!(!fixtures.is_empty(), "manifest is empty");

    for f in &fixtures {
        let attr = read(&format!("{}.decmpfs", f.name))
            .unwrap_or_else(|| panic!("{}: missing .decmpfs", f.name));
        let expected = read(&format!("{}.expected", f.name))
            .unwrap_or_else(|| panic!("{}: missing .expected", f.name));
        let rsrc = read(&format!("{}.rsrc", f.name));

        // The manifest must agree with the attribute, or the fixture set and
        // the manifest have drifted apart.
        let header =
            cmpfs::Header::parse(&attr).unwrap_or_else(|e| panic!("{}: header: {e}", f.name));
        assert_eq!(header.compression_type, f.compression_type, "{}", f.name);
        assert_eq!(header.uncompressed_size, f.declared_size, "{}", f.name);
        assert_eq!(f.declared_size, f.actual_size, "{}", f.name);

        // A resource fork must be present exactly when the type wants one.
        match header.storage() {
            cmpfs::Storage::ResourceFork => assert!(
                rsrc.is_some(),
                "{} is type {} but has no .rsrc",
                f.name,
                f.compression_type
            ),
            cmpfs::Storage::Xattr => assert!(
                rsrc.is_none(),
                "{} is type {} yet carries a resource fork",
                f.name,
                f.compression_type
            ),
            other => panic!("{}: macOS wrote storage {other:?}", f.name),
        }

        let got = cmpfs::decompress(&attr, rsrc.as_deref()).unwrap_or_else(|e| {
            panic!(
                "{} (type {}, {}): {e}",
                f.name, f.compression_type, f.purpose
            )
        });
        assert_eq!(
            got, expected,
            "{} (type {}) decoded to the wrong bytes",
            f.name, f.compression_type
        );
    }

    let mut types: Vec<u32> = fixtures.iter().map(|f| f.compression_type).collect();
    types.sort_unstable();
    types.dedup();
    eprintln!("{} fixtures, compression types {types:?}", fixtures.len());
}

/// The fixtures are only worth keeping if they reach the paths that no second
/// reference covers. Fails loudly if a future macOS stops emitting them, since
/// that silently turns this suite into a no-op for the provisional code.
#[test]
#[ignore]
fn covers_the_provisional_resource_fork_types() {
    let fixtures = manifest();
    let types: Vec<u32> = fixtures.iter().map(|f| f.compression_type).collect();

    let provisional = [8u32, 10, 12];
    assert!(
        types.iter().any(|t| provisional.contains(t)),
        "no fixture uses a resource-fork type that only apfs-fuse documents \
         (8, 10 or 12); found {types:?}. Either this macOS emits something \
         else, or the fixtures need regenerating with larger files."
    );
}
