<div align="center">

# cmpfs

**Decoder for Apple `decmpfs` transparent filesystem compression**

[![Crates.io](https://img.shields.io/crates/v/cmpfs.svg)](https://crates.io/crates/cmpfs)
[![Documentation](https://docs.rs/cmpfs/badge.svg)](https://docs.rs/cmpfs)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

**Pure Rust, zero unsafe** — works everywhere Rust compiles.

</div>

---

## Why cmpfs?

Most files on a macOS system volume are transparently compressed. Their data
fork is empty; the bytes live in the `com.apple.decmpfs` extended attribute or
in the resource fork. A reader that walks only the data fork returns **nothing**
for such a file while `stat` still reports its full size.

`cmpfs` turns those bytes back into file contents. It is filesystem agnostic —
[`hfsplus`](../hfsplus) and [`apfs`](../apfs) supply the attribute and the fork,
and use this crate to decode them.

The name is the on-disk magic, `cmpf`. The `decmpfs` crate name on crates.io
belongs to an unrelated tool that *applies* compression to a live filesystem.

## Usage

```rust
let header = cmpfs::Header::parse(&decmpfs_attribute)?;

let resource_fork = match header.storage() {
    cmpfs::Storage::ResourceFork => Some(read_resource_fork()?),
    _ => None,
};

let contents = cmpfs::decompress(&decmpfs_attribute, resource_fork.as_deref())?;
```

## Supported compression types

| Type | Codec | Payload |
|:----:|-------|---------|
| 1 | stored | attribute |
| 3 | zlib or stored | attribute |
| 4 | zlib or stored | resource fork |
| 7 | LZVN or stored | attribute |
| 8 | LZVN or stored | resource fork |
| 9 | stored | attribute |
| 10 | stored | resource fork |
| 11 | LZFSE | attribute |
| 12 | LZFSE | resource fork |

LZBITMAP (types 13 and 14) has no Rust decoder and reports
`CmpfsError::Unsupported`. Dataless placeholders (`0x80000001`, `0x80000002`)
report `CmpfsError::Dataless` — their contents are not on the volume at all.

## Telling machinery from user data

macOS hides `com.apple.decmpfs` and a compressed file's resource fork from
userspace, so a compressed file looks exactly like an ordinary one. A reader
that works on a raw volume has no such filter, and a caller that copies every
attribute onto an extracted file produces a file macOS reads as corrupt — the
attribute declares a data fork that is no longer empty.

`classify_xattr` draws the line without hiding anything:

```rust
use cmpfs::{classify_xattr, XattrKind};

let compressed = true;
assert_eq!(classify_xattr("com.apple.decmpfs", compressed), XattrKind::Compression);
assert_eq!(classify_xattr("com.apple.quarantine", compressed), XattrKind::User);

// A resource fork is user data on an uncompressed file — an icon, a classic
// resource map — so the name alone cannot decide.
assert_eq!(classify_xattr("com.apple.ResourceFork", true), XattrKind::Compression);
assert_eq!(classify_xattr("com.apple.ResourceFork", false), XattrKind::User);
```

`hfsplus` and `apfs` apply this in `list_xattrs`, which reports every attribute
and tags each one.

## Correctness

Only type 1 is documented by Apple, in `bsd/sys/decmpfs.h`. Everything else
lives in the closed AppleFSCompression project, so the layouts here are taken
from two independent readers — `libarchive` and `apfs-fuse` — and the cases
only one of them covers are marked provisional in the source. See
[`docs/FORMATS.md`](docs/FORMATS.md).

A payload that does not produce exactly the size its header declares is an
error, not a short buffer.

To check the decoder against bytes Apple actually wrote rather than bytes
reconstructed from a reference, mint fixtures on a Mac and run the ignored
tests:

```bash
python3 cmpfs/tools/mint-fixtures.py tests/decmpfs   # on macOS
cargo test -p cmpfs -- --ignored                     # anywhere
```

The script needs `getxattr` with `XATTR_SHOWCOMPRESSION`, because the kernel
hides both `com.apple.decmpfs` and a compressed file's resource fork from
ordinary reads — `xattr -l` on a compressed file prints nothing. See the script
header for the deciding lines in xnu.

## License

MIT
