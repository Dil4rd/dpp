# dpp

**Cross-platform Apple DMG extraction pipeline in pure Rust.**

Open macOS `.dmg` disk images on any OS — no macOS required. Navigate the full stack from DMG container down to individual files:

```
DMG (UDIF) → HFS+ filesystem → PKG installer (XAR) → Payload (PBZX/CPIO) → files
```

## Features

- **Full pipeline** — single API call goes from `.dmg` file to extracted files
- **Cross-platform** — works on Linux, macOS, and Windows
- **All Apple compression formats** — LZFSE, LZVN, Zlib, Bzip2
- **Memory efficient** — streams through temp files by default (~4KB resident memory)
- **Pure Rust** — zero `unsafe` in filesystem crates, minimal dependencies
- **Modular** — use the full pipeline or individual crates standalone
- **Parallel decompression** — opt-in multi-threaded PBZX extraction via `parallel` feature

## Quick Start

### CLI Tool

```bash
cargo install dpp-tool
```

```bash
# Overview of everything inside a DMG
dpp-tool info Kernel_Debug_Kit.dmg

# Browse the HFS+ filesystem
dpp-tool hfs tree Kernel_Debug_Kit.dmg /
dpp-tool hfs ls Kernel_Debug_Kit.dmg /Library/Developer

# Find all .kext bundles
dpp-tool hfs find Kernel_Debug_Kit.dmg -name "*.kext" -type d

# Inspect a PKG installer inside the DMG
dpp-tool pkg ls Kernel_Debug_Kit.dmg /KernelDebugKit.pkg

# List files inside a PKG payload
dpp-tool payload ls Kernel_Debug_Kit.dmg /KernelDebugKit.pkg com.apple.pkg.KDK /

# Extract a file to stdout
dpp-tool payload cat Kernel_Debug_Kit.dmg /KernelDebugKit.pkg com.apple.pkg.KDK /usr/bin/some_tool > tool
```

### Library

Add to `Cargo.toml`:

```toml
[dependencies]
dpp = "0.1"
```

For faster PBZX payload extraction with multi-threaded decompression:

```toml
[dependencies]
dpp = { version = "0.1", features = ["parallel"] }
```

Open a DMG and list the root directory:

```rust
let mut pipeline = dpp::DmgPipeline::open("image.dmg")?;
let mut hfs = pipeline.open_hfs()?;

for entry in hfs.list_directory("/")? {
    println!("{}", entry.name);
}
```

Find and extract packages:

```rust
// Walk the filesystem and find all .pkg files
let packages = dpp::find_packages("image.dmg")?;

// Extract a payload in one call
let archive = dpp::extract_pkg_payload("image.dmg", "/Some.pkg", "com.example.pkg")?;
for entry in archive.list()? {
    println!("{} ({} bytes)", entry.path(), entry.size());
}
```

Stream a file out with low memory usage:

```rust
let mut pipeline = dpp::DmgPipeline::open("image.dmg")?;
let mut hfs = pipeline.open_hfs()?;

let mut out = std::fs::File::create("kernel")?;
hfs.read_file_to("/System/Library/Kernels/kernel", &mut out)?;
```

## CLI Commands

| Command | Description |
|---------|-------------|
| `dpp-tool info <dmg>` | Full pipeline overview |
| `dpp-tool bench <dmg>` | Benchmark each pipeline stage |
| **dmg** | |
| `dpp-tool dmg info <dmg>` | DMG format and compression stats |
| `dpp-tool dmg ls <dmg>` | List partitions |
| `dpp-tool dmg cat <dmg> [id]` | Extract raw partition data |
| **hfs** | |
| `dpp-tool hfs info <dmg>` | HFS+ volume header |
| `dpp-tool hfs ls <dmg> <path>` | List directory |
| `dpp-tool hfs tree <dmg> [path]` | Browse filesystem tree |
| `dpp-tool hfs cat <dmg> <path>` | Extract file to stdout |
| `dpp-tool hfs stat <dmg> <path>` | File metadata |
| `dpp-tool hfs find <dmg> [opts]` | Find files by name/type |
| **pkg** | |
| `dpp-tool pkg info <dmg> <pkg>` | Package statistics |
| `dpp-tool pkg ls <dmg> <pkg>` | List XAR contents |
| `dpp-tool pkg cat <dmg> <pkg> <file>` | Extract XAR entry |
| **payload** | |
| `dpp-tool payload info <dmg> <pkg> <comp>` | Payload stats |
| `dpp-tool payload ls <dmg> <pkg> <comp> [path]` | List payload files |
| `dpp-tool payload tree <dmg> <pkg> <comp>` | Browse payload tree |
| `dpp-tool payload cat <dmg> <pkg> <comp> <file>` | Extract payload file |

## Architecture

```
dpp-tool  (CLI binary)
    │
   dpp  (pipeline library — orchestrates all crates below)
    │
    ├── udif      DMG / UDIF disk image reader & writer
    ├── hfsplus   HFS+ / HFSX filesystem parser
    ├── xara      XAR archive & macOS PKG parser
    └── pbzx      PBZX streaming archive & CPIO parser

   apfs  (standalone APFS filesystem parser)
```

Each crate is published independently and can be used on its own:

| Crate | Description | Dependencies |
|-------|-------------|:------------:|
| [`udif`](udif/) | Apple DMG (UDIF) disk images — read & write with LZFSE/LZVN/Zlib/Bzip2 | 9 |
| [`hfsplus`](hfsplus/) | HFS+ and HFSX filesystem — B-tree traversal, extent overflow, Unicode | 2 |
| [`xara`](xara/) | XAR archives and macOS PKG installers — TOC parsing, payload extraction | 4 |
| [`pbzx`](pbzx/) | PBZX streaming archives — chunked XZ decompression, CPIO read/write | 3 |
| [`apfs`](apfs/) | APFS containers and volumes — checksums, object maps, catalog B-trees | 2 |
| [`dpp`](dpp/) | Pipeline library chaining all of the above | 6 |

## Building

```bash
cargo build --release
```

Build with multi-threaded PBZX decompression:

```bash
cargo build --release --features parallel
```

Run tests (some tests require fixture files and skip gracefully if missing):

```bash
cargo test
```

## License

MIT
