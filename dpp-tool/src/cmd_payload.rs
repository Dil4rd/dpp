use std::collections::BTreeMap;
use std::io::{self, Cursor, Write};
use std::path::Path;
use std::time::Instant;

use crate::pipeline::{open_filesystem, open_pipeline};
use crate::style::*;
use crate::{FindArgs, PayloadCommand};

pub(crate) fn run(
    cmd: PayloadCommand,
    mode: dpp::ExtractMode,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        PayloadCommand::Info {
            dmg,
            pkg_path,
            component,
        } => info(&dmg, &pkg_path, &component, mode),
        PayloadCommand::Ls {
            dmg,
            pkg_path,
            component,
            path,
        } => ls(&dmg, &pkg_path, &component, path.as_deref(), mode),
        PayloadCommand::Tree {
            dmg,
            pkg_path,
            component,
            path,
            depth,
        } => tree(&dmg, &pkg_path, &component, path.as_deref(), depth, mode),
        PayloadCommand::Find {
            dmg,
            pkg_path,
            component,
            args,
        } => find(&dmg, &pkg_path, &component, args, mode),
        PayloadCommand::Cat {
            dmg,
            pkg_path,
            component,
            file,
        } => cat(&dmg, &pkg_path, &component, &file, mode),
        PayloadCommand::Extract {
            dmg,
            pkg_path,
            component,
            path,
            output,
        } => extract(&dmg, &pkg_path, &component, path.as_deref(), &output, mode),
    }
}

/// Normalize a CPIO path: strip leading "./" or "/" and represent root as "".
fn normalize_cpio(p: &str) -> String {
    let p = p.strip_prefix("./").unwrap_or(p);
    let p = p.strip_prefix('/').unwrap_or(p);
    if p == "." || p.is_empty() {
        String::new()
    } else {
        p.to_string()
    }
}

/// Turn a user-supplied path (like "/Library" or "Library") into the same form as normalize_cpio.
fn normalize_user_path(p: &str) -> String {
    let p = p.strip_prefix('/').unwrap_or(p);
    if p == "." || p.is_empty() {
        String::new()
    } else {
        p.trim_end_matches('/').to_string()
    }
}

/// Get the parent portion of a normalized path ("" for root-level entries).
fn parent_of(p: &str) -> &str {
    match p.rfind('/') {
        Some(i) => &p[..i],
        None => "",
    }
}

/// Get the file name portion of a normalized path.
fn basename_of(p: &str) -> &str {
    p.rsplit_once('/').map(|(_, name)| name).unwrap_or(p)
}

/// Open the PBZX payload for a component and return the parsed Archive.
fn open_archive(
    dmg_path: &Path,
    pkg_path: &str,
    component: &str,
    mode: dpp::ExtractMode,
) -> Result<pbzx::Archive, Box<dyn std::error::Error>> {
    let mut pipeline = open_pipeline(dmg_path)?;
    let mut fs = open_filesystem(&mut pipeline, mode)?;

    spinner_msg(&format!("Opening {pkg_path}"));
    let t = Instant::now();
    let mut pkg = fs.open_pkg(pkg_path)?;
    spinner_done(&format!(" ({})", format_duration(t.elapsed())));

    spinner_msg("Decompressing payload");
    let t = Instant::now();
    let payload = pkg.payload(component)?;
    let archive = pbzx::Archive::from_reader(Cursor::new(payload))?;
    spinner_done(&format!(
        " ({} decompressed, {})",
        format_size(archive.decompressed_size() as u64),
        format_duration(t.elapsed()),
    ));

    Ok(archive)
}

// ── info ────────────────────────────────────────────────────────────────

fn info(
    dmg_path: &Path,
    pkg_path: &str,
    component: &str,
    mode: dpp::ExtractMode,
) -> Result<(), Box<dyn std::error::Error>> {
    let archive = open_archive(dmg_path, pkg_path, component, mode)?;
    let entries = archive.list()?;

    header(&format!("Payload: {component}"));

    section("CPIO Archive");
    kv(
        "Decompressed size",
        &format_size(archive.decompressed_size() as u64),
    );

    let file_count = entries
        .iter()
        .filter(|e| !e.is_dir && !e.is_symlink)
        .count();
    let dir_count = entries.iter().filter(|e| e.is_dir).count();
    let symlink_count = entries.iter().filter(|e| e.is_symlink).count();
    let total_size: u64 = entries.iter().filter(|e| !e.is_dir).map(|e| e.size).sum();

    section("Contents");
    kv("Entries", &entries.len().to_string());
    kv_highlight("Files", &format_commas(file_count as u64));
    kv_highlight("Directories", &format_commas(dir_count as u64));
    if symlink_count > 0 {
        kv("Symlinks", &format_commas(symlink_count as u64));
    }
    kv("Total file size", &format_size(total_size));
    println!();

    Ok(())
}

// ── ls ──────────────────────────────────────────────────────────────────

