# Benchmarks

Performance benchmarks for the dpp pipeline and its component crates.

All benchmarks run on a macOS Kernel Debug Kit DMG (~1 GB compressed, ~1.15 GB uncompressed).

## Pipeline Stages

### HFS+ DMG

| Stage | Operation | Time | Throughput |
|-------|-----------|------|------------|
| 1. DMG open | Parse koly header + plist + mish blocks | ~235 ms | - |
| 2. HFS+ extraction | LZFSE decompress + write to temp file | ~8.2 s | 124 MB/s |
| 3. Volume open | Parse volume header + B-tree headers | ~1 ms | - |
| 4. Filesystem walk | Traverse catalog B-tree | ~45 ms | 85K entries/s |
| 5. PKG open | Read file + parse XAR header/TOC | ~120 ms | - |
| 6. PBZX decompress | XZ decompress + CPIO parse | ~22 s | 48.5 MB/s |

### APFS DMG

| Stage | Operation | Time | Throughput |
|-------|-----------|------|------------|
| 1. DMG open | Parse koly header + plist + mish blocks | ~235 ms | - |
| 2. APFS extraction | Decompress + write to temp file | ~1–8 s | Varies by compression |
| 3. Volume open | Checkpoint scan + object map + volume superblock | ~2 ms | - |
| 4. Filesystem walk | Traverse catalog B-tree | ~30–60 ms | Varies by file count |

**Bottleneck:** Decompression dominates (stages 2 and 6). LZFSE decompression is ~124 MB/s; XZ decompression is ~48.5 MB/s.

## Comparison with Alternatives

### Full Pipeline (DMG → files)

| Approach | Time | Platform | Automation |
|----------|------|----------|------------|
| **dpp pipeline** | ~31 s | All | Full API |
| macOS `hdiutil` + shell | ~25 s | macOS only | Shell scripting |
| dmgwiz + mount + xar | ~35 s | Linux only | Shell scripting |

### DMG Layer (udif vs alternatives)

Benchmarked on a 1 GB LZFSE-compressed DMG:

| Crate | Parse | Extract | LZFSE | Write |
|-------|-------|---------|:-----:|:-----:|
| **udif** | 235 ms | 8.2 s | ✓ | ✓ |
| dmgwiz | 210 ms | 8.5 s | ✓ | ❌ |
| apple-dmg | 180 ms | N/A (no LZFSE) | ❌ | ❌ |
| dmg-oxide | 190 ms | N/A (no LZFSE) | ❌ | ✓ |

### HFS+ Layer (hfsplus vs alternatives)

Benchmarked on a 1 GB HFSX partition with 3,847 files:

| Implementation | Volume open | Walk all files | Read file |
|----------------|-------------|----------------|-----------|
| **hfsplus** | 1.1 ms | 45 ms | 0.3 ms |
| hfsplus-rs | 2.3 ms | N/A | 0.5 ms |
| hfsfuse (C) | 0.8 ms | 38 ms | 0.2 ms |

### PBZX Layer (pbzx vs alternatives)

Benchmarked on a 1 GB PBZX payload (4.3 GB decompressed, 7,788 entries):

| Tool | List files | Decompress | Language |
|------|-----------|------------|----------|
| **pbzx** | 4.7 ms | 22.1 s | Rust |
| cpio-archive | 7.36 s | N/A | Rust |
| pbzx (C) | N/A | 20.8 s | C |

## Running Benchmarks

### Pipeline benchmark

```bash
cargo run -p dpp-tool --release -- bench Kernel_Debug_Kit.dmg
```

### PBZX comparison benchmark

```bash
cargo run --example cpio-compare --release -p pbzx -- Payload
```

## Memory Usage

| Mode | Peak memory | Notes |
|------|-------------|-------|
| TempFile (default) | ~50 MB | Streams through 64 KB buffers |
| InMemory | ~1.2 GB | Buffers entire partition in RAM |

The TempFile mode is recommended for production use — it uses temporary files for intermediate data and keeps memory usage low regardless of DMG size.
