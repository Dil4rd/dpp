# dpp-tool

**Cross-platform CLI for exploring Apple DMG disk images.**

Navigate the full stack from DMG container down to individual files — no macOS required:

```
DMG (UDIF) → HFS+ filesystem → PKG installer (XAR) → Payload (PBZX/CPIO) → files
```

## Install

```bash
cargo install dpp-tool
```

## Quick Start

```bash
# Overview of everything inside a DMG
dpp-tool info Kernel_Debug_Kit.dmg

# Browse the HFS+ filesystem
dpp-tool hfs tree Kernel_Debug_Kit.dmg /

# Inspect a PKG installer inside the DMG
dpp-tool pkg ls Kernel_Debug_Kit.dmg /KernelDebugKit.pkg

# Extract a file from a PKG payload to stdout
dpp-tool payload cat Kernel_Debug_Kit.dmg /KernelDebugKit.pkg com.apple.pkg.KDK /usr/bin/some_tool > tool
```

## Commands

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
| `dpp-tool pkg find <dmg> <pkg> [opts]` | Find XAR entries |
| `dpp-tool pkg cat <dmg> <pkg> <file>` | Extract XAR entry |
| **payload** | |
| `dpp-tool payload info <dmg> <pkg> <comp>` | Payload stats |
| `dpp-tool payload ls <dmg> <pkg> <comp> [path]` | List payload files |
| `dpp-tool payload tree <dmg> <pkg> <comp>` | Browse payload tree |
| `dpp-tool payload find <dmg> <pkg> <comp> [opts]` | Find payload files |
| `dpp-tool payload cat <dmg> <pkg> <comp> <file>` | Extract payload file |

## Examples

### DMG layer

```bash
# Compression format and partition map
dpp-tool dmg info Kernel_Debug_Kit.dmg

# List all partitions in the DMG
dpp-tool dmg ls Kernel_Debug_Kit.dmg

# Dump raw partition data
dpp-tool dmg cat Kernel_Debug_Kit.dmg 0 > partition.bin
```

### HFS+ filesystem

```bash
# Browse the directory tree
dpp-tool hfs tree Kernel_Debug_Kit.dmg /Library

# List a specific directory
dpp-tool hfs ls Kernel_Debug_Kit.dmg /Library/Developer

# Find all .kext bundles
dpp-tool hfs find Kernel_Debug_Kit.dmg -name "*.kext" -type d

# Extract a file
dpp-tool hfs cat Kernel_Debug_Kit.dmg /System/Library/Kernels/kernel > kernel
```

### PKG installer (XAR)

```bash
# List components inside a PKG
dpp-tool pkg ls Kernel_Debug_Kit.dmg /KernelDebugKit.pkg

# Package statistics
dpp-tool pkg info Kernel_Debug_Kit.dmg /KernelDebugKit.pkg
```

### Payload (PBZX/CPIO)

```bash
# List files inside a payload
dpp-tool payload ls Kernel_Debug_Kit.dmg /KernelDebugKit.pkg com.apple.pkg.KDK /

# Browse payload tree
dpp-tool payload tree Kernel_Debug_Kit.dmg /KernelDebugKit.pkg com.apple.pkg.KDK

# Extract a file from the payload
dpp-tool payload cat Kernel_Debug_Kit.dmg /KernelDebugKit.pkg com.apple.pkg.KDK /usr/bin/some_tool > tool
```

## License

MIT
