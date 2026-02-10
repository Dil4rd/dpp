<div align="center">

# pbzx

**A fast Rust library for Apple's PBZX archive format**

![Version](https://img.shields.io/badge/version-0.1.0-green)
[![Crates.io](https://img.shields.io/crates/v/pbzx.svg)](https://crates.io/crates/pbzx)
[![Documentation](https://docs.rs/pbzx/badge.svg)](https://docs.rs/pbzx)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
![Platform](https://img.shields.io/badge/platform-windows%20%7C%20linux%20%7C%20macos-lightgrey)

Parse, extract, and create PBZX archives used in macOS software updates and `.pkg` installers.

**The only native Rust PBZX implementation** — 1,500x faster file listing than `cpio-archive`.

</div>

---

## Why pbzx?

**pbzx is the only native Rust library for Apple's PBZX streaming archive format.**

No other Rust crate handles the full PBZX pipeline: XZ decompression → CPIO parsing → file extraction. Existing alternatives are C tools, Python scripts, or require manual shell pipelines.

| Feature | **pbzx** | pbzx (C) | cpio-archive | groob/mackit |
|---------|:-------:|:--------:|:------------:|:------------:|
| Language | Rust | C | Rust | Go |
| Read PBZX | ✓ | ✓ | ❌ | ✓ |
| **Write PBZX** | ✓ | ❌ | ❌ | ❌ |
| Parse CPIO | ✓ | ❌ | ✓ | ❌ |
| Build CPIO | ✓ | ❌ | ✓ | ❌ |
| All CPIO formats | ✓ | ❌ | partial | ❌ |
| Streaming | ✓ | ✓ | ❌ | ✓ |
| Memory safe | ✓ | ❌ | ✓ | ✓ |

> **Example:** Listing 7,788 files in a 1 GB PBZX payload takes **4.7ms** with pbzx versus
> **7.36 seconds** with `cpio-archive` — a **1,578x speedup** from seek-based header parsing.

## Features

| | |
|---|---|
| **List files** | Parse PBZX, decompress XZ chunks, parse CPIO, list entries |
| **Extract files** | Extract individual files or entire archive to disk |
| **Pack files** | Create new PBZX archives from directories or data |

### Format Support

| Format | Read | Write | Description |
|--------|:----:|:-----:|-------------|
| PBZX | ✓ | ✓ | Apple's streaming XZ compression |
| CPIO odc (`070707`) | ✓ | ✓ | POSIX.1 portable format |
| CPIO newc (`070701`) | ✓ | ✓ | SVR4 format (no CRC) |
| CPIO crc (`070702`) | ✓ | ❌ | SVR4 format (with CRC) |

## Quick Start

### Read Archive

```rust
use pbzx::Archive;

// Open and list files
let archive = Archive::open("Payload")?;
for entry in archive.list()? {
    println!("{}: {} bytes", entry.path, entry.size);
}

// Extract a single file
let data = archive.extract_file("path/to/file.txt")?;

// Extract all files
archive.extract_all("output_dir")?;
```

### Create Archive

```rust
use pbzx::writer::{CpioBuilder, PbzxWriter};
use std::fs::File;

// Build CPIO content
let mut cpio = CpioBuilder::new();
cpio.add_file("hello.txt", b"Hello, World!", 0o644);
cpio.add_directory("subdir", 0o755);
let cpio_data = cpio.finish();

// Write PBZX archive
let file = File::create("output.pbzx")?;
let mut writer = PbzxWriter::new(file);
writer.write_cpio(&cpio_data)?;
writer.finish()?;
```

## Documentation

| | |
|---|---|
| [Format Specifications](docs/FORMATS.md) | PBZX and CPIO binary format details |
| [Benchmarks](docs/BENCHMARKS.md) | Performance comparisons and metrics |
| [CLI Tool](docs/CLI.md) | Command-line tool usage |

## Example Output

```
$ pbzx-tool info Payload

PBZX Archive: Payload
════════════════════════════════════════════════════════

  Chunks:             64
  Compressed:         1,012 MB
  Decompressed:       4,273 MB
  Compression ratio:  23.7%

  CPIO entries:       7,788
  Files:              6,241
  Directories:        1,534
  Symlinks:           13
```

```
$ pbzx-tool list Payload

  Mode       Size  Path
──────────────────────────────────────────────────────
  drwxr-xr-x       0  ./usr
  drwxr-xr-x       0  ./usr/lib
  -rwxr-xr-x  12,480  ./usr/lib/libfoo.dylib
  -rw-r--r--   3,201  ./usr/share/man/man1/foo.1
  lrwxr-xr-x      18  ./usr/lib/libfoo.1.dylib -> libfoo.dylib
```

## Benchmarks

All benchmarks on a 1 GB PBZX file (4.3 GB decompressed, 7,788 entries):

| Operation | pbzx | cpio-archive | Speedup |
|-----------|------|--------------|---------|
| **List files** | 4.7ms | 7.36s | **1,578x** |
| **Build CPIO** | 1.08ms | 1.13ms | 1.05x |

| Metric | Value |
|--------|-------|
| XZ decompression throughput | 48.5 MB/s |
| Compression ratio (level 6) | 8.7% |

See [full benchmarks](docs/BENCHMARKS.md) for details.

## Alternatives

| Tool | Language | Read | Write | CPIO | Streaming | Platform |
|------|----------|:----:|:-----:|:----:|:---------:|----------|
| **pbzx** | Rust | ✓ | ✓ | ✓ | ✓ | All |
| [pbzx (C)](https://github.com/NiklasRosenstein/pbzx) | C | ✓ | ❌ | ❌ | ✓ | Unix |
| [groob/mackit](https://github.com/groob/mackit) | Go | ✓ | ❌ | ❌ | ✓ | All |
| [cpio-archive](https://crates.io/crates/cpio-archive) | Rust | ❌ | ❌ | ✓ | ❌ | All |
| [cpio](https://crates.io/crates/cpio) | Rust | ❌ | ❌ | partial | ❌ | All |

**Choose pbzx if you need:**
- Native Rust PBZX parsing (no C FFI)
- PBZX archive **creation** (no other tool supports this)
- Fast CPIO listing via seek-based parsing (1,500x faster than alternatives)
- Integration with the `dpp` pipeline (DMG → HFS+ → PKG → **PBZX**)

**Choose pbzx (C) if you need:**
- Battle-tested C implementation for shell pipelines
- Integration with existing C/C++ toolchains

## Next Steps

- [ ] **Parallel XZ decompression** — decompress chunks across multiple threads
- [ ] **CPIO crc writing** — emit SVR4 CRC format (`070702`)
- [ ] **Streaming CPIO parsing** — parse entries without full decompression
- [ ] **Apple Archive (AAR)** — support newer macOS 11+ payload format
- [ ] **Progress callbacks** — report decompression progress for UI integration
- [ ] **Configurable XZ compression level** — expose level parameter in PbzxWriter
- [ ] **Hard link deduplication** — detect and coalesce identical CPIO entries

## License

MIT
