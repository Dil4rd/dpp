use std::io;
use std::process;
use std::time::Instant;

use crate::style::*;
use crate::pipeline::{open_pipeline, open_apfs};

pub(crate) fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.is_empty() {
        print_usage();
        process::exit(1);
    }
    match args[0].as_str() {
        "info" => info(&args[1..]),
        "ls" => ls(&args[1..]),
        "tree" => tree(&args[1..]),
        "cat" => cat(&args[1..]),
        "stat" => stat(&args[1..]),
        "find" => find(&args[1..]),
        "-h" | "--help" | "help" => { print_usage(); Ok(()) }
        _ => {
            eprintln!("{RED}Unknown apfs command: {}{RESET}", args[0]);
            print_usage();
            process::exit(1);
        }
    }
}

fn print_usage() {
    eprintln!(
        r#"
{BOLD}dpp-tool apfs{RESET} — APFS filesystem commands

{BOLD}COMMANDS:{RESET}
    {GREEN}info{RESET}   <dmg>                                  Volume info
    {GREEN}ls{RESET}     <dmg> <path>                           List directory contents
    {GREEN}tree{RESET}   <dmg> [path]                           Browse filesystem tree
    {GREEN}cat{RESET}    <dmg> <path>                           Extract file to stdout
    {GREEN}stat{RESET}   <dmg> <path>                           File metadata (OID, perms, dates)
    {GREEN}find{RESET}   <dmg> [-name pat] [-type f|d|l]        Find files (default: *.pkg)
"#
    );
}

fn info(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.is_empty() {
        eprintln!("Usage: dpp-tool apfs info <dmg-file>");
        process::exit(1);
    }

    let dmg_path = &args[0];
    let mut pipeline = open_pipeline(dmg_path)?;
    let apfs = open_apfs(&mut pipeline)?;
    let vi = apfs.volume_info();

    header(&format!("APFS Volume: {dmg_path}"));

    section("Volume Info");
    kv("Name", &vi.name);
    kv("Block size", &format!("{} bytes", vi.block_size));
    kv_highlight("Files", &format_commas(vi.num_files));
    kv_highlight("Directories", &format_commas(vi.num_directories));
    kv("Symlinks", &format_commas(vi.num_symlinks));
    println!();

    Ok(())
}

