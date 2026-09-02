# Forensic Direction

> **Status: proposed, not adopted.** Nothing here is a rule yet. It records a
> direction discussed in Sep 2026 and the reasoning behind the changes already
> made under it. Do not cite it as settled convention, and do not surface it in
> user-facing docs until it is actually adopted.

## The three principles

For this to be trusted as a forensic tool rather than a converter, a reader
should:

1. **Not silently modify data.** No coercing an unrecognised on-disk value to a
   default, no lossy decoding without a flag, no clamping, no substituting `0`
   for a value that failed to parse.
2. **Fail safe.** One malformed entry should not destroy the whole result.
   Damaged images are the use case; a tool that cannot produce a partial
   listing of a corrupt disk is useless precisely when it matters.
3. **Be clear about failures.** Anything skipped, coerced or degraded must be
   visible to the caller.

2 and 3 are in tension, and that tension is the whole problem. Silently
skipping satisfies 2 and violates 3. It is the most common shape of bug in
this codebase.

## Where the codebase actually stands

Audited Sep 2026, one agent per crate, top findings spot-checked by hand.

| Crate | No silent modification | Fail safe | Clear about failures | Reporting channel |
|---|---|---|---|---|
| pbzx | ✗ | partial | **partial** | `ExtractStats` (counts only) |
| xara | ✗ | ✓ (by coercing) | ✗ | none |
| apfs | ✗ | partial | ✗ | none |
| hfsplus | ✗ | partial | ✗ | none |
| udif | ✗ | ✗ | ✗ | none |

No crate satisfies principle 1. Only `pbzx` has any mechanism for principle 3,
and it is a bare counter — "3 things were skipped" is not investigable, and a
count is the weak form to avoid when building the real thing.

`pbzx` is the closest to the target and the best model: it rejects unknown CPIO
magic instead of defaulting, uses checked `String::from_utf8` rather than
`from_utf8_lossy` anywhere in its parse path, and errors on a decompressed-size
mismatch instead of clamping.

## The recoverability test

Whether a deviation should be fatal is not a judgement call. It depends on
blast radius, and blast radius is determined by how the format addresses its
items:

- **Absolutely addressed → recoverable.** A UDIF block run carries
  `compressed_offset` into the data fork and `sector_number * 512` in the
  output. Nothing about run N depends on run N-1's decoded length, so a bad
  decode cannot move any other run. Recover the prefix, report the range.
- **Sequentially sized → not recoverable.** A pbzx chunk size or a cpio
  `filesize` *is* the stream position. A wrong one desynchronises every entry
  after it, so continuing produces confident garbage. These must stay fatal.

Check this before choosing, every time. It is the single most useful question
in the whole exercise.

There is a second kind of contamination that independence does not solve. A
partially recovered block is genuine bytes followed by fabricated padding, and
nothing distinguishes them. Containment holds *only if the deviation is
reported*. Without a report, "recover and continue" is just the original bug
with extra steps.

## Sequencing

Ordered so each step is safe on its own:

0. **Fix outright defects first.** No error-handling policy helps when the data
   path is simply wrong.
1. **Fail loudly on structural facts** — magic, versions, declared lengths,
   extents, checksums. Blast radius is one image, not one entry, and precedent
   already exists (`hfsplus/src/volume.rs` rejects unknown versions;
   `udif/src/format.rs` rejects unknown block types).
2. **Make per-entry types honest.** Most principle-1 violations disappear
   without failing anything, because the types can then represent uncertainty:
   `XarFileType::Other(String)` instead of `_ => File`, `Option<u64>`
   timestamps instead of a fabricated `0`.
3. **Add the anomaly channel**, then relax the strictness from step 1 that
   turned out to be too aggressive.

Steps 2 and 3 are both breaking. Batch them into one breaking release per
crate rather than dribbling versions.

## Representing uncertainty

`Option<T>` is not enough. Wherever parsing can fail there are *three* states,
and collapsing two of them is the same information loss the principles forbid:

```
Absent          — the archive did not say
Known(T)        — it said something we understood
Unmodelled(raw) — it said something we kept verbatim but did not interpret
```

This came up for `xara`'s `<type>`, `<link>` and `id` simultaneously. It is
worth one small shared type rather than three bespoke enums. Note the cost:
it changes several public field types at once and reaches into the Python
bindings.

Adding variants to a *data* enum breaks exhaustive matches downstream, and
`apfs` has an external dependent (`startup-disk`). Decide `#[non_exhaustive]`
policy for data types before step 2 — exhaustive stays correct for error enums,
where the compile error in `dpp-python` is the coverage check.

## Verification method

This is the part worth keeping regardless of whether the philosophy is adopted.
Each of these caught a real error during the session.

**Check the blast radius before choosing fail-loud or degrade.** See above.

**Confirm against the reference implementation, not intuition.** Every time it
was consulted it changed the answer:

- `libarchive` settled cpio padding (newc pads data to 4 bytes, odc does not)
  and stated the field width outright: *"entry_bytes_remaining is at least 64
  bits and therefore guaranteed to be big enough for a 33-bit file size."*
- Apple's `xar` `stat.c` lists **nine** file types, and `xar` emits `hardlink`
  itself — so rejecting unknown types rejects well-formed archives.
- `libdmg-hfsplus` compresses exactly `sectorCount * SECTOR_SIZE` per run and
  asserts the read returned that much.
- QEMU and 7-Zip both enforce exact decode lengths; `dmg2img` does not, and is
  the lenient outlier that a reader author could be misled by.

Distinguish two uses of a reference: as evidence of *what data exists in the
wild*, which is authoritative; and as a model for *error handling*, which it is
not — these are conversion tools, and the bar here is higher.

**Measure against real images before and after.** Cheap, and repeatedly
decisive. Scanning all 1929 compressed runs in the fixtures established that
Apple pads every block run; walking 7789 cpio headers established that Apple
ships odc, not newc; scanning the pbzx chunk stream established that it ends at
EOF with no terminator record. Write the throwaway scanner in a worktree.

**Distinguish "does not fire on our fixtures" from "is not a bug."** Several
real defects cannot trigger on any image in `tests/`. That is luck, not safety,
and it is not a reason to skip the fix — but it *is* worth stating in the
commit message, because it tells the next person how much risk the change
carries.

**Suspect a doc that justifies the behaviour you are changing.** Two sections
of `udif/docs/IMPLEMENTATION.md` documented this crate's own writer bug as a
property of the UDIF format, with ❌/✓ markers that read like a considered
decision. Section 5's ancestor was a unit test that compressed a string with
`flate2` and never touched a DMG at all. Careful documentation of the wrong
evidence looks identical to careful documentation of the right evidence.

**Look for sibling sites before declaring a fix complete.** Nearly every defect
found here had a twin: `decode_exact` covered three codecs and missed LZFSE —
the one Apple actually uses; the cpio format dispatch was hand-written ten
times; the stale doc had two sections with identical defects, and the second
was found only after the first was fixed.

**Prefer removing a layer to adding one.** Twice the fix for a duplication bug
was consolidation, not abstraction: the cpio reader detected its format twice
and dispatched by hand ten times, and four LZFSE call sites each open-coded the
same length handling. Both got smaller and correct at once.

## Fixture coverage is thinner than it looks

`tests/` holds six images. Exactly one XAR exists in the whole set
(`KernelDebugKit.pkg` inside `kdk.dmg`), and it has **no symlinks and no `<ea>`
blocks** — so no fixture test can catch the XAR bug class at all, however many
are added. Synthetic unit tests are the only coverage for those.

`dpp/tests/integration.rs::test_hfsplus_to_xar_to_pbzx` has been failing for
some time: `hfsp.raw` is the Google Chrome volume and contains no `.pkg`.
Because it is `#[ignore]`d, CI never sees it. Expect `5 passed; 1 failed` from
`cargo test -p dpp -- --ignored` on a clean tree.