fn ls(
    dmg_path: &Path,
    pkg_path: &str,
    component: &str,
    path: Option<&str>,
    mode: dpp::ExtractMode,
) -> Result<(), Box<dyn std::error::Error>> {
    let dir_arg = path.unwrap_or("/");
    let dir = normalize_user_path(dir_arg);

    let archive = open_archive(dmg_path, pkg_path, component, mode)?;
    let entries = archive.list()?;

    // Collect direct children of `dir`
    let mut children: Vec<_> = entries
        .iter()
        .filter(|e| {
            let np = normalize_cpio(&e.path);
            if np.is_empty() {
                return false;
            }
            parent_of(&np) == dir
        })
        .collect();

    // Sort: dirs first, then by name
    children.sort_by(|a, b| {
        let a_dir = a.is_dir;
        let b_dir = b.is_dir;
        b_dir.cmp(&a_dir).then_with(|| {
            basename_of(&normalize_cpio(&a.path)).cmp(basename_of(&normalize_cpio(&b.path)))
        })
    });

    let (d, r) = (dim(), reset());
    let display_dir = if dir.is_empty() { "/" } else { dir_arg };
    header(&format!("Payload: {component} — {display_dir}"));
    println!();
    println!("  {d}{:<5} {:>12}  Name{r}", "Kind", "Size");
    println!("  {d}{}{r}", "-".repeat(56));

    for entry in &children {
        let np = normalize_cpio(&entry.path);
        let name = basename_of(&np);
        let (color, icon) = entry_style(entry);
        let size_str = if entry.is_dir {
            "-".to_string()
        } else {
            format_size(entry.size)
        };
        let suffix = symlink_suffix(entry);
        println!(
            "  {d}{icon}{r}   {:>12}  {color}{name}{r}{suffix}",
            size_str,
        );
    }

    println!();
    let fc = children
        .iter()
        .filter(|e| !e.is_dir && !e.is_symlink)
        .count();
    let dc = children.iter().filter(|e| e.is_dir).count();
    println!("  {d}{fc} file(s), {dc} directory(ies){r}");
    println!();

    Ok(())
}

// ── tree ────────────────────────────────────────────────────────────────

fn tree(
    dmg_path: &Path,
    pkg_path: &str,
    component: &str,
    path: Option<&str>,
    max_depth: usize,
    mode: dpp::ExtractMode,
) -> Result<(), Box<dyn std::error::Error>> {
    let base_arg = path.unwrap_or("/");
    let base = normalize_user_path(base_arg);

    let archive = open_archive(dmg_path, pkg_path, component, mode)?;
    let entries = archive.list()?;

    // Build parent → children map
    let mut children_map: BTreeMap<String, Vec<&pbzx::FileEntry>> = BTreeMap::new();
    for entry in &entries {
        let np = normalize_cpio(&entry.path);
        if np.is_empty() {
            continue;
        }
        let parent = parent_of(&np).to_string();
        children_map.entry(parent).or_default().push(entry);
    }

    // Sort children in each bucket: dirs first, then by name
    for bucket in children_map.values_mut() {
        bucket.sort_by(|a, b| {
            b.is_dir.cmp(&a.is_dir).then_with(|| {
                basename_of(&normalize_cpio(&a.path)).cmp(basename_of(&normalize_cpio(&b.path)))
            })
        });
    }

    let display_base = if base.is_empty() { "/" } else { base_arg };
    header(&format!("Tree: {component} — {display_base}"));
    println!();

    print_tree(&children_map, &base, "", 0, max_depth);
    println!();

    Ok(())
}

fn print_tree(
    children_map: &BTreeMap<String, Vec<&pbzx::FileEntry>>,
    dir: &str,
    prefix: &str,
    depth: usize,
    max_depth: usize,
) {
    let (d, r, b) = (dim(), reset(), bold());

    if depth > max_depth {
        println!("  {prefix}{d}{TEE} ...{r}");
        return;
    }

    let entries = match children_map.get(dir) {
        Some(v) => v,
        None => return,
    };

    for (i, entry) in entries.iter().enumerate() {
        let np = normalize_cpio(&entry.path);
        let name = basename_of(&np);
        let is_last = i == entries.len() - 1;
        let connector = if is_last { ELBOW } else { TEE };
        let child_prefix = if is_last {
            format!("{prefix}    ")
        } else {
            format!("{prefix}{PIPE}   ")
        };

        let (color, _) = entry_style(entry);
        let size_str = if !entry.is_dir {
            format!("  {d}{}{r}", format_size(entry.size))
        } else {
            String::new()
        };
        let suffix = symlink_suffix(entry);

        println!("  {prefix}{d}{connector}{r} {color}{b}{name}{r}{suffix}{size_str}",);

        if entry.is_dir {
            print_tree(children_map, &np, &child_prefix, depth + 1, max_depth);
        }
    }
}

