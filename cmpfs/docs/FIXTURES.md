# decmpfs fixtures

`tests/decmpfs/` holds payloads captured from a real macOS filesystem, decoded
by the ignored tests in `tests/real_fixtures.rs`:

```bash
cargo test -p cmpfs -- --ignored
```

They are the only coverage that checks this crate against bytes Apple wrote,
rather than bytes reconstructed from a reference implementation. `tests/` is
gitignored, so they are generated locally and never committed.

## Layout

Per case, alongside a `manifest.tsv`:

| File | Contents |
|---|---|
| `<name>.decmpfs` | the `com.apple.decmpfs` attribute, verbatim |
| `<name>.rsrc` | the resource fork, verbatim, for the types that use one |
| `<name>.expected` | the original uncompressed bytes |

`manifest.tsv` is tab-separated — name, compression type, declared size, actual
size, attribute length, resource-fork length, sha256, description — so reading
it costs `cmpfs` no dev-dependency. Lines beginning `#` are comments; the first
records the macOS version and architecture the capture came from.

## How they are captured

Have macOS compress a file, then read back the two attributes it hides:

1. `ditto --hfsCompression src dst` compresses on copy, on an APFS or HFS+
   destination. It declines on data that would not shrink, so the caller has to
   check whether it actually did rather than assume.
2. Read `com.apple.decmpfs` and `com.apple.ResourceFork` from the result.

Step 2 is the awkward one. Both attributes are hidden from ordinary reads —
`xattr -l` on a compressed file prints nothing — so `getxattr` must be passed
`XATTR_SHOWCOMPRESSION` (`0x0020`, `bsd/sys/xattr.h`). See
[`FORMATS.md`](FORMATS.md) for why the kernel hides them.

No tool ships that flag, so this needs a few lines of code against libSystem:

```c
ssize_t getxattr(const char *path, const char *name, void *value,
                 size_t size, u_int32_t position, int options);
```

Note `position`: a resource fork can exceed one read, so page through it rather
than assuming a single call returns everything.

A Mac without developer tools has no usable Python — `/usr/bin/python3` is an
Xcode stub, and `xattr(1)` is itself a Python script — but `/usr/bin/ruby` is
real, and its stdlib `Fiddle` binds libSystem at runtime with no compiler.

Content should be deterministic, so a regeneration produces identical fixtures,
and compressible without being all zeros: an all-zero file can take a sparse
path and never get a decmpfs attribute at all.

## What a capture covers, and what it cannot

Measured on **macOS 26.3, arm64, Sep 2026**:

- `ditto` emitted **type 8 only** — LZVN in the resource fork.
- It declined below roughly 32 KiB. A 16 KiB file stayed uncompressed; 32 KiB
  compressed.
- Even at 32 KiB, well under one 64 KiB block, the payload went to the
  **resource fork**, not inline. So `ditto` produces no inline type on this
  release.
- A scan of 15,585 files under `/usr/share` and `/usr/lib/swift` found 13,259
  compressed, **every one of them type 8** — including a 747-byte `Info.plist`.
  So this is not a `ditto` quirk; it is what the OS writes.

Five fixtures come out of that, all type 8: a short single block, an exact
block, one byte over a block, three blocks with a short final one, and a fork
mixing compressed and stored blocks. Between them they cover the block-count
boundary, the offset table, and both branches of the per-block marker — the
last confirmed by inspection rather than assumed:

```
offset table: [16, 569, 66106, 66205]
  block 0:    553 bytes -> LZVN compressed
  block 1:  65537 bytes -> stored (0x06 marker)
  block 2:     99 bytes -> LZVN compressed
```

**Types 3, 4, 7, 9, 10, 11 and 12 cannot be captured this way**, because
nothing on a current macOS writes them. They remain covered only by the unit
tests in `src/`, which build their payloads from the reference implementations.
Reaching them needs a third-party encoder such as `afsctool`, which in turn
needs developer tools and a package manager; note that its output would be a
second independent writer rather than Apple's own.

**LZBITMAP, types 13 and 14, was not produced at all.** That is the measurement
behind the capability gap recorded in `.claude/docs/TODO.md`: no shipped code
path fails on ordinary input because of it.
