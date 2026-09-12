#!/usr/bin/env python3
"""Mint decmpfs test fixtures from a real macOS filesystem.

Run on macOS, on an APFS or HFS+ volume. Writes one set of files per case
into the output directory, plus a manifest the Rust fixture tests read.

    python3 cmpfs/tools/mint-fixtures.py tests/decmpfs

Why this exists: the compression types above 1 are not documented by Apple, so
`cmpfs` decodes them from two third-party readers. These fixtures are the only
way to check that decoding against bytes Apple actually wrote.

Why it needs a script rather than `xattr(1)`: the kernel hides both
`com.apple.decmpfs` and the resource fork of a compressed file, so `xattr -l`
shows nothing and `cat f/..namedfork/rsrc` is empty. `decmpfs_hides_xattr`
(xnu `bsd/kern/decmpfs.c`) returns 1 for our attribute and, per
`decmpfs_hides_rsrc`, for the resource fork of every compressed file. HFS+
turns that into `ENOATTR` unless `getxattr` is passed `XATTR_SHOWCOMPRESSION`
(`core/hfs_xattr.c`), which `xattr(1)` does not do. That flag is the whole
reason for the ctypes call below.
"""

import ctypes
import ctypes.util
import hashlib
import os
import platform
import subprocess
import sys
import tempfile

# bsd/sys/xattr.h
XATTR_SHOWCOMPRESSION = 0x0020
DECMPFS_XATTR = "com.apple.decmpfs"
RSRC_XATTR = "com.apple.ResourceFork"

BLOCK = 0x10000

_libc = ctypes.CDLL(ctypes.util.find_library("c"), use_errno=True)
_libc.getxattr.argtypes = [
    ctypes.c_char_p,  # path
    ctypes.c_char_p,  # name
    ctypes.c_void_p,  # value
    ctypes.c_size_t,  # size
    ctypes.c_uint32,  # position
    ctypes.c_int,  # options
]
_libc.getxattr.restype = ctypes.c_ssize_t


def getxattr(path, name):
    """Read an extended attribute, including ones the kernel hides.

    Returns None when the file has no such attribute.
    """
    p, n = path.encode(), name.encode()
    size = _libc.getxattr(p, n, None, 0, 0, XATTR_SHOWCOMPRESSION)
    if size < 0:
        return None

    # The resource fork can exceed one read; `position` pages through it.
    out = bytearray()
    while len(out) < size:
        chunk = ctypes.create_string_buffer(size - len(out))
        got = _libc.getxattr(
            p, n, chunk, len(chunk), len(out), XATTR_SHOWCOMPRESSION
        )
        if got <= 0:
            break
        out += chunk.raw[:got]
    return bytes(out)


def compress(src, dst):
    """Copy src to dst asking for filesystem compression.

    `ditto --hfsCompression` compresses "if appropriate" — it declines on data
    that would not shrink — so the caller must check whether it actually did.
    """
    subprocess.run(
        ["ditto", "--hfsCompression", src, dst],
        check=True,
        capture_output=True,
    )


def compressible(size):
    """Text that compresses hard, without being all zeros — an all-zero file
    can take a sparse path instead and never get a decmpfs attribute."""
    unit = b"the quick brown fox jumps over the lazy dog 0123456789\n"
    return (unit * (size // len(unit) + 1))[:size]


def incompressible(size):
    """Deterministic high-entropy bytes. Not `os.urandom`, so a rerun
    produces byte-identical fixtures."""
    out = bytearray()
    h = hashlib.sha256(b"cmpfs")
    while len(out) < size:
        h = hashlib.sha256(h.digest())
        out += h.digest()
    return bytes(out[:size])


def mixed(*parts):
    return b"".join(parts)


# (name, contents, what it is meant to exercise)
CASES = [
    # Inline: under MAX_DECMPFS_XATTR_SIZE (3802), so the payload fits in the
    # attribute and no resource fork is written.
    ("inline-small", compressible(2048), "inline compressed payload"),
    ("inline-tiny", compressible(64), "smallest inline payload"),
    (
        "inline-incompressible",
        incompressible(512),
        "inline stored payload, exercising the marker byte",
    ),
    # Resource fork: over one block, so the block table is genuinely walked.
    (
        "rsrc-three-blocks",
        compressible(3 * BLOCK + 1234),
        "multi-block fork with a short final block",
    ),
    (
        "rsrc-exact-block",
        compressible(BLOCK),
        "exactly one block: block-count boundary",
    ),
    (
        "rsrc-block-plus-one",
        compressible(BLOCK + 1),
        "one byte over a block: forces a second, 1-byte block",
    ),
    (
        "rsrc-mixed-blocks",
        mixed(compressible(BLOCK), incompressible(BLOCK), compressible(4096)),
        "compressed and stored blocks in one fork",
    ),
    (
        "rsrc-all-incompressible",
        incompressible(2 * BLOCK),
        "every block stored",
    ),
    # Degenerate sizes.
    ("empty", b"", "zero-length file"),
    ("one-byte", b"x", "single byte"),
]


def main():
    if platform.system() != "Darwin":
        sys.exit("must run on macOS: the fixtures are whatever this kernel writes")

    if len(sys.argv) != 2:
        sys.exit(f"usage: {sys.argv[0]} <output-dir>")
    outdir = os.path.abspath(sys.argv[1])
    os.makedirs(outdir, exist_ok=True)

    rows = []
    skipped = []

    with tempfile.TemporaryDirectory() as work:
        for name, content, purpose in CASES:
            src = os.path.join(work, f"{name}.src")
            dst = os.path.join(work, f"{name}.dst")
            with open(src, "wb") as f:
                f.write(content)
            compress(src, dst)

            attr = getxattr(dst, DECMPFS_XATTR)
            if attr is None:
                # Not a failure: "ditto declined" is itself a finding, and the
                # uncompressed case is already covered by the unit tests.
                skipped.append((name, purpose))
                continue

            rsrc = getxattr(dst, RSRC_XATTR)
            ctype = int.from_bytes(attr[4:8], "little")
            declared = int.from_bytes(attr[8:16], "little")

            write(outdir, f"{name}.decmpfs", attr)
            write(outdir, f"{name}.expected", content)
            if rsrc:
                write(outdir, f"{name}.rsrc", rsrc)

            rows.append(
                (
                    name,
                    ctype,
                    declared,
                    len(content),
                    len(attr),
                    len(rsrc) if rsrc else 0,
                    hashlib.sha256(content).hexdigest(),
                    purpose,
                )
            )
            print(
                f"  {name:24} type {ctype:<3} "
                f"{len(content):>9} bytes -> attr {len(attr):>5}"
                f" rsrc {len(rsrc) if rsrc else 0:>7}"
            )

    manifest = os.path.join(outdir, "manifest.tsv")
    with open(manifest, "w") as f:
        f.write(f"# macOS {platform.mac_ver()[0]} {platform.machine()}\n")
        f.write(
            "# name\ttype\tdeclared_size\tactual_size\tattr_len"
            "\trsrc_len\tsha256\tpurpose\n"
        )
        for row in rows:
            f.write("\t".join(str(c) for c in row) + "\n")

    print(f"\n{len(rows)} fixtures written to {outdir}")
    if skipped:
        print("not compressed by ditto (expected for some, report these back):")
        for name, purpose in skipped:
            print(f"  {name:24} {purpose}")

    types = sorted({r[1] for r in rows})
    print(f"compression types this system produced: {types}")


def write(outdir, name, data):
    with open(os.path.join(outdir, name), "wb") as f:
        f.write(data)


if __name__ == "__main__":
    main()