fn ls(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.len() < 2 {
        eprintln!("Usage: dpp-tool apfs ls <dmg-file> <path>");
        process::exit(1);
    }

    let dmg_path = &args[0];
    let path = &args[1];

    let mut pipeline = open_pipeline(dmg_path)?;
    let mut apfs = open_apfs(&mut pipeline)?;

    let mut entries = apfs.list_directory(path)?;
    entries.sort_by(|a, b| {
        let a_dir = a.kind == apfs::EntryKind::Directory;
        let b_dir = b.kind == apfs::EntryKind::Directory;
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
        let color = apfs_kind_color(entry.kind);
        let icon = apfs_kind_icon(entry.kind);
        let size_str = if entry.kind == apfs::EntryKind::File {
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
    let file_count = entries.iter().filter(|e| e.kind == apfs::EntryKind::File).count();
    let dir_count = entries.iter().filter(|e| e.kind == apfs::EntryKind::Directory).count();
    println!(
        "  {DIM}{} file(s), {} directory(ies){RESET}",
        file_count, dir_count
    );
    println!();

    Ok(())
}

fn tree(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.is_empty() {
        eprintln!("Usage: dpp-tool apfs tree <dmg-file> [path]");
        process::exit(1);
    }

    let dmg_path = &args[0];
    let base_path = if args.len() > 1 { &args[1] } else { "/" };

    let mut pipeline = open_pipeline(dmg_path)?;
    let mut apfs = open_apfs(&mut pipeline)?;

    header(&format!("Tree: {dmg_path}:{base_path}"));
    println!();

    print_tree(&mut apfs, base_path, "", 0, 3)?;
    println!();

    Ok(())
}

fn print_tree(
    apfs: &mut dpp::ApfsHandle,
    path: &str,
    prefix: &str,
    depth: usize,
    max_depth: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    if depth > max_depth {
        println!("  {prefix}{DIM}{TEE} ...{RESET}");
        return Ok(());
    }

    let mut entries = apfs.list_directory(path)?;
    entries.sort_by(|a, b| {
        let a_dir = a.kind == apfs::EntryKind::Directory;
        let b_dir = b.kind == apfs::EntryKind::Directory;
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

        let color = apfs_kind_color(entry.kind);
        let size_str = if entry.kind == apfs::EntryKind::File {
            format!("  {DIM}{}{RESET}", format_size(entry.size))
        } else {
            String::new()
        };

        println!(
            "  {prefix}{DIM}{connector}{RESET} {color}{BOLD}{}{RESET}{size_str}",
            entry.name
        );

        if entry.kind == apfs::EntryKind::Directory && !entry.name.starts_with('.') {
            let child_path = if path == "/" {
                format!("/{}", entry.name)
            } else {
                format!("{}/{}", path, entry.name)
            };
            print_tree(apfs, &child_path, &child_prefix, depth + 1, max_depth)?;
        }
    }

    Ok(())
}

fn cat(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.len() < 2 {
        eprintln!("Usage: dpp-tool apfs cat <dmg-file> <path>");
        process::exit(1);
    }

    let dmg_path = &args[0];
    let path = &args[1];

    let mut pipeline = dpp::DmgPipeline::open(dmg_path)?;
    let mut apfs = pipeline.open_apfs()?;

    let mut stdout = io::stdout().lock();
    apfs.read_file_to(path, &mut stdout)?;

    Ok(())
}

fn stat(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.len() < 2 {
        eprintln!("Usage: dpp-tool apfs stat <dmg-file> <path>");
        process::exit(1);
    }

    let dmg_path = &args[0];
    let path = &args[1];

    let mut pipeline = open_pipeline(dmg_path)?;
    let mut apfs = open_apfs(&mut pipeline)?;

    let stat = apfs.stat(path)?;

    header(&format!("stat: {path}"));

    section("Metadata");
    kv("OID", &stat.oid.to_string());
    kv("Kind", &format!("{:?}", stat.kind));
    kv("Size", &format!("{} ({})", format_size(stat.size), format_commas(stat.size)));
    kv("Permissions", &mode_string(stat.mode));
    kv("Owner", &format!("{}:{}", stat.uid, stat.gid));
    kv("Links", &stat.nlink.to_string());
    kv("Created", &format!("{} {DIM}(APFS nanosecond timestamp){RESET}", stat.create_time));
    kv("Modified", &format!("{} {DIM}(APFS nanosecond timestamp){RESET}", stat.modify_time));
    println!();

    Ok(())
}

fn find(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.is_empty() {
        eprintln!("Usage: dpp-tool apfs find <dmg> [-name <pattern>] [-type f|d|l]");
        eprintln!("       Default (no flags): -name \"*.pkg\" -type f");
        process::exit(1);
    }

    let dmg_path = &args[0];
    let mut name_pattern: Option<String> = None;
    let mut type_filter: Option<apfs::EntryKind> = None;

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
                    "f" => apfs::EntryKind::File,
                    "d" => apfs::EntryKind::Directory,
                    "l" => apfs::EntryKind::Symlink,
                    other => {
                        eprintln!("{RED}error:{RESET} unknown type '{other}' (use f, d, or l)");
                        process::exit(1);
                    }
                });
            }
            other => {
                eprintln!("{RED}error:{RESET} unknown flag: {other}");
                eprintln!("Usage: dpp-tool apfs find <dmg> [-name <pattern>] [-type f|d|l]");
                process::exit(1);
            }
        }
        i += 1;
    }

    // Default: find *.pkg files
    if name_pattern.is_none() && type_filter.is_none() {
        name_pattern = Some("*.pkg".to_string());
        type_filter = Some(apfs::EntryKind::File);
    }

    let mut pipeline = open_pipeline(dmg_path)?;
    let mut apfs = open_apfs(&mut pipeline)?;

    spinner_msg("Walking filesystem");
    let t = Instant::now();
    let entries = apfs.walk()?;
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
            let color = apfs_kind_color(entry.entry.kind);
            let size_str = if entry.entry.kind == apfs::EntryKind::File {
                format!("  {DIM}{}{RESET}", format_size(entry.entry.size))
            } else {
                String::new()
            };
            println!(
                "  {DIM}{}{RESET} {color}{}{RESET}{size_str}",
                apfs_kind_icon(entry.entry.kind),
                entry.path,
            );
        }
        println!();
        println!("  {DIM}{} match(es){RESET}", matches.len());
    }
    println!();

    Ok(())
}
