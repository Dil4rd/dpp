# Benchmarks

Performance benchmarks for the pbzx library.

All benchmarks run on a 1GB PBZX file (4.3GB decompressed, 7,788 entries).

## Comparison with `cpio-archive` crate

The `cpio-archive` crate only handles CPIO archives—it doesn't support PBZX or XZ compression. So we can only compare the **CPIO layer** operations:

```
PBZX file → [XZ decompress] → CPIO archive → [CPIO parse] → files
             ^^^^^^^^^^^^^^    ^^^^^^^^^^^
             pbzx only         both libraries
```

| Operation | pbzx | cpio-archive | Speedup |
|-----------|------|--------------|---------|
| **List files** | 4.7ms | 7.36s | **1,578x** |
| **Build CPIO** | 1.08ms | 1.13ms | 1.05x |

### Why is listing 1,500x faster?

Our implementation parses CPIO headers and **skips over file data** using seek operations. The `cpio-archive` crate reads all file data into memory even when you only need metadata.

## PBZX Decompression (XZ)

| Metric | Value |
|--------|-------|
| Input | 1,012 MB |
| Output | 4,273 MB |
| Ratio | 23.7% |
| Throughput | 48.5 MB/s |

## PBZX Compression (XZ)

Compression benchmarks on a 16MB sample:

| Level | Output Size | Ratio | Throughput |
|-------|-------------|-------|------------|
| 0 | 3.6 MB | 21.4% | 15.8 MB/s |
| 3 | 1.6 MB | 9.5% | 15.2 MB/s |
| 6 | 1.5 MB | 8.7% | 2.6 MB/s |
| 9 | 1.5 MB | 8.7% | 2.5 MB/s |

## Run Benchmarks

To run the comparison benchmark yourself:

```bash
cargo run -p pbzx --example cpio-compare --release -- Payload
```

This requires a PBZX file (e.g., `Payload` from a macOS `.pkg`).
