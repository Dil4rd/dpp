//! Checks that guard publishing, run by CI and locally.
//!
//! CI fetches and unpacks; this crate reads the result and reports. Keeping the
//! I/O in CI means the only dependency is one the workspace already resolves,
//! so nothing here is added to what `cargo audit` scans.
//!
//! ```text
//! cargo xtask publishable      # crates that go to crates.io, one per line
//! cargo xtask immutability     # published archives must match this tree
//! cargo xtask pinned-actions   # every action pinned in .github/workflows
//! cargo xtask actions-current <cutoff>   # pins must be at the newest aged release
//! ```

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Where CI writes `cargo metadata --no-deps`.
const METADATA: &str = "target/metadata.json";
/// Where CI unpacks both sides, as `<ours|theirs>/<name>-<version>/`.
const COMPARE: &str = "target/compare";
/// Where CI writes one releases and one tags response per action repository.
const RELEASES: &str = "target/releases";
const WORKFLOWS: &str = ".github/workflows";

/// Ships to PyPI rather than crates.io, so there is no published archive to
/// compare against. Not marked `publish = false` because that would edit a
/// manifest the wheels contain.
const NOT_ON_CRATES_IO: &[&str] = &["dpp-python"];

/// Records the commit that produced an archive, so it differs on every commit
/// regardless of the source. Describes provenance, not content.
const PROVENANCE: &str = ".cargo_vcs_info.json";

/// Tagged with the Rust version it installs rather than a version of the
/// action, so "newest release" would mean "newest Rust". The toolchain is
/// pinned in `rust-toolchain.toml` and bumped in its own commit.
const NOT_VERSIONED_BY_RELEASE: &[&str] = &["dtolnay/rust-toolchain"];

/// A workspace member, as far as these checks care.
struct Package {
    name: String,
    version: String,
    /// Whether it ships an executable. The bundled `Cargo.lock` is only
    /// reachable for such a crate, via `cargo install --locked`.
    has_bin: bool,
    /// False when `publish = false` excludes it from any registry.
    publishable: bool,
}

impl Package {
    fn archive(&self) -> String {
        format!("{}-{}", self.name, self.version)
    }
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let result = match args.next().as_deref() {
        Some("publishable") => publishable(),
        Some("immutability") => immutability(),
        Some("pinned-actions") => pinned_actions(),
        Some("actions-current") => match args.next() {
            Some(cutoff) => actions_current(&cutoff),
            None => Err("actions-current needs a cutoff timestamp".into()),
        },
        other => Err(format!(
            "unknown task {:?}; see the module documentation",
            other.unwrap_or("<none>")
        )),
    };

    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(message) => {
            eprintln!("xtask: {message}");
            ExitCode::FAILURE
        }
    }
}

type Task = Result<bool, String>;

/// Members that reach crates.io, in a stable order.
fn crates_io_packages() -> Result<Vec<Package>, String> {
    let raw = fs::read_to_string(METADATA)
        .map_err(|e| format!("{METADATA}: {e}; run `cargo metadata --no-deps` first"))?;
    let meta: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("{METADATA}: {e}"))?;

    let mut packages: Vec<Package> = meta["packages"]
        .as_array()
        .ok_or("metadata has no packages array")?
        .iter()
        .map(|p| Package {
            name: p["name"].as_str().unwrap_or_default().to_owned(),
            version: p["version"].as_str().unwrap_or_default().to_owned(),
            has_bin: p["targets"]
                .as_array()
                .is_some_and(|ts| ts.iter().any(|t| t["kind"].to_string().contains("\"bin\""))),
            // `publish` is absent for a publishable crate and an empty array
            // for `publish = false`.
            publishable: p["publish"].as_array().is_none_or(|list| !list.is_empty()),
        })
        .filter(|p| p.publishable && !NOT_ON_CRATES_IO.contains(&p.name.as_str()))
        .collect();

    packages.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(packages)
}

/// Prints `<name> <version>` per crates.io package, for CI to fetch.
fn publishable() -> Task {
    for package in crates_io_packages()? {
        println!("{} {}", package.name, package.version);
    }
    Ok(true)
}

