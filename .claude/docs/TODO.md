# Open Work

Findings from the Sep 2026 audit, ordered by what is safe to do next. Reasoning
and criteria live in [Forensic Direction](FORENSIC-DIRECTION.md).

Line numbers were correct at `e1ddf50`. Severity is about what reaches the
caller: **high** = wrong data returned as if it were right; **med** = silent
omission; **low** = cosmetic or unreachable.

Provenance is marked. Items I verified by reading the code or measuring are
`[verified]`; items reported by an audit agent and not independently checked
are `[unverified]` — confirm before acting.

## Done

Tier 1 items 1-4 and 10-12, landed `7dbef09`..`e1ddf50`:

- udif: short decodes rejected across all four codecs; writer pads the final
  chunk; raw remainder uses checked arithmetic
- pbzx: cpio entry data read in the format its header declared; odc filesize
  and mtime widened past 32 bits; zero chunk header with data behind it
  rejected
- apfs: extent block numbers that overflow the address space rejected

## Next: the anomaly channel

**This is the critical path, ahead of the remaining Tier 1 items.** Nearly
everything below is "containable, needs reporting", and tightening those first
just adds more `PROVISIONAL(anomaly-channel)` markers to unwind later.

Model it on `pbzx`'s `ExtractStats`, but carry the affected **byte range**, not
a counter. "bytes 4096-4608 of partition 0 were not recovered" is
investigable; "1 block skipped" is not.

`rg 'PROVISIONAL\(anomaly-channel\)'` lists the sites to revisit once it
exists. `rg 'DELIBERATE\(fatal\)'` lists the one that must **not** be relaxed
by symmetry.

Do not relax these when it lands; they are containable and want
degrade-and-report, not a new abort:

- making raw runs strict (`udif`)
- erroring on over-long decodes (`udif`)

## Tier 1 remainder: defects returning wrong data

**5. apfs reads are unverified** `[verified]` — high.
`read_object` (`apfs/src/object.rs:73`) checks Fletcher-64 and is **never
called**. Every b-tree, omap and volume read uses the non-verifying
`read_block`: `btree.rs:375,486,514,581`, `omap.rs:20`, `lib.rs:109`.
Superblock and checkpoint reads *are* verified (`superblock.rs:298,344,356`),
so it is not total. Measure `appfs.raw` for Fletcher failures before switching
— the same block-audit approach used for the DMGs — since this could reject
real images.

**6. apfs ignores each extent's logical address** `[verified]` — high.
An APFS extent's logical offset lives in its *key*
(`j_file_extent_key_t.logical_addr`); the value carries only length, physical
block and crypto id. `lookup_file_extents` discards it — `for (_key, val) in
&entries` at `apfs/src/catalog.rs:418` — so both readers reconstruct logical
position by summing lengths: `ApfsForkReader::new` (`extents.rs:90-91`) and
`read_file_data` (`extents.rs:33`).

That is only correct for a dense, in-order file. Across a hole the running sum
under-counts, so every extent after it is placed at the wrong logical offset
and reads return data from elsewhere in the file, with no error. Fixing it
means preserving the key through `lookup_file_extents` and building the map
from it — and deciding what a hole yields (zeros, and ideally a report).

**7. hfsplus returns truncated files as complete** `[verified]` — high.
`read_fork_data` (`hfsplus/src/extents.rs:128`) ends `Ok(bytes_written)` with
no check that it reached `total_bytes`, and `read_file` (`lib.rs:115`) discards
the count entirely.

**8. hfsplus `ForkReader` cannot see overflow extents** `[verified]` — med.
`ForkReader::new` (`hfsplus/src/extents.rs:18`) takes only `reader, fork,
block_size` — no B-tree — so it maps just the eight inline descriptors in
`fork.extents`. Its `Read` impl still uses the full `logical_size`, so reading
past the eighth extent fails `logical_to_physical` and returns
`UnexpectedEof("logical offset beyond extent map")`.

Correction to the earlier audit note: this is a loud error, not a silent short
read. `open_file` is affected; `read_file` is not, because `read_fork_data`
(`extents.rs:165`) does consult the overflow tree. Fixing it means threading
the extents B-tree into the constructor — closer to feature work than a fix.

**9. Comparators return `Less` on undecodable keys** `[verified]` — high.
`apfs/src/catalog.rs:510`, `apfs/src/omap.rs:60`, `hfsplus/src/catalog.rs:278`,
`hfsplus/src/extents.rs:240`. A malformed key steers the descent past itself,
producing a false negative: a file that exists reports as not found. Touches
the comparator contract reworked in `faaebeb` and `145c2fa`, so agree the
approach first.

## Tier 2: silent data modification

Needs the type changes from step 2 of the sequencing, and the
`#[non_exhaustive]` decision first.

- `apfs/src/catalog.rs:355` — `Err(_) => (0, 0, 0)` fabricates size and both
  timestamps; the entry is still returned and is indistinguishable from a real
  zero-length, epoch-stamped file. **The worst single site in apfs.**
