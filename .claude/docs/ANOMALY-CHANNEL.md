# The Anomaly Channel

> **Status: proposed, not adopted.** Design options for the reporting channel
> required by [Forensic Direction](FORENSIC-DIRECTION.md), which is itself still
> proposed. Nothing here is a rule.

## The problem

Principle 2 says one malformed entry must not destroy the whole result.
Principle 3 says anything skipped, coerced or degraded must be visible to the
caller. Satisfying 2 without 3 is silent data loss, the most common shape of bug
in this codebase. The channel is how 3 gets satisfied.

`rg 'PROVISIONAL\(anomaly-channel\)'` lists the sites waiting on it.

## Scope of the change

Type-level enforcement — making a degraded read impossible to mistake for a
clean one — is often assumed to require changing every public API across five
independently published crates. It does not.

Only functions that return recovered data need new shapes: `read_file`,
`read_file_to`, `open_file`, `walk`, `list_directory`, `extract_all` and their
siblings. Roughly six to eight per crate.

Every other site — internal parse steps, the places that currently drop an
extent or coerce a type — can record without changing shape, because each
already has `&mut self` or belongs to a handle that can own an accumulator.

Consequence: the recording half is not a breaking change, and it carries most
of the value.

## Reporting model

**An anomaly names the region of the format's own address space that it
invalidates.**

The coordinate space is whatever the format uses to address its items. Byte
ranges are one instance, not the general case.

| Domain | Address space | Region |
|---|---|---|
| udif runs, pbzx chunks, file data | byte offset | byte range |
| apfs / hfsplus b-trees | key | key range, from the parent's separator keys |
| xara TOC | file id / path | subtree under a path prefix |

A byte-offset model does not fit `xara`, whose TOC is XML — "bytes 4000-4100 of
the decompressed TOC" answers no useful question. Under the address-space model
it fits: the TOC is a tree of `<file>` elements, so a failure inside one scopes
to that entry and its children.

This is the recoverability test from
[Forensic Direction](FORENSIC-DIRECTION.md) reused. That document grounds blast
radius in "how the format addresses its items"; the same addressing scheme
supplies the coordinates to report in. Each crate implements one small `Region`
type over an address space it already has — no range algebra, nothing forced
into a byte model.

### A failed b-tree descent has a known location and extent

In `btree_scan_node` (`apfs/src/btree.rs`) the parent node is in hand while
iterating children:

```rust
for i in 0..node.node_header.btn_nkeys as usize {
    let child_oid = node.child_oid(i)?;
    let child_block = resolve_child_oid(...)?;
    let child_data = object::read_block(...)?;   // any of these can fail
    let child_node = BTreeNode::parse(&child_data)?;
```

When child `i` fails, its separator key and the next one are both available, so
the report is exact: *the subtree at oid B, covering keys `[K_i, K_i+1)`, was
not enumerated*. The number of lost records is unknown; the affected extent is
not. A failure is a single located event whether the node was a leaf or an
interior branch.

### Three tiers

| Tier | Content | Cap policy |
|---|---|---|
| Coverage | which regions of the address space are accounted for | never capped |
| Counts | per-kind totals | never capped |
| Detail | message and context per instance | capped: first N located examples |

Counts stay uncapped alongside coverage because first-N locations alone cannot
distinguish 20 lost entries from 20,000, and that difference decides whether a
result is usable.

## Prior art

Checked against the implementations, per
[the reference-verification rule](CODING.md).

### libarchive — severity in the return code

```c
#define	ARCHIVE_OK	  0	/* Operation was successful. */
#define	ARCHIVE_RETRY	(-10)	/* Retry might succeed. */
#define	ARCHIVE_WARN	(-20)	/* Partial success. */
#define	ARCHIVE_FAILED	(-25)	/* Current operation cannot complete. */
#define	ARCHIVE_FATAL	(-30)	/* No more operations are possible. */
```

**Applicable:** the blast-radius taxonomy of
[Forensic Direction](FORENSIC-DIRECTION.md), arrived at independently by a
mature multi-format reader — *degraded but usable* / *this item is dead* / *the
whole container is dead*. Severity as a first-class outcome rather than a
boolean.

**Not applicable:** `archive_errno()` and `archive_error_string()` retain only
the most recent error, unstructured, on shared mutable state. No regions, no
history, nothing programmatically actionable.