/// Every file in a crate directory, keyed by its path inside the archive.
fn archive_contents(root: &Path) -> Result<BTreeMap<PathBuf, Vec<u8>>, String> {
    let mut files = BTreeMap::new();
    let mut pending = vec![root.to_path_buf()];

    while let Some(dir) = pending.pop() {
        let entries = fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        for entry in entries {
            let path = entry.map_err(|e| format!("{}: {e}", dir.display()))?.path();
            if path.is_dir() {
                pending.push(path);
            } else {
                let relative = path
                    .strip_prefix(root)
                    .map_err(|e| format!("{}: {e}", path.display()))?
                    .to_path_buf();
                let bytes = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
                files.insert(relative, bytes);
            }
        }
    }
    Ok(files)
}

/// Drops the checksum of every workspace member from a bundled `Cargo.lock`.
///
/// Cargo resolves internal path dependencies through a registry it builds
/// during packaging, so the recorded checksum covers an archive created moments
/// earlier, including that archive's provenance file and therefore the current
/// commit. Left in, every crate with an internal dependency reports as changed
/// on every commit.
///
/// Coverage is unaffected: a member is still pinned by version, and whether its
/// contents changed is what its own comparison reports.
fn normalise_lock(lock: &str, members: &[String]) -> String {
    let mut out = String::with_capacity(lock.len());
    let mut current: Option<&str> = None;

    for line in lock.lines() {
        if line == "[[package]]" {
            current = None;
        } else if let Some(name) = line.strip_prefix("name = ") {
            current = Some(name.trim_matches('"'));
        } else if line.starts_with("checksum = ")
            && current.is_some_and(|name| members.iter().any(|m| m == name))
        {
            continue;
        }
        let _ = writeln!(out, "{line}");
    }
    out
}

/// Fails when a crate whose version is published no longer packages to what
/// crates.io holds.
fn immutability() -> Task {
    let packages = crates_io_packages()?;
    let members: Vec<String> = packages.iter().map(|p| p.name.clone()).collect();
    let mut ok = true;

    for package in &packages {
        let archive = package.archive();
        let theirs = Path::new(COMPARE)
            .join("theirs")
            .join(&archive)
            .join(&archive);
        if !theirs.is_dir() {
            // A -dev version is never published, and a new version has no
            // published counterpart. Neither is an error.
            println!("  ok   {archive} — not published, nothing to contradict");
            continue;
        }
        let ours = Path::new(COMPARE)
            .join("ours")
            .join(&archive)
            .join(&archive);
        if !ours.is_dir() {
            return Err(format!(
                "{}: missing; did `cargo package` run?",
                ours.display()
            ));
        }

        let lock = PathBuf::from("Cargo.lock");
        let mut differences = Vec::new();
        let (mine, published) = (archive_contents(&ours)?, archive_contents(&theirs)?);

        let paths: std::collections::BTreeSet<&PathBuf> =
            mine.keys().chain(published.keys()).collect();
        for path in paths {
            if path.as_path() == Path::new(PROVENANCE) {
                continue;
            }
            // The bundled lock is only reachable for a crate that ships an
            // executable, via `cargo install --locked`. Library dependents
            // resolve from requirements and never read it, so comparing it
            // would require a release no consumer could observe.
            if *path == lock && !package.has_bin {
                continue;
            }

            let (a, b) = (published.get(path), mine.get(path));
            let changed = if *path == lock {
                a.map(|v| normalise_lock(&String::from_utf8_lossy(v), &members))
                    != b.map(|v| normalise_lock(&String::from_utf8_lossy(v), &members))
            } else {
                a != b
            };
            if changed {
                differences.push(path.display().to_string());
            }
        }

        if differences.is_empty() {
            println!("  ok   {archive} — identical to the published archive");
        } else {
            ok = false;
            println!("  FAIL {archive} — differs from the published archive:");
            for path in &differences {
                println!("         {path}");
            }
            println!(
                "::error file={}/Cargo.toml::{archive} is published and this tree no longer \
                 matches it; bump the version before merging",
                package.name
            );
        }
    }

    if !ok {
        println!();
        println!("Published versions are immutable. Give every crate listed above a -dev");
        println!("version so the tree stops claiming a version number that is already taken.");
    }
    Ok(ok)
}

