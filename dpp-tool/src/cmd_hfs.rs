use std::io;
use std::process;
use std::time::Instant;

use crate::style::*;
use crate::pipeline::{open_pipeline, open_hfs};

pub(crate) fn run(args: &[String], mode: dpp::ExtractMode) -> Result<(), Box<dyn std::error::Error>> {
    if args.is_empty() {
        print_usage();
        process::exit(1);
    }
    match args[0].as_str() {
        "info" => info(&args[1..], mode),
        "ls" => ls(&args[1..], mode),
        "tree" => tree(&args[1..], mode),
        "cat" => cat(&args[1..], mode),
        "stat" => stat(&args[1..], mode),
        "find" => find(&args[1..], mode),
        "-h" | "--help" | "help" => { print_usage(); Ok(()) }
        _ => {
            eprintln!("{RED}Unknown hfs command: {}{RESET}", args[0]);
            print_usage();
            process::exit(1);
        }
    }
}

fn print_usage() {
    eprintln!(
        r#"
{BOLD}dpp-tool hfs{RESET} — HFS+ filesystem commands

{BOLD}COMMANDS:{RESET}
    {GREEN}info{RESET}   <dmg>                                  Volume header
    {GREEN}ls{RESET}     <dmg> <path>                           List directory contents
    {GREEN}tree{RESET}   <dmg> [path]                           Browse filesystem tree
    {GREEN}cat{RESET}    <dmg> <path>                           Extract file to stdout
    {GREEN}stat{RESET}   <dmg> <path>                           File metadata (CNID, perms, dates, forks)
    {GREEN}find{RESET}   <dmg> [-name pat] [-type f|d|l]        Find files (default: *.pkg)
"#
    );
}

fn info(args: &[String], mode: dpp::ExtractMode) -> Result<(), Box<dyn std::error::Error>> {
    if args.is_empty() {
        eprintln!("Usage: dpp-tool hfs info <dmg-file>");
        process::exit(1);
    }

    let dmg_path = &args[0];
    let mut pipeline = open_pipeline(dmg_path)?;
    let hfs = open_hfs(&mut pipeline, mode)?;
    let vh = hfs.volume_header();

    header(&format!("HFS+ Volume: {dmg_path}"));

    section("Volume Header");
    let sig = if vh.is_hfsx {
        format!("HFSX {DIM}(case-sensitive){RESET}")
    } else {
        format!("HFS+ {DIM}(case-insensitive){RESET}")
    };
    kv("Signature", &sig);
    kv("Version", &vh.version.to_string());
    kv("Block size", &format!("{} bytes", vh.block_size));
    kv("Total blocks", &format_commas(vh.total_blocks as u64));
    kv("Free blocks", &format_commas(vh.free_blocks as u64));
    kv_highlight("Files", &format_commas(vh.file_count as u64));
    kv_highlight("Folders", &format_commas(vh.folder_count as u64));
    println!();

    Ok(())
}

fn ls(args: &[String], mode: dpp::ExtractMode) -> Result<(), Box<dyn std::error::Error>> {
    if args.len() < 2 {
        eprintln!("Usage: dpp-tool hfs ls <dmg-file> <path>");
        process::exit(1);
    }

    let dmg_path = &args[0];
    let path = &args[1];

    let mut pipeline = open_pipeline(dmg_path)?;
    let mut hfs = open_hfs(&mut pipeline, mode)?;

    let mut entries = hfs.list_directory(path)?;
    entries.sort_by(|a, b| {
        let a_dir = a.kind == hfsplus::EntryKind::Directory;
        let b_dir = b.kind == hfsplus::EntryKind::Directory;
        b_dir.cmp(&a_dir).then(a.name.cmp(&b.name))
    });

    header(&format!("{dmg_path}:{path}"));
    println!();
    println!(
        "  {DIM}{:<5} {:>12}  {}{RESET}",
        "Kind", "Size", "Name"
    );
    println!("  {DIM}{}{RESET}", "-".repeat(56));

    for entry in &entries {
        let color = kind_color(entry.kind);
        let icon = kind_icon(entry.kind);
        let size_str = if entry.kind == hfsplus::EntryKind::File {
            format_size(entry.size)
        } else {
            "-".to_string()
        };

        println!(
            "  {DIM}{icon}{RESET}   {:>12}  {color}{}{RESET}",
            size_str, entry.name
        );
    }

    println!();
    let file_count = entries.iter().filter(|e| e.kind == hfsplus::EntryKind::File).count();
    let dir_count = entries.iter().filter(|e| e.kind == hfsplus::EntryKind::Directory).count();
    println!(
        "  {DIM}{} file(s), {} directory(ies){RESET}",
        file_count, dir_count
    );
    println!();

    Ok(())
}

