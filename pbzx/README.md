<div align="center">

# pbzx

**A fast Rust library for Apple's PBZX archive format**

![Version](https://img.shields.io/badge/version-0.1.0-green)
[![Crates.io](https://img.shields.io/crates/v/pbzx.svg)](https://crates.io/crates/pbzx)
[![Documentation](https://docs.rs/pbzx/badge.svg)](https://docs.rs/pbzx)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

Parse, extract, and create PBZX archives used in macOS software updates and `.pkg` installers.

</div>

---

## Features

| | |
|---|---|
| **List files** | Parse PBZX, decompress XZ chunks, parse CPIO, list entries |
| **Extract files** | Extract individual files or entire archive to disk |
| **Pack files** | Create new PBZX archives from directories or data |

## Quick Start

### Read Archive

```rust
use pbzx::Archive;

// Open and list files
let archive = Archive::open("Yourfile")?;
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

## License

MIT
