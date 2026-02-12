<div align="center">

# xara

**Cross-platform Rust library for reading XAR archives and macOS PKG installer packages**

[![Crates.io](https://img.shields.io/crates/v/xara.svg)](https://crates.io/crates/xara)
[![Documentation](https://docs.rs/xara/badge.svg)](https://docs.rs/xara)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
![Platform](https://img.shields.io/badge/platform-windows%20%7C%20linux%20%7C%20macos-lightgrey)

Parse `.pkg` installer files and XAR archives on any platform — extract payloads, read metadata, list contents.

**Lightweight and focused** — minimal dependencies, no signing or crypto overhead.

</div>

---

## Why xara?

**xara is a lightweight, focused XAR/PKG parser that does one thing well: extract data from macOS packages.**

| Feature | **xara** | apple-xar | zar | apple-flat-package |
|---------|:-------:|:---------:|:---:|:------------------:|
| Read XAR | ✓ | ✓ | ✓ | ✓ |
| Write XAR | ❌ | ✓ | ✓ | ❌ |
| PKG support | ✓ | ❌ | ❌ | ✓ |
| Payload extraction | ✓ | ❌ | ❌ | ✓ |
| Component packages | ✓ | ❌ | ❌ | ✓ |
| Product packages | ✓ | ❌ | ❌ | ✓ |
| Streaming | ✓ | ❌ | ❌ | ❌ |
| Signing | ❌ | ✓ | ✓ | ✓ |
| Minimal deps | ✓ | ❌ | ❌ | ❌ |

> **Example:** To extract kernel debug symbols from a macOS `.pkg`, you need both XAR parsing
> and PKG-specific logic (Distribution XML, component payloads). xara handles both in a single crate
> with only 4 dependencies.

## Features

| | |
|---|---|
| **Parse XAR archives** | Read header, decompress TOC, navigate file tree |
| **Read PKG installers** | Product and component package support |
| **Extract payloads** | Get PBZX payload data for any component |
| **Distribution XML** | Parse product package metadata |
| **PackageInfo** | Read component package metadata |
| **Streaming extraction** | Stream file data to any writer |

### XAR Format Support

| Feature | Support | Description |
|---------|:-------:|-------------|
| Zlib TOC | ✓ | Compressed table of contents |
| Gzip heap | ✓ | Gzip-compressed file data |
| Octet-stream | ✓ | Uncompressed file data |
| Checksums | ✓ | SHA-1, MD5, SHA-256 in TOC |
| Signatures | ❌ | Cryptographic signing (not needed for reading) |

### PKG Support

| Type | Support | Description |
|------|:-------:|-------------|
| Product package | ✓ | Multi-component `.pkg` with Distribution XML |
| Component package | ✓ | Single-component `.pkg` with Payload |
| Flat package | ✓ | Modern macOS flat package format |
| Bundle package | ❌ | Legacy directory-based packages |

## Quick Start

### Read XAR Archive

```rust
use xara::XarArchive;
use std::fs::File;
use std::io::BufReader;

let file = File::open("archive.xar")?;
let mut xar = XarArchive::open(BufReader::new(file))?;

// List all files
for file in xar.files() {
    println!("{}: {} bytes", file.path, file.data.as_ref().map_or(0, |d| d.size));
}

// Read a specific file
if let Some(entry) = xar.find("path/to/file.txt") {
    let entry = entry.clone();
    let data = xar.read_file(&entry)?;
}
```

### Read macOS PKG

```rust
use xara::PkgReader;

let file = File::open("installer.pkg")?;
let mut pkg = PkgReader::open(BufReader::new(file))?;

// Check package type
if pkg.is_product_package() {
    let dist = pkg.distribution()?.unwrap();
    println!("Distribution XML:\n{}", dist);
}

// List components
for component in pkg.components() {
    println!("Component: {}", component);

    // Get PackageInfo
    if let Some(info) = pkg.package_info(&component)? {
        println!("  PackageInfo: {}", &info[..80.min(info.len())]);
    }
}
```

### Extract PKG Payload

```rust
// Extract payload into memory
let payload = pkg.payload("com.apple.pkg.KDK")?;

// Or stream to file (low memory)
let mut out = File::create("Payload.pbzx")?;
pkg.payload_to("com.apple.pkg.KDK", &mut out)?;
```

## Documentation

| | |
|---|---|
| [Format Specification](docs/FORMATS.md) | XAR header, TOC XML, and heap structure |
| [CLI Tool](docs/CLI.md) | Command-line tool usage |

## Example Output

Via `dpp-tool pkg` (which uses the xara library internally):

```
$ dpp-tool pkg info Kernel_Debug_Kit.dmg /Library/Developer/KDKs/KDK.pkg

  PKG: /Library/Developer/KDKs/KDK.pkg
  ════════════════════════════════════════════════════════════

  Package
  ────────────────────────────────────────────────────────────
  Type                     Product package (multi-component)
  Components               1
  XAR version              1
  Checksum                 Sha1
  TOC size                 1.22 KB compressed, 4.78 KB uncompressed

  Contents
  ────────────────────────────────────────────────────────────
  Entries                  6
  Files                    4
  Directories              2
  Compressed size          512.30 MB
  Uncompressed size        512.30 MB
  Space savings            0.0%

  Components
  ────────────────────────────────────────────────────────────
  └── com.apple.pkg.KDK  512.30 MB compressed, 512.30 MB uncompressed
```

```
$ dpp-tool pkg ls Kernel_Debug_Kit.dmg /Library/Developer/KDKs/KDK.pkg

  XAR Contents
  ────────────────────────────────────────────────────────────
  com.apple.pkg.KDK                                          dir
  com.apple.pkg.KDK/Bom                                  1.00 KB
  com.apple.pkg.KDK/PackageInfo                            892 B
  com.apple.pkg.KDK/Payload                            512.30 MB
  Distribution                                           3.12 KB

  4 file(s), 1 directory(ies)
```

## Alternatives

| Crate | Read | Write | PKG | Payload | Streaming | Signing | Deps |
|-------|:----:|:-----:|:---:|:-------:|:---------:|:-------:|:----:|
| **xara** | ✓ | ❌ | ✓ | ✓ | ✓ | ❌ | 4 |
| [apple-xar](https://crates.io/crates/apple-xar) | ✓ | ✓ | ❌ | ❌ | ❌ | ✓ | 12+ |
| [zar](https://crates.io/crates/zar) | ✓ | ✓ | ❌ | ❌ | ❌ | ✓ | 10+ |
| [apple-flat-package](https://crates.io/crates/apple-flat-package) | ✓ | ❌ | ✓ | ✓ | ❌ | ✓ | 15+ |
| [xar](https://crates.io/crates/xar) | ✓ | ✓ | ❌ | ❌ | ❌ | ❌ | 6+ |

**Choose xara if you need:**
- Lightweight PKG/XAR reading with minimal dependencies
- Streaming payload extraction (low memory footprint)
- Integration with the `dpp` pipeline (DMG → HFS+ → **PKG** → PBZX)
- Simple API that handles both product and component packages

**Choose apple-xar / zar if you need:**
- XAR archive creation/writing
- Digital signature verification
- Standalone XAR operations without PKG semantics

**Choose apple-flat-package if you need:**
- Full PKG signing and verification
- Bill of Materials (BOM) parsing
- Integration with the apple-platform-rs ecosystem

## Next Steps

- [ ] **Write support** — create XAR archives from files
- [ ] **Bzip2 heap** — decompress bzip2-encoded file data
- [ ] **LZMA heap** — decompress LZMA-encoded file data
- [ ] **Signature verification** — validate RSA/X.509 signatures
- [ ] **BOM parsing** — read Bill of Materials for component packages
- [ ] **Scripts extraction** — extract pre/postinstall scripts from components
- [ ] **Checksum validation** — verify extracted/archived checksums from TOC
- [ ] **Bundle packages** — support legacy directory-based `.pkg` format

## License

MIT
