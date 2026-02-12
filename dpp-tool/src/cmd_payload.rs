use std::collections::BTreeMap;
use std::io::{self, Cursor, Write};
use std::process;
use std::time::Instant;

use crate::style::*;
use crate::pipeline::{open_pipeline, open_filesystem};

pub(crate) fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.is_empty() {
        print_usage();
        process::exit(1);
    }
    match args[0].as_str() {
        "info" => info(&args[1..]),
        "ls" => ls(&args[1..]),
        "tree" => tree(&args[1..]),
        "find" => find(&args[1..]),
        "cat" => cat(&args[1..]),
        "-h" | "--help" | "help" => { print_usage(); Ok(()) }
        _ => {
            eprintln!("{RED}Unknown payload command: {}{RESET}", args[0]);
            print_usage();
            process::exit(1);
        }
    }
}

fn print_usage() {
    eprintln!(
        r#"
{BOLD}dpp-tool payload{RESET} — Component payload (PBZX/CPIO) commands

{BOLD}COMMANDS:{RESET}
    {GREEN}info{RESET}   <dmg> <pkg-path> <component>                            Payload stats
    {GREEN}ls{RESET}     <dmg> <pkg-path> <component> [path]                     List files
    {GREEN}tree{RESET}   <dmg> <pkg-path> <component> [path]                     Browse file tree
    {GREEN}find{RESET}   <dmg> <pkg-path> <component> [-name pat] [-type f|d|l]  Find files
    {GREEN}cat{RESET}    <dmg> <pkg-path> <component> <file>                     Extract file to stdout
"#
    );
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
    match p.rfind('/') {
        Some(i) => &p[i + 1..],
        None => p,
    }
}

