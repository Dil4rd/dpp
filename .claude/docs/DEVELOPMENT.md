# Development

## Build & Test Commands

```bash
cargo build --release              # Build all crates
cargo build -p <crate>             # Build single crate (pbzx, udif, hfsplus, xara, apfs, cmpfs, dpp, dpp-tool)
cargo test                         # Run all tests except the #[ignore]d fixture ones
cargo test -p dpp                  # Run one crate's tests
cargo test <test_name>             # Run a single test by name
cargo test -- --nocapture          # Show eprintln diagnostic output
cargo test -p apfs -- --ignored    # Run the fixture tests (needs tests/, see below)
cargo run -p dpp-tool -- <cmd>     # Run CLI tool (dmg, fs, hfs, apfs, pkg, payload, info, bench)
cargo run -p dpp-tool -- --in-memory fs info <dmg>  # In-memory extraction mode
cargo run -p pbzx --example pbzx-tool --release -- <file>   # Run pbzx example
cargo run -p udif --example udif-tool --release -- <cmd>    # Run udif example
cargo bench -p apfs                # Run APFS benchmarks (criterion)

# Python bindings (requires maturin + Python 3.9+)
cd dpp-python && maturin develop            # Build and install into current venv
cd dpp-python && maturin develop --release   # Release-mode build
cd dpp-python && python -m pytest tests/     # Run Python tests
python -c "import dpp; print(dir(dpp))"      # Quick smoke test
```

## Test Fixtures

Test fixtures live in `tests/` (large binary files: DMGs, raw partitions, PBZX payloads). The directory is gitignored, so the fixtures exist only on a maintainer's machine.

Every test that needs one is marked `#[ignore]`, and they live in `<crate>/tests/` rather than beside the code: driving a real image is integration testing, and keeping them in a separate target means the compiler rejects one that reaches for a private item instead of letting it pin an implementation detail. The file is `fixtures.rs` in `apfs`, `hfsplus` and `udif`; `dpp` uses `integration.rs` and `cmpfs` uses `real_fixtures.rs`. That has two consequences worth knowing:

- **`cargo test` does not run them, and neither does CI.** The only tests that exercise the parsers against real images run locally, on request, via `cargo test -p <crate> -- --ignored`. Run them before calling parser work finished; a green CI says nothing about whether an image still parses.
- **They fail rather than skip when fixtures are absent.** They `.unwrap()` on `File::open`, so `--ignored` on a machine without `tests/` panics. Do not add `--include-ignored` to CI without changing that.

`tests/decmpfs/` is different from the rest: it is **generated, not collected**. On a Mac, have macOS compress a file — `ditto --hfsCompression` does — then capture its `com.apple.decmpfs` attribute and resource fork verbatim, alongside the original bytes and a TSV manifest. `cmpfs/docs/FIXTURES.md` documents the layout, the capture approach and what a current macOS actually emits. That is the only coverage checking `cmpfs` against bytes Apple wrote — resource-fork types 8, 10 and 12 rest on one third-party reader that marks two of them assumptions. Reading those bytes needs `getxattr` with `XATTR_SHOWCOMPRESSION`; the kernel hides them from ordinary reads, so `xattr(1)` cannot see them.

This gap is why a comparator bug that broke 13 of 15 symlinks in `tests/appfs.raw` passed every check. Synthetic tests that construct their own input are the only kind CI rewards, so prefer adding both: a unit test CI can run, beside the code in `src/`, and an `#[ignore]`d one in `<crate>/tests/fixtures.rs` that proves the behaviour against a real image.

## Workspace Conventions

- All crates use **edition 2024**, **MIT license**.
- The toolchain is pinned in `rust-toolchain.toml`. Bump it deliberately, in its own commit, fixing any newly-introduced lints there — otherwise a Rust release turns CI red on code nobody touched, and it lands on whoever opens the next pull request.
- Apple formats are **big-endian** — `byteorder` is used throughout.
- Each crate has its own `error.rs` with `thiserror`-derived error types.
- Per-crate documentation lives in `<crate>/docs/`: `FORMATS.md` for the on-disk formats (all but `apfs` and `dpp`), plus `CLI.md`, `IMPLEMENTATION.md`, `BENCHMARKS.md` and `COMPARISON.md` where they apply. `apfs` has no `docs/` at all.
- Tests split by what they reach for, not by size. A test that needs a private item stays a `#[cfg(test)]` module in `src/`; one that only drives the public API belongs in `<crate>/tests/`, where the compiler enforces that. `tests/` links dev-dependencies only, so a crate moving tests out may need to repeat a normal dependency there.
- A large `#[cfg(test)]` module goes in its own file as a plain submodule — `mod tests;` in `src/foo.rs` resolves to `src/foo/tests.rs`, and in `src/lib.rs` to `src/tests.rs`. It stays a unit test with `use super::*` and full private access. Do not use `#[path = "..."]` for this: it hard-codes a filename that a later rename leaves pointing at the wrong file, whereas a plain submodule follows the module it belongs to.

