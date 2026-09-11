# decmpfs on-disk format

## Sources

Apple documents only the attribute header and compression type 1, in
`bsd/sys/decmpfs.h` (xnu):

```c
#define DECMPFS_MAGIC 0x636d7066 /* cmpf */
#define DECMPFS_XATTR_NAME "com.apple.decmpfs"

typedef struct __attribute__((packed)) {
    /* this structure represents the xattr on disk; the fields below are little-endian */
    uint32_t compression_magic;
    uint32_t compression_type;
    union { uint64_t uncompressed_size; ... };
    unsigned char attr_bytes[0];
} decmpfs_disk_header;

enum {
    CMP_Type1 = 1, /* uncompressed data in xattr */
    /* additional types defined in AppleFSCompression project */
    CMP_MAX = 255
};
```

Types 3 and above live in the closed AppleFSCompression project. Two
independent readers cover them:

- **libarchive**, `archive_write_disk_posix.c` — zlib only, both directions.
  `hfs_decompress` is a genuine decoder, and its writer
  (`hfs_write_resource_fork_header`) pins the resource-fork layout.
- **apfs-fuse**, `ApfsLib/Decmpfs.cpp` — all types, read only.

Where they disagree or only one covers a case, the source says so.

## Attribute header

16 bytes, little-endian, unlike almost every other Apple structure this
workspace parses.

| Offset | Size | Field |
|-------:|-----:|-------|
| 0 | 4 | magic `0x636d7066` (`fpmc` on disk) |
| 4 | 4 | compression type |
| 8 | 8 | uncompressed size |
| 16 | .. | inline payload, for the attribute-resident types |

## Compression types

| Type | Codec | Payload | Notes |
|:----:|-------|---------|-------|
| 1 | stored | attribute | the only type xnu names; no marker byte |
| 3 | zlib / stored | attribute | |
| 4 | zlib / stored | resource fork | block table, below |
| 7 | LZVN / stored | attribute | |
| 8 | LZVN / stored | resource fork | offset table, below |
| 9 | stored | attribute | marker `0xCC`; apfs-fuse only |
| 10 | stored | resource fork | apfs-fuse only, and marked an assumption there |
| 11 | LZFSE | attribute | |
| 12 | LZFSE | resource fork | apfs-fuse only, and marked an assumption there |
| 13, 14 | LZBITMAP | attribute / fork | no Rust decoder; reported unsupported |
| `0x80000001`, `0x80000002` | — | none | dataless placeholders |

## Stored blocks

A block that would not compress is stored with a one-byte marker.

For zlib, libarchive writes and tests exactly `0xff`; apfs-fuse tests
`(b & 0x0f) == 0x0f`. RFC 1950 fixes the low nibble of a zlib CMF byte at 8, so
the nibble test cannot misread a real zlib stream, and it accepts what both
writers produce. This crate tests the nibble.

For LZVN the marker is `0x06`, the end-of-stream opcode, which no compressed
block can begin with.

## Resource fork, type 4

The fork opens with a resource-fork header: a big-endian offset to the resource
data, which itself starts with a big-endian length. The decmpfs block table
follows.

```
+0     be u32  offset to resource data (Apple writes 0x100)
+0x100 be u32  length of the resource data
+0x104 le u32  block count            <- block table base
+0x108 le u32  block 0 offset, relative to the block table base
       le u32  block 0 length
       ...
```

libarchive reads the table at a hardcoded 260 (`RSRC_H_SIZE`); apfs-fuse
follows the stored offset. Both land in the same place on any fork Apple
writes. This crate follows the stored offset.

Each block decompresses to 64 KiB, except the last.

## Resource fork, types 8, 10 and 12

A different shape: a table of little-endian offsets at the very start of the
fork, each block running to the next offset. The block count is derived from
the uncompressed size, and the table therefore holds `count + 1` entries.

```
+0  le u32  block 0 offset, relative to the start of the fork
    le u32  block 1 offset
    ...
    le u32  end of the last block
```

Blocks in these forks are followed by padding — macOS leaves 80–300 bytes after
each LZVN block's end-of-stream opcode, which the kernel and Apple's
`lzvn_decode_buffer` ignore. A strict whole-stream decoder rejects them, so the
LZVN decoder used here is deliberately length-tolerant.

## Rejected input

A block longer than 64 KiB + 1 cannot be valid: a stored block is one marker
byte plus a full block, and a compressed one is smaller. Both references cap it
there, and so does this crate.

A payload that does not decode to exactly the size the header declares is an
error rather than a short buffer, since `Vec<u8>` cannot express "complete
except for a hole".