fn tree(args: &[String], mode: dpp::ExtractMode) -> Result<(), Box<dyn std::error::Error>> {
    if args.is_empty() {
        eprintln!("Usage: dpp-tool hfs tree <dmg-file> [path]");
        process::exit(1);
    }

    let dmg_path = &args[0];
    let base_path = if args.len() > 1 { &args[1] } else { "/" };

    let mut pipeline = open_pipeline(dmg_path)?;
    let mut hfs = open_hfs(&mut pipeline, mode)?;

    header(&format!("Tree: {dmg_path}:{base_path}"));
    println!();

    print_tree(&mut hfs, base_path, "", 0, 3)?;
    println!();

    Ok(())
}

fn print_tree(
    hfs: &mut dpp::HfsHandle,
    path: &str,
    prefix: &str,
    depth: usize,
    max_depth: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    if depth > max_depth {
        println!("  {prefix}{DIM}{TEE} ...{RESET}");
        return Ok(());
    }

    let mut entries = hfs.list_directory(path)?;
    // Sort: directories first, then by name
    entries.sort_by(|a, b| {
        let a_dir = a.kind == hfsplus::EntryKind::Directory;
        let b_dir = b.kind == hfsplus::EntryKind::Directory;
        b_dir.cmp(&a_dir).then(a.name.cmp(&b.name))
    });

    for (i, entry) in entries.iter().enumerate() {
        let is_last = i == entries.len() - 1;
        let connector = if is_last { ELBOW } else { TEE };
        let child_prefix = if is_last {
            format!("{prefix}    ")
        } else {
            format!("{prefix}{PIPE}   ")
        };

        let color = kind_color(entry.kind);
        let size_str = if entry.kind == hfsplus::EntryKind::File {
            format!("  {DIM}{}{RESET}", format_size(entry.size))
        } else {
            String::new()
        };

        println!(
            "  {prefix}{DIM}{connector}{RESET} {color}{BOLD}{}{RESET}{size_str}",
            entry.name
        );

        if entry.kind == hfsplus::EntryKind::Directory && !entry.name.starts_with('.') {
            let child_path = if path == "/" {
                format!("/{}", entry.name)
            } else {
                format!("{}/{}", path, entry.name)
            };
            print_tree(hfs, &child_path, &child_prefix, depth + 1, max_depth)?;
        }
    }

    Ok(())
}

fn cat(args: &[String], mode: dpp::ExtractMode) -> Result<(), Box<dyn std::error::Error>> {
    if args.len() < 2 {
        eprintln!("Usage: dpp-tool hfs cat <dmg-file> <path>");
        process::exit(1);
    }

    let dmg_path = &args[0];
    let path = &args[1];

    let mut pipeline = dpp::DmgPipeline::open(dmg_path)?;
    let mut hfs = pipeline.open_hfs_with_mode(mode)?;

    let mut stdout = io::stdout().lock();
    hfs.read_file_to(path, &mut stdout)?;

    Ok(())
}