## Feature Flags

- **pbzx:** `extract`, `list`, `pack` — all enabled by default.
- **udif:** `extract`, `list`, `create` — all enabled by default.

## CI/CD

| Workflow | What | When |
| --- | --- | --- |
| `ci.yml` | fmt, clippy, tests on three OSes, `parallel` feature, docs, `cargo audit` | push + PR on `main`/`dev`, daily |
| `publish.yml` | packages, then publishes to crates.io | PR into `main` (package only), push to `main` |
| `publish-pypi.yml` | maturin wheels → PyPI | push to `main`, only if the version differs from PyPI |
| `dependencies.yml` | `cargo machete`, wheel licence notices | PR on `main`/`dev` |
| `immutability.yml` | refuses a change to an already-published version | PR on `main`/`dev` |
| `release-readiness.yml` | stale dependencies and action pins, past cooldown | PR into `main`, weekly |

There is no tag step: **merging to `main` publishes.**

Both publish paths use Trusted Publishing, not stored credentials. `publish.yml`
requests `id-token: write` and `rust-lang/crates-io-auth-action` exchanges the
OIDC token for a ~30-minute credential; it reaches cargo as the
`CARGO_REGISTRY_TOKEN` environment variable, which is merely how cargo reads a
credential — no such repository secret exists. `publish-pypi.yml` does the same
via `pypa/gh-action-pypi-publish`. The only `secrets.` reference in any workflow
is the per-run `GITHUB_TOKEN` in `ci.yml`.

Two prerequisites live in repository settings, not in any file; a mismatch fails
at authentication before anything is uploaded:

- Every crate must have this repository, workflow and the environment name
  `publish` registered under Trusted Publishing on crates.io.
- The `publish` environment's deployment branch rule restricts it to `main`.

The checks in the last two rows live in the `xtask` crate rather than in a
script, so `cargo fmt`, `clippy` and `cargo doc` cover the code guarding the
publish path. CI fetches and unpacks; `xtask` decides. That split keeps its only
dependency one the workspace already resolves, adding nothing to what
`cargo audit` scans. `.github/actions/immutability` holds the plumbing both
`immutability.yml` and `publish.yml` share.

`publish-pypi.yml` runs no tests, so it verifies nothing — `ci.yml`'s `python`
job is what covers the bindings. `dependencies.yml` is a separate pipeline on
purpose: both its jobs install a third-party binary from crates.io at job time,
so they run with no credentials and no shared cache.

## Release Flow

Work lands on `dev`, which merges to `main` only when a release is intended.

- Crates being changed carry a `-dev` suffix on `dev` (`0.3.0-dev`), so a tree that differs from what was published never claims the published version number. Internal dependency pins carry it too.
- The release commit strips every `-dev` and dates the `[Unreleased]` changelog sections. It must be the **last** commit before merging to `main`, because the merge publishes immediately.
- It must also regenerate `dpp-python/THIRD-PARTY-LICENSES.md`. That file names every crate **with its version**, workspace members included, so stripping a `-dev` changes it and `dependencies.yml` fails the pull request on the stale copy. Regenerate after the version bump, in the same commit.
- `publish.yml` refuses to **publish** if any `Cargo.toml` still contains a `-dev` version, so a mis-ordered merge fails instead of burning a version number on crates.io permanently. It still **packages** on the release pull request, `-dev` and all — `cargo package` builds a temporary registry from the workspace members, so even an unpublished crate verifies. Checking first would have meant the release pull request could never answer the one question it is for, since the merge that strips `-dev` is also the merge that publishes. On a pull request the leftover suffixes are a notice; on push to `main` they are an error.

### Immutability

A published version's bytes never change, so consumers do not need to pin a
hash. crates.io enforces most of this: a version cannot be republished, the
index records a sha256, and cargo verifies it on download. Trusted Publishing
additionally records the commit behind each version (`apfs 0.4.0` → `4d533fc`),
and git commits are immutable, so that record stays accurate.

What crates.io cannot enforce is the repository's side. `cargo publish` refuses
a version that already exists, and `publish.yml` treats that refusal as success
so a re-run cannot half-release the workspace. The consequence: editing a crate
without bumping its version publishes nothing and still reports success. CI is
green and no consumer receives the change, including for a security fix.

The unit compared is the packaged archive, not just the source, with one
exception. Cargo bundles a copy of `Cargo.lock` into every `.crate`, pruned to
that crate's own dependency closure, and there is no way to exclude it. That
copy is only read for a crate that ships an executable, via
`cargo install --locked`; library dependents resolve from requirements and never
read it. The lock therefore counts for `dpp-tool` and is ignored for the
libraries. Counting it everywhere required almost the whole workspace to
republish after one transitive bump, to change a file no consumer reads.

`[dev-dependencies]` do count: consumers never resolve them, but they are part
of the packaged manifest.

