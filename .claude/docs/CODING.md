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