### libyal libfsapfs / libfshfs — closest analogues, gap unsolved

Joachim Metz's forensic APFS and HFS+ readers are strict validators that abort
on structural corruption. The recourse offered is a build-time "verbose and
debug output" flag that cascades messages through the library layers.

Open issues covering this use case: libfsapfs
[#12](https://github.com/libyal/libfsapfs/issues/12),
[#41](https://github.com/libyal/libfsapfs/issues/41) "unable to read corrupt
file system"; [#43](https://github.com/libyal/libfsapfs/issues/43) object map
recovery; [#53](https://github.com/libyal/libfsapfs/issues/53) "How to read a
corrupted APFS volume?"; [#33](https://github.com/libyal/libfsapfs/issues/33)
checksum mismatch; [#57](https://github.com/libyal/libfsapfs/issues/57) missing
container key bag.

**Conclusions:** the requirement is real rather than speculative — the leading
library in this space has the same gap and its users hit it. And "strict
validator plus a debug log" is a failure mode to design away from: a build-gated
text stream is not a channel, because nothing downstream can consume it.
Degradation must be data, not logging.

*Evidence quality: issue tracker and wiki, not libfsapfs source. A read of the
library's posture, not a line-level claim.*

### GNU ddrescue — the mapfile

Each record is two integers and a status character: start position, size, and
one of

| Char | Meaning |
|---|---|
| `?` | non-tried block |
| `*` | failed block, non-trimmed |
| `/` | failed block, non-scraped |
| `-` | failed block, bad sector(s) |
| `+` | finished block |

**Applicable:** the mapfile covers its whole domain. Every byte falls inside
some record, so *recovered*, *never attempted* and *attempted and failed* are
three distinguishable states. A `Vec<Anomaly>` cannot represent the second: a
problem list enumerates what went wrong but cannot answer which parts of the
artifact are real. The status characters also encode how hard the tool tried,
and the mapfile is a persistable, resumable artifact rather than an in-memory
report.

### Prisma `Diagnostics` — accumulate, then decide

```rust
pub fn push_error(&mut self, err: DatamodelError)
pub fn push_warning(&mut self, warning: DatamodelWarning)
pub fn has_errors(&self) -> bool
pub fn to_result(&mut self) -> Result<(), Diagnostics>
```

**Applicable:** the `to_result()` hinge. Accumulate during the work, convert to
a `Result` at a chosen boundary. This is how strict and relaxed modes share one
code path.

### No existing crate to depend on

Nothing models region provenance over a recovered artifact. The Rust
diagnostics crates (`miette`, rust-analyzer's and Prisma's internal types)
target compilers and source spans. The crates.io hits for "anomaly" are
unrelated — statistical anomaly detection, and an archived error-context
library. This would be written here.

## Options

### A. `Recovered<T>` at every data-returning API

| Pros | Cons |
|---|---|
| A caller cannot obtain degraded bytes without the record of what is degraded | Largest change; breaking across five crates |
| Enforcement is structural, not conventional | Fixes the anomaly type's shape before real images show which anomalies occur |

### B. Accumulator on the handle

The volume/archive owns the anomalies; parse steps push; callers drain.

| Pros | Cons |
|---|---|
| Zero signature changes; not a breaking change | Ignorable — satisfies the letter of principle 3, not its spirit |
| Works for streaming readers, which have no return slot | Nothing makes a caller look |

### C. Strict / relaxed mode switch

Policy on top of the channel, not a substitute for it. The parser always
records; a policy decides when accumulated anomalies become an `Err`.

| Pros | Cons |
|---|---|
| One implementation, mode chosen at the boundary | A global "careful or not" flag is too coarse — must be per-severity, or strict aborts on a benign trailing hole |
| Precedent: `fsck -p`, libarchive `WARN` vs `FATAL`, `dd conv=noerror` | Useless alone: says nothing about *what* was degraded |

Strict should be the default; a forensic reader that degrades unless asked
otherwise is the thing this effort exists to prevent.

### D. Anomaly budget

A stopping rule rather than a design.

| Pros | Cons |
|---|---|
| Guards against confident garbage from a thoroughly corrupt image | A raw count is arbitrary and scales with image size — a fraction of unrecovered bytes is more defensible |
| Doubles as the overhead backstop | Not a reporting mechanism on its own |

### E. Two-tier API, old names delegating to new

Existing signatures unchanged; reporting variants added beside them; old names
become thin wrappers over the new ones. Precedent: `str::from_utf8` vs
`from_utf8_lossy`.

| Pros | Cons |
|---|---|
| No breaking release; incremental adoption | Converts a bug into documented behaviour — losing anomalies becomes the specified contract of a supported function |
| One implementation, no duplicated logic | The convenient name carries the lossy behaviour, so the default discards |
| External dependent (`startup-disk`) unaffected | |

Three conditions:

- **Discard detail, never severity.** A wrapper that drops anomalies *and*
  their fatality silently returns short buffers again, undoing `42ac895` and
  every comparable fix while being advertised as non-breaking. The wrapper must
  still map anomalies at or above the fatal threshold back to `Err`.
- **`#[deprecated]`, not prose.** API comments and README notes are what libyal
  relies on, and its users still hit the gap. `#[deprecated(since, note)]`
  warns at every call site, appears in rustdoc, and is not breaking. Same
  reasoning as `#[expect]` over `#[allow]` in [Coding](CODING.md): the marker
  should enforce itself.
- **Plan the name swap.** `std` puts the checked behaviour on the unqualified
  name and makes the lossy variant say so. Swap at the next major.

## Reporting overhead

A shattered multi-gigabyte image could produce millions of records.

**Coalesce.** Cost scales with the number of status *transitions*, not with
damaged size — one contiguous bad region is one record whatever its length.
ddrescue's multi-pass algorithm exists partly to "delimit large bad areas fast
[and] keep the mapfile small".
[`rangemap`](https://docs.rs/rangemap/latest/rangemap/) guarantees that
"contiguous and overlapping ranges that map to the same value are coalesced
into a single range."

**Adapt the representation.** Alternating good/bad defeats run-length encoding.
[Roaring bitmaps](https://roaringbitmap.org/about/) partition into 65,536-value
chunks and per chunk use "an uncompressed bitmap, a simple list of integers, or
a list of runs". HFS+ does the same kind of escalation: eight extent descriptors
inline, then the overflow B-tree.

**A bitmap floor bounds the worst case.** One bit per block is 32 MB for a 1 TB
image at 4 KiB blocks, regardless of fragmentation.

**Cap detail, not coverage or counts.** Per the three tiers above. Precedent for
capping: clang stops at `-ferror-limit=20` with "too many errors emitted,
stopping now"; the kernel has `WARN_ONCE` and printk ratelimiting; syslog
collapses to "last message repeated N times".

**Stream rather than accumulate.** ddrescue writes its mapfile periodically,
scaling the interval with size. A caller-installed sink makes library memory
cost O(1).

**Take granularity from the format.** Record at the volume's block size; a
sub-block anomaly is reported as its containing block. Caps record count at
total-blocks.

**The abort fraction is the backstop.** Past some damage level the honest answer
is a coverage summary, not a ten-million-entry list.

## Recommendation

1. **Per-crate `Anomaly` type and accumulator on the handle (B).** No signature
   changes, lands outside any breaking release, converts every
   `PROVISIONAL(anomaly-channel)` site from silently dropped to recorded.
2. **Policy layer (C and D).** `to_result()`-style conversion, strict by
   default, plus the abort fraction. Still no signature changes.
3. **Reporting variants with delegating wrappers (E),** under the three
   conditions above.
4. **Type-level enforcement (A)** at data-returning entry points only, batched
   with the Tier 2 type changes into the single breaking release per crate the
   sequencing already calls for.
5. **Shared crate last, if at all.**

Decide the address-space reporting model before step 1: it is cheap to adopt
there and expensive to retrofit later.

## Open decisions

- **Where the type lives.** The five format crates are siblings with no shared
  dependency; only `dpp` depends on them. A shared vocabulary means either a
  sixth published crate entering five crates' public semver, or per-crate types
  with translation in `dpp` — which duplicates the type five times and puts the
  mapping in the one place that cannot see the original context.
- **Streaming APIs.** `ApfsForkReader` implements `Read`; `read()` returns
  `io::Result<usize>` with no slot for a report. A design that only works for
  whole-operation calls leaves the largest data paths uncovered. Favours B.
- **Whether a hole is an anomaly.** No. An APFS hole is genuinely zeros, not
  missing data (`8d6019e`). *Absent by design* versus *absent because we
  failed* is exactly what the coverage statuses encode; conflating them is a bug
  in either direction.
