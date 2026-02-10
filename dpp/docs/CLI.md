# CLI Tool

The `dpp-tool` crate provides a fancy command-line interface for exploring the full Apple DMG pipeline.

## Building

```bash
cargo build -p dpp-tool --release
```

## Command Structure

Commands are organized by pipeline layer with consistent verbs across layers:

| Verb | Meaning |
|------|---------|
| `ls` | List contents |
| `info` | Show metadata |
| `cat` | Output/extract content |

### Overview Commands

```bash
dpp-tool info <dmg>              # Full pipeline overview (DMG + HFS+ + PKGs)
dpp-tool bench <dmg>             # Benchmark each pipeline stage
```

### dmg — DMG/UDIF Container

```bash
dpp-tool dmg info <dmg>              # Format version, compression stats, block types
dpp-tool dmg ls <dmg>                # List partitions
dpp-tool dmg cat <dmg> [partition-id] # Raw partition data to stdout (default: main)
```

### hfs — HFS+ Filesystem

```bash
dpp-tool hfs info <dmg>              # Volume header (signature, blocks, counts)
dpp-tool hfs ls <dmg> <path>         # List directory contents
dpp-tool hfs tree <dmg> [path]       # Browse filesystem tree
dpp-tool hfs cat <dmg> <path>        # File to stdout
dpp-tool hfs stat <dmg> <path>       # File metadata (CNID, perms, dates, forks)
```

### pkg — PKG/XAR Archive

```bash
dpp-tool pkg ls <dmg>                # Find all .pkg files on volume
dpp-tool pkg info <dmg> <pkg-path>   # Package type, XAR contents listing
dpp-tool pkg cat <dmg> <pkg> <file>  # XAR entry to stdout (Distribution, etc.)
```

### component — PKG Component Payloads

```bash
dpp-tool component ls <dmg> <pkg-path>                    # List components
dpp-tool component info <dmg> <pkg-path> <component>      # Component metadata
dpp-tool component cat <dmg> <pkg-path> <component> <dest> # Extract payload to dir
```

## Examples

### Full pipeline overview

```
$ cargo run -p dpp-tool -- info Kernel_Debug_Kit.dmg

  > Opening Kernel_Debug_Kit.dmg... done (234.5ms)

  DMG Pipeline: Kernel_Debug_Kit.dmg
  ════════════════════════════════════════════════════════════

  DMG (UDIF)
  ────────────────────────────────────────────────────────────
  Version                  4
  Partitions               7
  Compressed               975.87 MB
  Uncompressed             1,148.08 MB
  Space savings            15.0%
  Block types              LZFSE: 988

    ID       Sectors          Size    Ratio  Name
    ----------------------------------------------------------
    -1             1         512 B     89.6%  MBR : 0
     2       2089050    1020.04 MB      4.3%  Apple_HFSX : 3

  > Extracting HFS+ partition... done (8.2s)

  HFS+ Volume
  ────────────────────────────────────────────────────────────
  Signature                HFSX (case-sensitive)
  Block size               4096 bytes
  Files                    3,847
  Folders                  612

  > Scanning for .pkg files... done (45.3ms)

  Packages
  ────────────────────────────────────────────────────────────
  └── /Library/Developer/KDKs/KDK_14.2_23C64.pkg  512.3 MB
```

### Browsing the filesystem

```
$ cargo run -p dpp-tool -- hfs tree Kernel_Debug_Kit.dmg /Library

  ├── Developer/
  │   └── KDKs/
  │       ├── KDK_14.2_23C64.pkg          512.3 MB
  │       ├── readme.txt                     1.2 KB
  │       └── LICENSE.txt                    4.8 KB
  └── Receipts/
```

### Inspecting a package

```bash
dpp-tool pkg info Kernel_Debug_Kit.dmg /Library/Developer/KDKs/KDK.pkg
dpp-tool component ls Kernel_Debug_Kit.dmg /Library/Developer/KDKs/KDK.pkg
```

### Extracting a component

```bash
dpp-tool component cat Kernel_Debug_Kit.dmg /path.pkg com.apple.pkg.KDK ./output
```

### Extracting raw data

```bash
# Dump main HFS+ partition to a file
dpp-tool dmg cat Kernel_Debug_Kit.dmg > partition.raw

# Extract a specific XAR entry (e.g., Distribution XML)
dpp-tool pkg cat Kernel_Debug_Kit.dmg /path.pkg Distribution
```

### Benchmarking

```
$ cargo run -p dpp-tool --release -- bench Kernel_Debug_Kit.dmg

  Benchmark: Kernel_Debug_Kit.dmg
  ════════════════════════════════════════════════════════════

  Stage 1: DMG Open (UDIF parse)
  ────────────────────────────────────────────────────────────
  Time                     234.5ms
  Partitions found         7

  Stage 2: HFS+ Extraction (decompress + parse)
  ────────────────────────────────────────────────────────────
  Time                     8.23s
  Throughput               124.3 MB/s

  Stage 3: Filesystem Walk (B-tree traversal)
  ────────────────────────────────────────────────────────────
  Time                     45.3ms
  Files                    3,847
  Entries/sec              84,923

  Pipeline Summary

  Stage                         Time           %
  --------------------------------------------------
  DMG open                       234.5ms    2.7%  ##
  HFS+ extraction                  8.23s   96.4%  ######################################
  Filesystem walk                 45.3ms    0.5%  #
  --------------------------------------------------
  Total                            8.51s
```