- `apfs/src/catalog.rs:348`, `apfs/src/lib.rs:295` — `_ => EntryKind::File`
  reports FIFOs, sockets and devices as regular files
- `apfs/src/catalog.rs:307` — `from_utf8_lossy` plus truncation at the first
  embedded NUL
- `apfs/src/catalog.rs:499` — xattr length clamp returns a short buffer as
  complete
- `apfs/src/superblock.rs:89` — `max_file_systems` clamp silently hides
  volumes past 100; `:230-233` lossy volume name
- `apfs/src/lib.rs:203` — symlink target strips *all* trailing zeros
- `hfsplus/src/unicode.rs:179` — `from_utf16_lossy` on every filename the crate
  emits; `:168` odd-length name loses its trailing byte; `:145` incomplete
  case-fold table mis-orders non-Latin names
- `udif/src/reader.rs:854-870` — partition `ID`/`Name`/`Attributes` default to
  `0`/`""`, and `0` collides with a real partition 0
- `pbzx/src/cpio.rs:263` — `check: 0` for odc conflates "format has no
  checksum" with "checksum is zero"

## Tier 3: fail-safe

One bad entry destroys the whole result.

- `apfs/src/lib.rs:359` + `btree.rs` — one corrupt node kills the entire
  `walk()`
- `apfs/src/catalog.rs:337,342` — `Err(_) => continue` drops entries with no
  counter
- `apfs/src/superblock.rs:293-320` — five silent `continue`s; a fully corrupt
  checkpoint area falls back to the block-0 superblock and the caller sees a
  successful open on stale metadata
- `hfsplus/src/catalog.rs:410` — `?` aborts the listing and discards entries
  already collected; `:399-404` a non-leaf forward link truncates the listing
  and returns `Ok`
- `hfsplus/src/lib.rs:266` — one unreadable subdirectory aborts the whole walk
- `udif/src/reader.rs:459-461` — out-of-bounds block runs dropped with a bare
  `continue`
- `pbzx/src/cpio.rs:548` — device nodes, FIFOs and sockets silently consumed
  and not counted
- `pbzx/src/cpio.rs` `peek_format` guards — a garbage archive lists as empty
  rather than erroring

## Tests and infrastructure

- ~~`test_hfsplus_to_xar_to_pbzx` cannot pass~~ — fixed: repointed at the HFS+
  partition inside `kdk.dmg`. `cargo test -p dpp -- --ignored` is now
  `6 passed; 0 failed`.
- No real-image XAR coverage. The only XAR fixture has no symlinks and no
  `<ea>` blocks, so fixture tests are structurally blind to that bug class.
- `rust-toolchain.toml` exists only on `dev`. Every PR targeting `main` hits
  phantom lints — this already cost an external contributor a wasted round.
  Add the pin to `main` or merge `dev`.
- Decide `#[non_exhaustive]` for data enums. Blocks Tier 2. `apfs` has an
  external dependent (`startup-disk`) that would break on a new variant.
- `apfs` has no `docs/FORMATS.md`, unlike the other format crates.

## Open with the contributor

`xara` PR #5 landed (`834263d`). Four behaviour changes were agreed for a
follow-up PR and are **not** yet done — do not start them without checking, he
may be mid-flight:

1. unrecognised or missing `<type>`, a symlink with no `<link>`, and a
   malformed `id` should not be fatal, and should not be coerced to a default
2. `<link>` is discarded on non-symlinks. **The stated reason was wrong** —
   checked against `lib/stat.c`, xar does *not* write `<link>` for hardlinks.
   Symlinks get `xar_prop_set(f, "link", target)`, a child element; hardlinks
   get `xar_attr_set(f, "type", "link", ...)`, an attribute on `<type>` whose
   value is `original` for the first occurrence and the original's id for the
   rest. So the real gap is that `xara` reads only the *text* of `<type>` and
   drops that attribute, losing hardlink identity entirely. Decide whether to
   model it before touching `<link>` handling.
3. `type` and both checksums still get `.trim()` while `name` and `link` are
   verbatim
4. `extract.rs` propagates the new decode-size errors, so one corrupt entry
   kills the run and leaves a partial tree

~~His `xar-name-base64-enctype` branch decodes `enctype="base64"` on
`<name>`.~~ Landed as PR #6 (`21d8fe8`). It needs no policy conversation
after all: `enctype` is `<name>`-only by construction, so it is not a general
data-transformation question. Verified on both sides of the reference —
`xar_prop_serialize` gates the base64 branch on `key == "name"`,
`xar_prop_unserialize` honours the attribute only when `isname`, and
libarchive sets `base64text` only in its `name` branch and decodes only in
`case FILE_NAME`. Recorded in `xara/docs/FORMATS.md`.

Do **not** extend the decode to `<link>` or other elements: no conforming
writer emits them, no reference reader decodes them, and doing so would both
reinterpret valid archives and add whole-TOC fatal paths for input that xar
and libarchive read without complaint. This was attempted and reverted.