/// `owner/repo` for every action pinned in a workflow, with the shas seen.
fn pins() -> Result<BTreeMap<String, Vec<String>>, String> {
    let mut found: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let entries = fs::read_dir(WORKFLOWS).map_err(|e| format!("{WORKFLOWS}: {e}"))?;

    for entry in entries {
        let path = entry.map_err(|e| format!("{WORKFLOWS}: {e}"))?.path();
        if path.extension().is_none_or(|e| e != "yml") {
            continue;
        }
        let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        for line in text.lines() {
            let Some((_, reference)) = line.split_once("uses:") else {
                continue;
            };
            let Some((repo, rest)) = reference.trim().split_once('@') else {
                continue;
            };
            let sha: String = rest.chars().take_while(char::is_ascii_hexdigit).collect();
            // Only sha pins are comparable; tags and branches are not.
            if sha.len() == 40 && repo.contains('/') {
                let shas = found.entry(repo.to_owned()).or_default();
                if !shas.contains(&sha) {
                    shas.push(sha);
                }
            }
        }
    }
    Ok(found)
}

/// Prints every pinned action repository, for CI to query.
fn pinned_actions() -> Task {
    for repo in pins()?.keys() {
        if !NOT_VERSIONED_BY_RELEASE.contains(&repo.as_str()) {
            println!("{repo}");
        }
    }
    Ok(true)
}

/// A version tag as comparable numbers, or `None` when it is not a version.
///
/// Ranking by publication date is incorrect: projects backport patches to old
/// major branches, so the most recently published `actions/checkout` release
/// can be a v2 while v7 is current.
fn tag_order(tag: &str) -> Option<(u64, u64, u64)> {
    let mut parts = tag.trim_start_matches('v').split('.');
    let mut next = || parts.next().unwrap_or("0").parse::<u64>().ok();
    let (major, minor, patch) = (next()?, next().unwrap_or(0), next().unwrap_or(0));
    parts.next().is_none().then_some((major, minor, patch))
}

fn read_json(path: &Path) -> Result<serde_json::Value, String> {
    let raw = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&raw).map_err(|e| format!("{}: {e}", path.display()))
}

/// Fails when a pinned action is behind the newest release past its cooldown.
///
/// `cutoff` is an RFC 3339 timestamp; releases published after it are ignored.
/// String comparison suffices because the API emits one fixed UTC format, which
/// sorts lexicographically.
fn actions_current(cutoff: &str) -> Task {
    let mut stale = Vec::new();

    for (repo, pinned) in pins()? {
        if NOT_VERSIONED_BY_RELEASE.contains(&repo.as_str()) {
            println!("  skip {repo} — tags track the toolchain, not the action");
            continue;
        }

        let slug = repo.replace('/', "__");
        let releases = read_json(&Path::new(RELEASES).join(format!("{slug}.releases.json")))?;
        let tags = read_json(&Path::new(RELEASES).join(format!("{slug}.tags.json")))?;

        let newest = releases
            .as_array()
            .unwrap_or(&Vec::new())
            .iter()
            .filter(|r| {
                r["draft"] == false
                    && r["prerelease"] == false
                    && r["published_at"].as_str().is_some_and(|at| at <= cutoff)
            })
            .filter_map(|r| {
                let tag = r["tag_name"].as_str()?;
                Some((tag_order(tag)?, tag.to_owned()))
            })
            .max();

        let Some((_, tag)) = newest else {
            println!("  ok   {repo} — no release is older than the cutoff");
            continue;
        };

        let head = tags
            .as_array()
            .unwrap_or(&Vec::new())
            .iter()
            .find(|t| t["name"].as_str() == Some(tag.as_str()))
            .and_then(|t| t["commit"]["sha"].as_str())
            .map(str::to_owned);

        let Some(head) = head else {
            println!("  ok   {repo} — {tag} resolves to no commit");
            continue;
        };

        let behind: Vec<&String> = pinned.iter().filter(|sha| **sha != head).collect();
        if behind.is_empty() {
            println!("  ok   {repo} — pinned at {tag}");
        } else {
            println!(
                "  FAIL {repo} — pinned {}, {tag} is {}",
                short(behind[0]),
                short(&head)
            );
            stale.push(format!("{repo}@{head}  # {tag}"));
        }
    }

    if stale.is_empty() {
        return Ok(true);
    }
    println!();
    println!("::error::pinned actions are behind their newest released version");
    println!("Update these pins before merging to main:");
    for pin in &stale {
        println!("    {pin}");
    }
    Ok(false)
}

fn short(sha: &str) -> &str {
    &sha[..8.min(sha.len())]
}

#[cfg(test)]
mod tests;
