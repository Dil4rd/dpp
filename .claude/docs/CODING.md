# Coding Guidelines

Standards and conventions for all code in the dpp workspace.

## Tests required

Every code change must include or update relevant tests. Run all four pre-commit checks (see [CLAUDE.md](../../CLAUDE.md)) before considering work complete. If test fixtures are missing, verify the code compiles and any new unit tests pass.

## Docs required

Update `CHANGELOG.md` in **all** affected crates for user-visible changes. Update `README.md` if public API, features, or usage changes. Update `<crate>/docs/FORMATS.md` if binary format parsing changes.

## Zero warnings policy

Code must pass `cargo clippy -- -D warnings` and `RUSTDOCFLAGS="-D warnings" cargo doc` with no warnings. Fix warnings immediately rather than suppressing them (use `#[allow(...)]` only when the lint is a false positive, with a justifying comment).

## No unsafe code

No `unsafe` code in the `hfsplus` and `apfs` crates. Avoid `unsafe` elsewhere unless strictly necessary with a justifying comment.

## Error handling

Use `thiserror` derive in each crate's `error.rs`. Propagate errors with `?` — do not `unwrap()` or `expect()` in library code.

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
