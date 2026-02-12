<div align="center">

# dpp

**All-in-one Rust pipeline for Apple DMG → HFS+/APFS → PKG → PBZX extraction**

[![Crates.io](https://img.shields.io/crates/v/dpp.svg)](https://crates.io/crates/dpp)
[![Documentation](https://docs.rs/dpp/badge.svg)](https://docs.rs/dpp)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
![Platform](https://img.shields.io/badge/platform-windows%20%7C%20linux%20%7C%20macos-lightgrey)

Open a `.dmg`, browse the HFS+ or APFS filesystem inside, extract `.pkg` installers, and unpack their payloads — all in a single pipeline, on any platform.

**No macOS required** — works on Linux and Windows too.

</div>

---

## Why dpp?

**dpp is the only Rust library that chains the entire Apple package extraction pipeline into a single API.**

Without dpp, extracting files from a macOS `.dmg` requires 4+ separate tools:

```
DMG file → [dmg tool] → raw partition → [hfs/apfs tool] → pkg file → [xar tool] → payload → [pbzx tool] → files
```

With dpp:

```rust
let files = dpp::extract_pkg_payload("image.dmg", "/path/to.pkg", "component")?;
```

| Feature | **dpp** | Manual pipeline |
|---------|:-------:|:---------------:|
| Single API call | ✓ | ❌ (4+ tools) |
| HFS+ and APFS support | ✓ | partial |
| Auto-detect filesystem | ✓ | ❌ |
| Memory-efficient streaming | ✓ | ❌ (temp files) |
| Type-safe error handling | ✓ | ❌ (string errors) |
| Cross-platform | ✓ | partial |
| Low memory mode | ✓ | ❌ |

## Features

| | |
|---|---|
| **Open DMG** | Parse UDIF disk images with all compression formats |
| **Browse HFS+** | Navigate HFS+ filesystems inside the DMG |
| **Browse APFS** | Navigate APFS filesystems inside the DMG |
| **Auto-detect FS** | `open_filesystem()` detects HFS+ or APFS automatically |
| **Extract PKG** | Open `.pkg` installers found on the volume |
| **Unpack PBZX** | Decompress XZ payloads and parse CPIO archives |
| **Find packages** | Auto-discover all `.pkg` files in a DMG |
| **Low memory** | Stream through temp files instead of buffering in RAM |

### Pipeline

```
                 ┌─────────┐
             ┌──▶│  HFS+   │──┐
┌─────────┐  │   │ (volume)│  │   ┌─────────┐    ┌─────────┐
│  UDIF   │──┤   └─────────┘  ├──▶│   XAR   │───▶│  PBZX   │
│  (DMG)  │  │   ┌─────────┐  │   │  (PKG)  │    │ (files) │
└─────────┘  └──▶│  APFS   │──┘   └─────────┘    └─────────┘
     udif        │ (volume)│          xara           pbzx
                 └─────────┘
              hfsplus / apfs
```

### Extraction Modes

| Mode | Memory | Speed | Best for |
|------|--------|-------|----------|
| `TempFile` (default) | Low (~4 KB) | Normal | Large DMGs (1+ GB) |
| `InMemory` | High (partition size) | Fast | Small DMGs (< 500 MB) |

## Quick Start

### Browse a DMG (auto-detect filesystem)

```rust
use dpp::DmgPipeline;

// Open DMG and auto-detect HFS+ or APFS
let mut pipeline = DmgPipeline::open("image.dmg")?;
let mut fs = pipeline.open_filesystem()?;

// Check what was detected
println!("Filesystem: {:?}", fs.fs_type());

// List root directory (unified FsDirEntry)
for entry in fs.list_directory("/")? {
    println!("{:?} {:>12} {}", entry.kind, entry.size, entry.name);
}

// Get unified volume info
let info = fs.volume_info();
println!("Files: {}, Dirs: {}", info.file_count, info.directory_count);

// Read a file
let data = fs.read_file("/path/to/file.txt")?;
```

### Browse a DMG (HFS+ specific)

```rust
use dpp::DmgPipeline;

// Open DMG and extract HFS+ volume directly
let mut pipeline = DmgPipeline::open("Kernel_Debug_Kit.dmg")?;
let mut hfs = pipeline.open_hfs()?;

// List root directory
for entry in hfs.list_directory("/")? {
    println!("{:?} {:>12} {}", entry.kind, entry.size, entry.name);
}

// Read a file
let data = hfs.read_file("/Library/Developer/KDKs/readme.txt")?;
```

### Browse a DMG (APFS specific)

```rust
use dpp::DmgPipeline;

// Open DMG and extract APFS volume directly
let mut pipeline = DmgPipeline::open("app.dmg")?;
let mut apfs = pipeline.open_apfs()?;

// Volume info
let vi = apfs.volume_info();
println!("Volume: {} ({} files)", vi.name, vi.num_files);

// List root directory
for entry in apfs.list_directory("/")? {
    println!("{:?} {:>12} {}", entry.kind, entry.size, entry.name);
}
```

### Extract a PKG Payload

```rust
// Open a .pkg found on the HFS+ volume
let mut pkg = hfs.open_pkg("/path/to/installer.pkg")?;

// List components
for component in pkg.components() {
    println!("Component: {}", component);
}

// Extract payload (PBZX archive)
let payload = pkg.payload("com.apple.pkg.KDK")?;
let archive = pbzx::Archive::from_reader(std::io::Cursor::new(payload))?;

// List files in the payload
for entry in archive.list()? {
    println!("{}: {} bytes", entry.path, entry.size);
}
```

### One-Call Extraction

```rust
// Find all .pkg files in a DMG
let packages = dpp::find_packages("image.dmg")?;
for pkg_path in &packages {
    println!("Found: {}", pkg_path);
}

// Extract a specific component payload in one call
let archive = dpp::extract_pkg_payload(
    "image.dmg",
    "/path/to/installer.pkg",
    "com.apple.pkg.KDK",
)?;

for entry in archive.list()? {
    println!("{}", entry.path);
}
```

### Low-Memory Streaming

```rust
// Use TempFile mode for large DMGs (default)
let mut hfs = pipeline.open_hfs()?;

// Stream a PKG through a temp file instead of buffering
let mut pkg = hfs.open_pkg_streaming("/large-installer.pkg")?;

// Stream payload to disk
let mut out = std::fs::File::create("payload.pbzx")?;
pkg.payload_to("component", &mut out)?;
```

## Documentation

| | |
|---|---|
| [CLI Tool](docs/CLI.md) | `dpp-tool` usage for exploring DMGs interactively |
| [Architecture](docs/ARCHITECTURE.md) | Pipeline design and type-erasure patterns |

## Example Output

```
$ dpp-tool info Kernel_Debug_Kit.dmg

  DMG Pipeline: Kernel_Debug_Kit.dmg
  ════════════════════════════════════════════════════════════

  DMG (UDIF)
  ────────────────────────────────────────────────────────────
  Version                  4
  Partitions               7
  Compressed               975.87 MB (975.87 MB)
  Uncompressed             1.12 GB
  Space savings            15.0%
  Block types              LZFSE: 988

    ID       Sectors          Size    Ratio  Name
    ----------------------------------------------------------
    -1             1         512 B     89.6%  MBR : 0
     2       2089050    1020.04 MB      4.3%  Apple_HFSX : 3

  HFS+ Volume
  ────────────────────────────────────────────────────────────
  Signature                HFSX (case-sensitive)
  Version                  5
  Block size               4096 bytes
  Files                    3,847
  Folders                  612

  Packages
  ────────────────────────────────────────────────────────────
  └── /Library/Developer/KDKs/KDK_14.2_23C64.pkg  512.30 MB
```

```
$ dpp-tool hfs tree Kernel_Debug_Kit.dmg /Library/Developer/KDKs

  Tree: Kernel_Debug_Kit.dmg:/Library/Developer/KDKs
  ════════════════════════════════════════════════════════════

  ├── KDK_14.2_23C64.pkg  512.30 MB
  ├── readme.txt  1.22 KB
  └── LICENSE.txt  4.78 KB
```

```
$ dpp-tool payload ls Kernel_Debug_Kit.dmg /path.pkg com.apple.pkg.KDK /usr

  Payload: com.apple.pkg.KDK — /usr
  ════════════════════════════════════════════════════════════

  Kind          Size  Name
  --------------------------------------------------------
  dir              -  lib
  dir              -  share
              12.19 KB  libfoo.dylib

  1 file(s), 2 directory(ies)
```

## Alternatives

There is no equivalent single-crate solution in Rust. The closest approach is to manually combine separate tools:

| Approach | DMG | HFS+ | APFS | PKG | PBZX | Single API | Cross-platform |
|----------|:---:|:----:|:----:|:---:|:----:|:----------:|:--------------:|
| **dpp** | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| `hdiutil` + shell | ✓ | ✓ | ✓ | ✓ | ✓ | ❌ | macOS only |
| dmgwiz + hfsfuse + xar | ✓ | ✓ | ❌ | ✓ | ❌ | ❌ | Unix only |
| apple-platform-rs | partial | ❌ | ❌ | partial | ❌ | ❌ | ✓ |

**Choose dpp if you need:**
- End-to-end DMG → files extraction on any platform
- Single Rust dependency for the entire pipeline
- Memory-efficient streaming for large disk images
- Programmatic access to macOS installer contents

## Next Steps

- [ ] **Encrypted DMG** — support FileVault-encrypted disk images
- [ ] **Parallel extraction** — decompress multiple partitions concurrently
- [ ] **Progress callbacks** — report extraction progress for UI integration
- [ ] **Selective extraction** — extract specific files without full pipeline

## License

MIT