`cargo xtask immutability` compares each packaged `.crate` against the
published archive of the same version, on pull requests to `main` and `dev`. An
unpublished or `-dev` version has nothing to contradict and passes. Two things
are normalised away because they differ on every commit regardless of the
source: `.cargo_vcs_info.json`, which records the producing commit, and the
checksums of workspace members inside the bundled lock. Cargo resolves internal
path dependencies through a registry it builds during packaging, so those
checksums cover an archive created moments earlier. Coverage is unaffected;
whether a member changed is what its own comparison reports.

```bash
cargo package --workspace --locked --no-verify --exclude dpp-python
cargo metadata --no-deps --format-version 1 --locked > target/metadata.json
# then fetch and unpack each published archive as .github/actions/immutability does
cargo xtask immutability
```

`publish.yml` runs the same check in its own `immutable` job, sharing the steps
through a composite action. That job holds no permissions; the `package` job
needs `id-token` and `attestations` write for the attestation step, and this
check needs neither. `publish` depends on both.

It gates the publish job rather than running inside it because publishing is
sequential with sleeps for index propagation: failing partway would leave
earlier crates uploaded and their version numbers permanently taken.

Pull requests are where this should be caught, since adding `-dev` is a one-line
fix there. Both copies pass trivially on a release pull request, because
stripping `-dev` produces versions that are not yet published. The `publish.yml`
copy covers pushes that never went through a pull request.

### Dependency Updates

Driven by CI, not by a bot. `dependabot.yml` was removed because it duplicated
these checks and its one-pull-request-per-bump model fought the rule above,
which makes every dependency change a release. `release-readiness.yml` reports
what is stale, weekly and on every pull request into `main`:

- **Cargo** — `cargo update --dry-run --verbose`. Cargo reports both kinds of
  staleness itself: `Updating x -> y` is inside the declared requirement and
  needs only `cargo update`; `Unchanged x (available: y)` is outside it and
  needs a `Cargo.toml` edit, which changes a published crate and so drags a
  version bump behind it.
- **Actions** — `cargo xtask actions-current`, because no native equivalent of
  `cargo update` exists for pinned shas. Releases are ranked by **version, not
  publication date**: projects backport patches to old major branches, so the
  newest-by-date `actions/checkout` release can be a v2 while v7 is current, and
  ranking by date once recommended a downgrade. `dtolnay/rust-toolchain` is
  skipped, since its tags are Rust versions and the toolchain is bumped
  deliberately.

```bash
cargo update --dry-run --verbose
# the actions check needs release metadata fetched first; see the workflow
cargo xtask actions-current "$(date -u -d '7 days ago' +%Y-%m-%dT%H:%M:%SZ)"
```

Updates then move as a release:

1. `cargo update` for the lockfile updates, and edit the requirement for
   anything reported as `Unchanged ... (available: ...)`. Refresh stale action
   pins.
2. Add `-dev` to every crate the immutability check names, and `[Unreleased]`
   changelog entries.
3. Release as usual: strip `-dev`, regenerate the licence notices, merge.

**The scheduled run is the notification.** Nothing opens a pull request to say a
dependency moved, so a red Monday run is the signal. Confirm GitHub Actions
failure notifications are on — watching the repository is a separate setting,
and this repository has no watchers, which is why the first batch of Dependabot
pull requests went unnoticed.

### Cooldown

Versions published within the last week are not adopted. A compromised release
is usually yanked or reported within days, so waiting means most are identified
before being taken.

Advisories are exempt and are acted on immediately. `cargo audit` does not apply
the cooldown.

The action check applies the window now, as a cutoff date passed to
`cargo xtask actions-current`. The cargo check does not yet, because of the
toolchain. `registry.global-min-publish-age` filters candidate versions by
publication age and falls back to the newest version past the window, naming
what it withheld, but it became a stable cargo feature only in 1.100 and the
workspace pins an earlier release. Without it a week-old update cannot be
distinguished from an hour-old one, and blocking on both would require adopting
versions on their release day. The cargo job therefore reports without
blocking.

To finish it once cargo 1.100 is the stable release, follow the block comment in
`release-readiness.yml`: point that job's toolchain at `stable`, add
`--config 'registry.global-min-publish-age="7 days"'`, and drop its
`continue-on-error`. Keep the change to that job — do not bump
`rust-toolchain.toml` for it, since that is a deliberate commit of its own.

### Security updates

`cargo audit` is the entire advisory pipeline. It reads RustSec directly, runs
daily and on every pull request, and has reported advisories the GitHub Advisory
Database never carried: both `quick-xml` advisories affecting this workspace
appeared with no entry there. Dependabot is decommissioned, so nothing else
watches for advisories and nothing writes the fix. A failing `audit` run is the
signal and a person acts on it.

A dependency bump that fixes an advisory is still a change to a published crate,
so the immutability check fails its pull request until every crate it touches
carries `-dev`. Urgency changes the timing, not the requirement: the release
happens immediately rather than in a batch.