fn stat(args: &[String], mode: dpp::ExtractMode) -> Result<(), Box<dyn std::error::Error>> {
    if args.len() < 2 {
        eprintln!("Usage: dpp-tool hfs stat <dmg-file> <path>");
        process::exit(1);
    }

    let dmg_path = &args[0];
    let path = &args[1];

    let mut pipeline = open_pipeline(dmg_path)?;
    let mut hfs = open_hfs(&mut pipeline, mode)?;

    let stat = hfs.stat(path)?;

    header(&format!("stat: {path}"));

    section("Metadata");
    kv("CNID", &stat.cnid.to_string());
    kv("Kind", &format!("{:?}", stat.kind));
    kv("Size", &format!("{} ({})", format_size(stat.size), format_commas(stat.size)));
    kv("Permissions", &mode_string(stat.permissions.mode));
    kv("Owner", &format!("{}:{}", stat.permissions.owner_id, stat.permissions.group_id));
    kv("Data fork extents", &stat.data_fork_extents.to_string());

    if stat.resource_fork_size > 0 {
        kv("Resource fork", &format_size(stat.resource_fork_size));
    }

    // Dates (HFS+ epoch: 1904-01-01, these are raw values)
    kv("Created", &format!("{} {DIM}(HFS+ timestamp){RESET}", stat.create_date));
    kv("Modified", &format!("{} {DIM}(HFS+ timestamp){RESET}", stat.modify_date));
    println!();

    Ok(())
}

fn find(args: &[String], mode: dpp::ExtractMode) -> Result<(), Box<dyn std::error::Error>> {
    if args.is_empty() {
        eprintln!("Usage: dpp-tool hfs find <dmg> [-name <pattern>] [-type f|d|l]");
        eprintln!("       Default (no flags): -name \"*.pkg\" -type f");
        process::exit(1);
    }

    let dmg_path = &args[0];
    let mut name_pattern: Option<String> = None;
    let mut type_filter: Option<hfsplus::EntryKind> = None;

    let mut i = 1;
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
                type_filter = Some(match args[i].as_str() {
                    "f" => hfsplus::EntryKind::File,
                    "d" => hfsplus::EntryKind::Directory,
                    "l" => hfsplus::EntryKind::Symlink,
                    other => {
                        eprintln!("{RED}error:{RESET} unknown type '{other}' (use f, d, or l)");
                        process::exit(1);
                    }
                });
            }
            other => {
                eprintln!("{RED}error:{RESET} unknown flag: {other}");
                eprintln!("Usage: dpp-tool hfs find <dmg> [-name <pattern>] [-type f|d|l]");
                process::exit(1);
            }
        }
        i += 1;
    }

    // Default: find *.pkg files
    if name_pattern.is_none() && type_filter.is_none() {
        name_pattern = Some("*.pkg".to_string());
        type_filter = Some(hfsplus::EntryKind::File);
    }

    let mut pipeline = open_pipeline(dmg_path)?;
    let mut hfs = open_hfs(&mut pipeline, mode)?;

    spinner_msg("Walking filesystem");
    let t = Instant::now();
    let entries = hfs.walk()?;
    spinner_done(&format!(" ({})", format_duration(t.elapsed())));

    let matches: Vec<_> = entries
        .iter()
        .filter(|e| {
            if let Some(ref kind) = type_filter {
                if e.entry.kind != *kind {
                    return false;
                }
            }
            if let Some(ref pattern) = name_pattern {
                let basename = e.path.rsplit('/').next().unwrap_or(&e.path);
                if !glob_match(pattern, basename) {
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
            let color = kind_color(entry.entry.kind);
            let size_str = if entry.entry.kind == hfsplus::EntryKind::File {
                format!("  {DIM}{}{RESET}", format_size(entry.entry.size))
            } else {
                String::new()
            };
            println!(
                "  {DIM}{}{RESET} {color}{}{RESET}{size_str}",
                kind_icon(entry.entry.kind),
                entry.path,
            );
        }
        println!();
        println!("  {DIM}{} match(es){RESET}", matches.len());
    }
    println!();

    Ok(())
}