/// Open the PBZX payload for a component and return the parsed Archive.
fn open_archive(
    dmg_path: &str,
    pkg_path: &str,
    component: &str,
) -> Result<pbzx::Archive, Box<dyn std::error::Error>> {
    let mut pipeline = open_pipeline(dmg_path)?;
    let mut fs = open_filesystem(&mut pipeline)?;

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

fn info(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.len() < 3 {
        eprintln!("Usage: dpp-tool payload info <dmg> <pkg-path> <component>");
        process::exit(1);
    }

    let archive = open_archive(&args[0], &args[1], &args[2])?;
    let entries = archive.list()?;

    header(&format!("Payload: {}", args[2]));

    section("CPIO Archive");
    kv("Decompressed size", &format_size(archive.decompressed_size() as u64));

    let file_count = entries.iter().filter(|e| !e.is_dir && !e.is_symlink).count();
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

fn ls(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.len() < 3 {
        eprintln!("Usage: dpp-tool payload ls <dmg> <pkg-path> <component> [path]");
        process::exit(1);
    }

    let dir_arg = if args.len() > 3 { &args[3] } else { "/" };
    let dir = normalize_user_path(dir_arg);

    let archive = open_archive(&args[0], &args[1], &args[2])?;
    let entries = archive.list()?;

    // Collect direct children of `dir`
    let mut children: Vec<_> = entries
        .iter()
        .filter(|e| {
            let np = normalize_cpio(&e.path);
            if np.is_empty() { return false; }
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

    let display_dir = if dir.is_empty() { "/" } else { dir_arg };
    header(&format!("Payload: {} — {display_dir}", args[2]));
    println!();
    println!(
        "  {DIM}{:<5} {:>12}  {}{RESET}",
        "Kind", "Size", "Name"
    );
    println!("  {DIM}{}{RESET}", "-".repeat(56));

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
            "  {DIM}{icon}{RESET}   {:>12}  {color}{name}{RESET}{suffix}",
            size_str,
        );
    }

    println!();
    let fc = children.iter().filter(|e| !e.is_dir && !e.is_symlink).count();
    let dc = children.iter().filter(|e| e.is_dir).count();
    println!("  {DIM}{fc} file(s), {dc} directory(ies){RESET}");
    println!();

    Ok(())
}

// ── tree ────────────────────────────────────────────────────────────────

fn tree(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.len() < 3 {
        eprintln!("Usage: dpp-tool payload tree <dmg> <pkg-path> <component> [path]");
        process::exit(1);
    }

    let base_arg = if args.len() > 3 { &args[3] } else { "/" };
    let base = normalize_user_path(base_arg);

    let archive = open_archive(&args[0], &args[1], &args[2])?;
    let entries = archive.list()?;

    // Build parent → children map
    let mut children_map: BTreeMap<String, Vec<&pbzx::FileEntry>> = BTreeMap::new();
    for entry in &entries {
        let np = normalize_cpio(&entry.path);
        if np.is_empty() { continue; }
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
    header(&format!("Tree: {} — {display_base}", args[2]));
    println!();

    print_tree(&children_map, &base, "", 0, 3);
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
    if depth > max_depth {
        println!("  {prefix}{DIM}{TEE} ...{RESET}");
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
            format!("  {DIM}{}{RESET}", format_size(entry.size))
        } else {
            String::new()
        };
        let suffix = symlink_suffix(entry);

        println!(
            "  {prefix}{DIM}{connector}{RESET} {color}{BOLD}{name}{RESET}{suffix}{size_str}",
        );

        if entry.is_dir {
            print_tree(children_map, &np, &child_prefix, depth + 1, max_depth);
        }
    }
}

// ── find ────────────────────────────────────────────────────────────────

fn find(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.len() < 3 {
        eprintln!("Usage: dpp-tool payload find <dmg> <pkg-path> <component> [-name <pattern>] [-type f|d|l]");
        process::exit(1);
    }

    let mut name_pattern: Option<String> = None;
    let mut type_filter: Option<&str> = None;

    let mut i = 3;
    while i < args.len() {
        match args[i].as_str() {
            "-name" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("{RED}error:{RESET} -name requires a pattern argument");
                    process::exit(1);
                }
                name_pattern = Some(args[i].clone());
            }
            "-type" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("{RED}error:{RESET} -type requires an argument (f, d, or l)");
                    process::exit(1);
                }
                match args[i].as_str() {
                    "f" | "d" | "l" => type_filter = Some(if args[i] == "f" { "f" } else if args[i] == "d" { "d" } else { "l" }),
                    other => {
                        eprintln!("{RED}error:{RESET} unknown type '{other}' (use f, d, or l)");
                        process::exit(1);
                    }
                }
            }
            other => {
                eprintln!("{RED}error:{RESET} unknown flag: {other}");
                eprintln!("Usage: dpp-tool payload find <dmg> <pkg-path> <component> [-name <pattern>] [-type f|d|l]");
                process::exit(1);
            }
        }
        i += 1;
    }

    let archive = open_archive(&args[0], &args[1], &args[2])?;
    let entries = archive.list()?;

    let matches: Vec<_> = entries
        .iter()
        .filter(|e| {
            let np = normalize_cpio(&e.path);
            if np.is_empty() { return false; }

            if let Some(tf) = type_filter {
                match tf {
                    "f" => if e.is_dir || e.is_symlink { return false; }
                    "d" => if !e.is_dir { return false; }
                    "l" => if !e.is_symlink { return false; }
                    _ => {}
                }
            }

            if let Some(ref pattern) = name_pattern {
                if !glob_match(pattern, basename_of(&np)) {
                    return false;
                }
            }

            true
        })
        .collect();

    println!();
    if matches.is_empty() {
        println!("  {DIM}No matches found{RESET}");
    } else {
        for entry in &matches {
            let np = normalize_cpio(&entry.path);
            let (color, icon) = entry_style(entry);
            let size_str = if !entry.is_dir {
                format!("  {DIM}{}{RESET}", format_size(entry.size))
            } else {
                String::new()
            };
            println!(
                "  {DIM}{icon}{RESET} {color}/{np}{RESET}{size_str}",
            );
        }
        println!();
        println!("  {DIM}{} match(es){RESET}", matches.len());
    }
    println!();

    Ok(())
}

// ── cat ─────────────────────────────────────────────────────────────────

fn cat(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.len() < 4 {
        eprintln!("Usage: dpp-tool payload cat <dmg> <pkg-path> <component> <file>");
        process::exit(1);
    }

    let dmg_path = &args[0];
    let pkg_path = &args[1];
    let component = &args[2];
    let file_path = &args[3];

    let mut pipeline = dpp::DmgPipeline::open(dmg_path)?;
    let mut fs = pipeline.open_filesystem()?;
    let mut pkg = fs.open_pkg(pkg_path)?;

    let payload = pkg.payload(component)?;
    let archive = pbzx::Archive::from_reader(Cursor::new(payload))?;

    // Normalize the requested path to match CPIO conventions
    let normalized = normalize_user_path(file_path);
    // Try both with and without "./" prefix
    let data = archive.extract_file(&normalized)
        .or_else(|_| archive.extract_file(&format!("./{normalized}")))
        .or_else(|_| archive.extract_file(file_path))?;

    let mut stdout = io::stdout().lock();
    stdout.write_all(&data)?;

    Ok(())
}

// ── helpers ─────────────────────────────────────────────────────────────

fn entry_style(entry: &pbzx::FileEntry) -> (&str, &str) {
    if entry.is_dir {
        (BLUE, "dir")
    } else if entry.is_symlink {
        (CYAN, "lnk")
    } else {
        (WHITE, "   ")
    }
}

fn symlink_suffix(entry: &pbzx::FileEntry) -> String {
    if let Some(ref target) = entry.link_target {
        format!(" {DIM}-> {target}{RESET}")
    } else {
        String::new()
    }
}
