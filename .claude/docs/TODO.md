# Open Work

Findings from the Sep 2026 audit, ordered by what is safe to do next. Reasoning
and criteria live in [Forensic Direction](FORENSIC-DIRECTION.md).

Line numbers were correct as of the Sep 2026 audit. Severity is about what
reaches the caller: **high** = wrong data returned as if it were right;
**med** = silent omission; **low** = cosmetic or unreachable.

Provenance is marked. Items I verified by reading the code or measuring are
`[verified]`; items reported by an audit agent and not independently checked
are `[unverified]` — confirm before acting.

## Done

Tier 1 items 1-4 and 10-12, in udif 0.4.0, pbzx 0.4.0 and apfs 0.3.0:

- udif: short decodes rejected across all four codecs; writer pads the final
  chunk; raw remainder uses checked arithmetic
- pbzx: cpio entry data read in the format its header declared; odc filesize
  and mtime widened past 32 bits; zero chunk header with data behind it
  rejected
- apfs: extent block numbers that overflow the address space rejected

Item 6 in apfs 0.3.0: `lookup_extents` returns `FileExtentRecord`, so each
extent is placed at the `logical_addr` from its own key rather than at the sum
of preceding lengths. Interior holes read as zeros and `ApfsForkReader` reads
through them. Overlapping extents and a gap after the final extent stay
`PROVISIONAL(anomaly-channel)`: the trailing case cannot be told apart from
lost extent records, so it is a short count rather than fabricated zeros.

The `read_file` half of item 7 in hfsplus 0.3.0; see that entry for the rest.

## Next: the anomaly channel

Design options, prior art and a staged proposal are in
[Anomaly Channel](ANOMALY-CHANNEL.md) — proposed, awaiting review.

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

**5. apfs reads are unverified** `[verified]` — high. **Measured; safe to
proceed.** A tracing run over `appfs.raw` (`ApfsVolume::open` + `walk()`,
302 entries) touched 61 distinct blocks through those six sites; all 61 pass
`fletcher::verify_object`, so `fletcher.rs` is correct as written and the
switch would not reject this healthy image. The only failures anywhere were
4 all-zero blocks (197367-197370), unwritten slots in the checkpoint
descriptor ring — their stored checksum is 0 and the Fletcher-64 of an
all-zero buffer is `0xFFFF_FFFF_FFFF_FFFF`. Those are read by
`find_latest_nxsb` (`superblock.rs:291`), which already calls
`verify_object` and uses the failure to *identify* unused slots; it is not
one of the six sites and is unaffected.

`read_object` (`apfs/src/object.rs:73`) checks Fletcher-64 and is **never
called**. Every b-tree, omap and volume read uses the non-verifying
`read_block`: `btree.rs:375,486,514,581`, `omap.rs:20`, `lib.rs:109`.
Superblock and checkpoint reads *are* verified (`superblock.rs:298,344,356`),
so it is not total. Measure `appfs.raw` for Fletcher failures before switching
— the same block-audit approach used for the DMGs — since this could reject
real images.

**7. hfsplus `read_fork_data` reports a short read as success** `[verified]`
— med, and waiting on the anomaly channel by design.

`read_fork_data` (`hfsplus/src/extents.rs:128`) ends `Ok(bytes_written)` with
no check that it reached `total_bytes`, so a fork whose extents end early
returns a byte count the caller must compare against `logical_size` itself.

As of hfsplus 0.3.0 `read_file` does that comparison and fails with
`CorruptedData` rather than returning a short buffer, so the common path is
covered. That strictness is *terminal, not provisional* — its return type is
`Vec<u8>`, which cannot express "complete except for a hole", so it must be
complete or fail whatever the reporting policy becomes. The channel would add
a sibling API, not change this one.

`read_fork_data` is left as it is on purpose. It is the streaming primitive, a
truncated fork is absolutely addressed, and its target state is
degrade-and-report — so making it strict now would only have to be relaxed
again. Do it once, with the channel. `read_file_to` still returns the
recovered count, and a test pins that asymmetry so it is not "fixed" by
symmetry.

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
the comparator contract, so agree the approach first.

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

- No real-image XAR coverage. The only XAR fixture has no symlinks and no
  `<ea>` blocks, so fixture tests are structurally blind to that bug class.
- `rust-toolchain.toml` exists only on `dev`. Every PR targeting `main` hits
  phantom lints — this already cost an external contributor a wasted round.
  Add the pin to `main` or merge `dev`.
- Decide `#[non_exhaustive]` for data enums. Blocks Tier 2. `apfs` has an
  external dependent (`startup-disk`) that would break on a new variant.
- `apfs` has no `docs/FORMATS.md`, unlike the other format crates.

## Open in xara

Four behaviours agreed for a follow-up and not yet done as of xara 0.4.0.
Check with the contributor before starting any of them; he may be mid-flight.

1. unrecognised or missing `<type>`, a symlink with no `<link>`, and a
   malformed `id` are fatal, and are coerced to a default; neither should
   happen
2. `<link>` is discarded on non-symlinks. Note the format detail, since the
   obvious assumption is wrong: xar writes `<link>` only for symlinks
   (`xar_prop_set(f, "link", target)` in `lib/stat.c`). Hardlinks instead carry
   `xar_attr_set(f, "type", "link", ...)` — an attribute on `<type>` valued
   `original` for the first occurrence and the original's id for the rest.
   `xara` reads only the *text* of `<type>` and drops that attribute, so
   hardlink identity is lost entirely. Decide whether to model it before
   touching `<link>` handling.
3. `type` and both checksums get `.trim()` while `name` and `link` are
   verbatim
4. `extract.rs` propagates the decode-size errors, so one corrupt entry kills
   the run and leaves a partial tree

`enctype="base64"` on `<name>` is decoded as of xara 0.4.0, and needs no
policy decision: it is `<name>`-only by construction, not a general data
transformation. Both sides of the reference agree — `xar_prop_serialize` gates
the base64 branch on `key == "name"`, `xar_prop_unserialize` honours the
attribute only when `isname`, and libarchive sets `base64text` only in its
`name` branch and decodes only in `case FILE_NAME`. Recorded in
`xara/docs/FORMATS.md`.

Do **not** extend the decode to `<link>` or other elements. No conforming
writer emits them and no reference reader decodes them, so doing so would
reinterpret valid archives and add whole-TOC fatal paths for input that xar
and libarchive read without complaint.