// ── find ────────────────────────────────────────────────────────────────

fn find(
    dmg_path: &Path,
    pkg_path: &str,
    component: &str,
    args: FindArgs,
    mode: dpp::ExtractMode,
) -> Result<(), Box<dyn std::error::Error>> {
    let name_pattern = args.name;
    let type_filter: Option<&str> = match args.file_type {
        None => None,
        Some('f') => Some("f"),
        Some('d') => Some("d"),
        Some('l') => Some("l"),
        Some(other) => return Err(format!("unknown type '{other}' (use f, d, or l)").into()),
    };

    let archive = open_archive(dmg_path, pkg_path, component, mode)?;
    let entries = archive.list()?;

    let matches: Vec<_> = entries
        .iter()
        .filter(|e| {
            let np = normalize_cpio(&e.path);
            if np.is_empty() {
                return false;
            }

            if let Some(tf) = type_filter {
                match tf {
                    "f" if e.is_dir || e.is_symlink => return false,
                    "d" if !e.is_dir => return false,
                    "l" if !e.is_symlink => return false,
                    _ => {}
                }
            }

            if let Some(ref pattern) = name_pattern
                && !glob_match(pattern, basename_of(&np))
            {
                return false;
            }

            true
        })
        .collect();

    let (d, r) = (dim(), reset());
    println!();
    if matches.is_empty() {
        println!("  {d}No matches found{r}");
    } else {
        for entry in &matches {
            let np = normalize_cpio(&entry.path);
            let (color, icon) = entry_style(entry);
            let size_str = if !entry.is_dir {
                format!("  {d}{}{r}", format_size(entry.size))
            } else {
                String::new()
            };
            println!("  {d}{icon}{r} {color}/{np}{r}{size_str}",);
        }
        println!();
        println!("  {d}{} match(es){r}", matches.len());
    }
    println!();

    Ok(())
}

// ── cat ─────────────────────────────────────────────────────────────────

fn cat(
    dmg_path: &Path,
    pkg_path: &str,
    component: &str,
    file_path: &str,
    mode: dpp::ExtractMode,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut pipeline = dpp::DmgPipeline::open(dmg_path)?;
    let mut fs = pipeline.open_filesystem_with_mode(mode)?;
    let mut pkg = fs.open_pkg(pkg_path)?;

    let payload = pkg.payload(component)?;
    let archive = pbzx::Archive::from_reader(Cursor::new(payload))?;

    // Normalize the requested path to match CPIO conventions
    let normalized = normalize_user_path(file_path);
    // Try both with and without "./" prefix
    let data = archive
        .extract_file(&normalized)
        .or_else(|_| archive.extract_file(&format!("./{normalized}")))
        .or_else(|_| archive.extract_file(file_path))?;

    let mut stdout = io::stdout().lock();
    stdout.write_all(&data)?;

    Ok(())
}

// ── extract ─────────────────────────────────────────────────────────────

fn extract(
    dmg_path: &Path,
    pkg_path: &str,
    component: &str,
    filter_path: Option<&str>,
    output: &Path,
    mode: dpp::ExtractMode,
) -> Result<(), Box<dyn std::error::Error>> {
    let archive = open_archive(dmg_path, pkg_path, component, mode)?;

    let display_path = filter_path.unwrap_or("/");
    header(&format!("Extract Payload: {component}:{display_path}"));

    let base = filter_path.unwrap_or("/");

    spinner_msg("Extracting files");
    let t = Instant::now();
    let stats = archive.extract_path(base, output)?;
    spinner_done(&format!(" ({})", format_duration(t.elapsed())));

    if stats.symlinks_skipped > 0 {
        warn_msg(&format!(
            "{} symlink(s) skipped during extraction",
            stats.symlinks_skipped
        ));
    }

    let (d, r) = (dim(), reset());
    println!();
    section("Extraction Summary");
    kv("Output", &output.display().to_string());
    kv("Directories", &format_commas(stats.dirs));
    kv_highlight("Files", &format_commas(stats.files));
    if stats.symlinks_skipped > 0 {
        kv("Symlinks skipped", &format_commas(stats.symlinks_skipped));
    }
    kv(
        "Total size",
        &format!(
            "{} {d}({} bytes){r}",
            format_size(stats.bytes),
            format_commas(stats.bytes)
        ),
    );
    println!();

    Ok(())
}

// ── helpers ─────────────────────────────────────────────────────────────

fn entry_style(entry: &pbzx::FileEntry) -> (&str, &str) {
    if entry.is_dir {
        (blue(), "dir")
    } else if entry.is_symlink {
        (cyan(), "lnk")
    } else {
        (white(), "   ")
    }
}

fn symlink_suffix(entry: &pbzx::FileEntry) -> String {
    if let Some(ref target) = entry.link_target {
        let d = dim();
        let r = reset();
        format!(" {d}-> {target}{r}")
    } else {
        String::new()
    }
}
