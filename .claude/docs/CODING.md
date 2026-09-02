# Coding Guidelines

Standards and conventions for all code in the dpp workspace.

## Tests required

Every code change must include or update relevant tests. Run all four pre-commit checks (see [CLAUDE.md](../../CLAUDE.md)) before considering work complete. If test fixtures are missing, verify the code compiles and any new unit tests pass.

## Docs required

Update `CHANGELOG.md` in **all** affected crates for user-visible changes. Update `README.md` if public API, features, or usage changes. Update `<crate>/docs/FORMATS.md` if binary format parsing changes.

## Zero warnings policy

Code must pass `cargo clippy -- -D warnings` and `RUSTDOCFLAGS="-D warnings" cargo doc` with no warnings. Fix warnings immediately rather than suppressing them.

When a lint genuinely does not apply, prefer `#[expect(lint, reason = "…")]` over `#[allow(...)]`. `expect` fails the build once the item stops tripping the lint, so the annotation deletes itself instead of outliving its justification. Fields parsed purely to mirror an on-disk layout are the usual legitimate case.

Run clippy over the **whole** workspace, including `dpp-python`. It is excluded from the test and doc gates because it needs Python headers, but not from clippy — and it is the only place a new `ApfsError` variant is caught (see below).

## No unsafe code

No `unsafe` code in the `hfsplus` and `apfs` crates. Avoid `unsafe` elsewhere unless strictly necessary with a justifying comment.

## Public API surface

These crates are published, so `pub` is a semver commitment. New items default to `pub(crate)` unless something outside the crate actually calls them — check before widening, and check before narrowing too: `apfs` has a third-party dependent (`startup-disk`) that drives `catalog`, `omap`, `object`, `superblock` and `extents` directly rather than through `ApfsVolume`.

Keep an API private when using it correctly requires knowledge the caller has no way to check. `apfs::btree` is the worked example: its comparator must reproduce the tree's on-disk key ordering, and breaking that returns `Ok(None)` for records that exist rather than an error. Higher-level readers own their comparators so callers never face that contract.

## Error handling

Use `thiserror` derive in each crate's `error.rs`. Propagate errors with `?` — do not `unwrap()` or `expect()` in library code.

Distinguish a malformed image (`CorruptedData`) from a well-formed one using a feature the reader does not implement (`Unsupported`). Conflating them tells callers to distrust a volume that is fine.

`ApfsError` deliberately has **no** `#[non_exhaustive]`. `dpp-python`'s `apfs_to_pyerr` matches it exhaustively, so adding a variant fails the build until it has a deliberate Python mapping. That compile error is the coverage check; `#[non_exhaustive]` would require a wildcard arm and let new variants fall silently into whatever bucket it names. The accepted cost is a semver major per new variant.

## Provisional strictness

Some readers currently **fail** on a per-entry deviation that they should
eventually **recover from and report**. That is deliberate but temporary: there
is no channel for reporting a degraded result yet, so the only alternative to
failing is failing silently, which is worse for a forensic tool.

A deviation is recoverable when its blast radius is one item. UDIF block runs
qualify: each is addressed absolutely on both sides — `compressed_offset` into
the data fork, `sector_number * 512` in the output — so a bad decode cannot
move any other run. Sequential formats do not qualify: a pbzx chunk size or a
cpio `filesize` *is* the stream position, so a wrong one desynchronises
everything after it and must stay fatal.

Mark both kinds so neither is lost:

```rust
// PROVISIONAL(anomaly-channel): fail now, degrade and report once there is
// somewhere to report to. Blast radius is this run only.

// DELIBERATE(fatal): not recoverable — the declared size is the stream
// position, so continuing past a wrong one corrupts every later entry.
```

`rg 'PROVISIONAL\(anomaly-channel\)'` lists the sites to revisit when the
channel lands. When relaxing one, the report has to carry the affected byte
range, not just a counter: "bytes 4096-4608 of partition 0 were not recovered"
is investigable, "1 block skipped" is not.

## Generic I/O

Keep reader types generic over `R: Read + Seek`. No trait objects — fully monomorphized.

## Big-endian

Apple formats are big-endian. Use `byteorder` for all binary reads/writes.

## Safe arithmetic and indexing

Prefer checked arithmetic (`checked_add`, `checked_mul`, `checked_sub`) and fallible slice access (`.get(...)`) over raw operators and direct indexing. This prevents panics from overflow or out-of-bounds access when processing untrusted binary data.

```rust
// Good — propagates None/Err on overflow or out-of-bounds
let end = start.checked_add(len).ok_or(Error::Overflow)?;
let slice = buf.get(start..end).ok_or(Error::Truncated)?;

// Bad — panics on overflow or out-of-bounds
let slice = &buf[start..start + len];
```
