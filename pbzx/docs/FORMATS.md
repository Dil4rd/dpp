# Format Specifications

This document describes the binary formats supported by the pbzx library.

## Supported Formats

| Format | Magic | Description |
|--------|-------|-------------|
| PBZX | `pbzx` | Apple's streaming XZ compression |
| CPIO odc | `070707` | POSIX.1 portable format |
| CPIO newc | `070701` | SVR4 format (no CRC) |
| CPIO crc | `070702` | SVR4 format (with CRC) |

## PBZX Structure

PBZX is Apple's streaming compression format used in macOS software updates and installer packages (`.pkg` files). The format wraps XZ-compressed chunks of a CPIO archive.

```
+------------------+
| Header (12 bytes)|  Magic "pbzx" + flags (u64 BE)
+------------------+
| Chunk 1          |  uncompressed_size (u64 BE) + compressed_size (u64 BE) + XZ data
+------------------+
| Chunk 2          |
+------------------+
| ...              |
+------------------+
| Chunk N          |
+------------------+
```

The concatenated decompressed chunks form a CPIO archive.

### Header

| Offset | Size | Type | Description |
|--------|------|------|-------------|
| 0 | 4 | ASCII | Magic bytes `pbzx` |
| 4 | 8 | u64 BE | Flags (chunk size hint) |

### Chunk

| Offset | Size | Type | Description |
|--------|------|------|-------------|
| 0 | 8 | u64 BE | Uncompressed size |
| 8 | 8 | u64 BE | Compressed size |
| 16 | varies | bytes | XZ-compressed data |

If `compressed_size == uncompressed_size`, the chunk data is stored uncompressed.

## CPIO odc Format (070707)

The POSIX.1 portable format uses octal ASCII for all numeric fields.

```
+------------------+
| Header (76 bytes)|  Magic + metadata in octal ASCII
+------------------+
| Filename         |  namesize bytes (null-terminated)
+------------------+
| File data        |  filesize bytes
+------------------+
| ... repeat ...   |
+------------------+
| TRAILER!!!       |  End marker
+------------------+
```

### Header Fields

| Offset | Size | Field | Description |
|--------|------|-------|-------------|
| 0 | 6 | magic | `070707` |
| 6 | 6 | dev | Device number |
| 12 | 6 | ino | Inode number |
| 18 | 6 | mode | File mode and type |
| 24 | 6 | uid | User ID |
| 30 | 6 | gid | Group ID |
| 36 | 6 | nlink | Number of links |
| 42 | 6 | rdev | Device number (for special files) |
| 48 | 11 | mtime | Modification time |
| 59 | 6 | namesize | Length of filename (including null) |
| 65 | 11 | filesize | File size in bytes |

## CPIO newc/crc Format (070701/070702)

The SVR4 formats use hexadecimal ASCII and support larger files.

### Header Fields

| Offset | Size | Field | Description |
|--------|------|-------|-------------|
| 0 | 6 | magic | `070701` (newc) or `070702` (crc) |
| 6 | 8 | ino | Inode number |
| 14 | 8 | mode | File mode and type |
| 22 | 8 | uid | User ID |
| 30 | 8 | gid | Group ID |
| 38 | 8 | nlink | Number of links |
| 46 | 8 | mtime | Modification time |
| 54 | 8 | filesize | File size |
| 62 | 8 | devmajor | Major device number |
| 70 | 8 | devminor | Minor device number |
| 78 | 8 | rdevmajor | Major rdev number |
| 86 | 8 | rdevminor | Minor rdev number |
| 94 | 8 | namesize | Length of filename |
| 102 | 8 | check | CRC (crc format only, 0 for newc) |

Header and filename are padded to 4-byte boundaries. File data is also padded to 4-byte boundaries.
