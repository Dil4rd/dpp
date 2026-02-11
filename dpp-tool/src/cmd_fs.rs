use std::io;
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
        "cat" => cat(&args[1..]),
        "stat" => stat(&args[1..]),
        "find" => find(&args[1..]),
        "-h" | "--help" | "help" => { print_usage(); Ok(()) }
        _ => {
            eprintln!("{RED}Unknown fs command: {}{RESET}", args[0]);
            print_usage();
            process::exit(1);
        }
    }
}

fn print_usage() {
    eprintln!(
        r#"
{BOLD}dpp-tool fs{RESET} — Auto-detecting filesystem commands (HFS+ / APFS)

{BOLD}COMMANDS:{RESET}
    {GREEN}info{RESET}   <dmg>                                  Volume info (auto-detect)
    {GREEN}ls{RESET}     <dmg> <path>                           List directory contents
    {GREEN}tree{RESET}   <dmg> [path]                           Browse filesystem tree
    {GREEN}cat{RESET}    <dmg> <path>                           Extract file to stdout
    {GREEN}stat{RESET}   <dmg> <path>                           File metadata
    {GREEN}find{RESET}   <dmg> [-name pat] [-type f|d|l]        Find files (default: *.pkg)
"#
    );
}

fn info(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.is_empty() {
        eprintln!("Usage: dpp-tool fs info <dmg-file>");
        process::exit(1);
    }

    let dmg_path = &args[0];
    let mut pipeline = open_pipeline(dmg_path)?;
    let fs = open_filesystem(&mut pipeline)?;
    let vi = fs.volume_info();

    let type_label = match vi.fs_type {
        dpp::FsType::HfsPlus => "HFS+ Volume",
        dpp::FsType::Apfs => "APFS Volume",
    };
    header(&format!("{type_label}: {dmg_path}"));

    section("Volume Info");
    if let Some(ref name) = vi.name {
        kv("Name", name);
    }
    if let Some(is_hfsx) = vi.is_hfsx {
        let sig = if is_hfsx {
            format!("HFSX {DIM}(case-sensitive){RESET}")
        } else {
            format!("HFS+ {DIM}(case-insensitive){RESET}")
        };
        kv("Signature", &sig);
    }
    if let Some(version) = vi.version {
        kv("Version", &version.to_string());
    }
    kv("Block size", &format!("{} bytes", vi.block_size));
    if let Some(total) = vi.total_blocks {
        kv("Total blocks", &format_commas(total as u64));
    }
    if let Some(free) = vi.free_blocks {
        kv("Free blocks", &format_commas(free as u64));
    }
    kv_highlight("Files", &format_commas(vi.file_count));
    kv_highlight("Directories", &format_commas(vi.directory_count));
    if let Some(symlinks) = vi.symlink_count {
        kv("Symlinks", &format_commas(symlinks));
    }
    println!();

    Ok(())
}

fn ls(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.len() < 2 {
        eprintln!("Usage: dpp-tool fs ls <dmg-file> <path>");
        process::exit(1);
    }

    let dmg_path = &args[0];
    let path = &args[1];

    let mut pipeline = open_pipeline(dmg_path)?;
    let mut fs = open_filesystem(&mut pipeline)?;

    let mut entries = fs.list_directory(path)?;
    entries.sort_by(|a, b| {
        let a_dir = a.kind == dpp::FsEntryKind::Directory;
        let b_dir = b.kind == dpp::FsEntryKind::Directory;
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
        let color = fs_kind_color(entry.kind);
        let icon = fs_kind_icon(entry.kind);
        let size_str = if entry.kind == dpp::FsEntryKind::File {
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
    let file_count = entries.iter().filter(|e| e.kind == dpp::FsEntryKind::File).count();
    let dir_count = entries.iter().filter(|e| e.kind == dpp::FsEntryKind::Directory).count();
    println!(
        "  {DIM}{} file(s), {} directory(ies){RESET}",
        file_count, dir_count
    );
    println!();

    Ok(())
}

fn tree(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.is_empty() {
        eprintln!("Usage: dpp-tool fs tree <dmg-file> [path]");
        process::exit(1);
    }

    let dmg_path = &args[0];
    let base_path = if args.len() > 1 { &args[1] } else { "/" };

    let mut pipeline = open_pipeline(dmg_path)?;
    let mut fs = open_filesystem(&mut pipeline)?;

    header(&format!("Tree: {dmg_path}:{base_path}"));
    println!();

    print_tree(&mut fs, base_path, "", 0, 3)?;
    println!();

    Ok(())
}

fn print_tree(
    fs: &mut dpp::FilesystemHandle,
    path: &str,
    prefix: &str,
    depth: usize,
    max_depth: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    if depth > max_depth {
        println!("  {prefix}{DIM}{TEE} ...{RESET}");
        return Ok(());
    }

    let mut entries = fs.list_directory(path)?;
    entries.sort_by(|a, b| {
        let a_dir = a.kind == dpp::FsEntryKind::Directory;
        let b_dir = b.kind == dpp::FsEntryKind::Directory;
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

        let color = fs_kind_color(entry.kind);
        let size_str = if entry.kind == dpp::FsEntryKind::File {
            format!("  {DIM}{}{RESET}", format_size(entry.size))
        } else {
            String::new()
        };

        println!(
            "  {prefix}{DIM}{connector}{RESET} {color}{BOLD}{}{RESET}{size_str}",
            entry.name
        );

        if entry.kind == dpp::FsEntryKind::Directory && !entry.name.starts_with('.') {
            let child_path = if path == "/" {
                format!("/{}", entry.name)
            } else {
                format!("{}/{}", path, entry.name)
            };
            print_tree(fs, &child_path, &child_prefix, depth + 1, max_depth)?;
        }
    }

    Ok(())
}

fn cat(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.len() < 2 {
        eprintln!("Usage: dpp-tool fs cat <dmg-file> <path>");
        process::exit(1);
    }

    let dmg_path = &args[0];
    let path = &args[1];

    let mut pipeline = dpp::DmgPipeline::open(dmg_path)?;
    let mut fs = pipeline.open_filesystem()?;

    let mut stdout = io::stdout().lock();
    fs.read_file_to(path, &mut stdout)?;

    Ok(())
}

fn stat(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.len() < 2 {
        eprintln!("Usage: dpp-tool fs stat <dmg-file> <path>");
        process::exit(1);
    }

    let dmg_path = &args[0];
    let path = &args[1];

    let mut pipeline = open_pipeline(dmg_path)?;
    let mut fs = open_filesystem(&mut pipeline)?;

    let stat = fs.stat(path)?;

    header(&format!("stat: {path}"));

    section("Metadata");
    let id_label = match stat.fs_type {
        dpp::FsType::HfsPlus => "CNID",
        dpp::FsType::Apfs => "OID",
    };
    kv(id_label, &stat.id.to_string());
    kv("Kind", &format!("{:?}", stat.kind));
    kv("Size", &format!("{} ({})", format_size(stat.size), format_commas(stat.size)));
    kv("Permissions", &mode_string(stat.mode));
    kv("Owner", &format!("{}:{}", stat.uid, stat.gid));

    if let Some(nlink) = stat.nlink {
        kv("Links", &nlink.to_string());
    }
    if let Some(extents) = stat.data_fork_extents {
        kv("Data fork extents", &extents.to_string());
    }
    if let Some(rsrc_size) = stat.resource_fork_size {
        kv("Resource fork", &format_size(rsrc_size));
    }

    let time_label = match stat.fs_type {
        dpp::FsType::HfsPlus => "HFS+ timestamp",
        dpp::FsType::Apfs => "APFS nanosecond timestamp",
    };
    kv("Created", &format!("{} {DIM}({time_label}){RESET}", stat.create_time));
    kv("Modified", &format!("{} {DIM}({time_label}){RESET}", stat.modify_time));
    println!();

    Ok(())
}

fn find(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.is_empty() {
        eprintln!("Usage: dpp-tool fs find <dmg> [-name <pattern>] [-type f|d|l]");
        eprintln!("       Default (no flags): -name \"*.pkg\" -type f");
        process::exit(1);
    }

    let dmg_path = &args[0];
    let mut name_pattern: Option<String> = None;
    let mut type_filter: Option<dpp::FsEntryKind> = None;

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
                    "f" => dpp::FsEntryKind::File,
                    "d" => dpp::FsEntryKind::Directory,
                    "l" => dpp::FsEntryKind::Symlink,
                    other => {
                        eprintln!("{RED}error:{RESET} unknown type '{other}' (use f, d, or l)");
                        process::exit(1);
                    }
                });
            }
            other => {
                eprintln!("{RED}error:{RESET} unknown flag: {other}");
                eprintln!("Usage: dpp-tool fs find <dmg> [-name <pattern>] [-type f|d|l]");
                process::exit(1);
            }
        }
        i += 1;
    }

    // Default: find *.pkg files
    if name_pattern.is_none() && type_filter.is_none() {
        name_pattern = Some("*.pkg".to_string());
        type_filter = Some(dpp::FsEntryKind::File);
    }

    let mut pipeline = open_pipeline(dmg_path)?;
    let mut fs = open_filesystem(&mut pipeline)?;

    spinner_msg("Walking filesystem");
    let t = Instant::now();
    let entries = fs.walk()?;
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
            let color = fs_kind_color(entry.entry.kind);
            let size_str = if entry.entry.kind == dpp::FsEntryKind::File {
                format!("  {DIM}{}{RESET}", format_size(entry.entry.size))
            } else {
                String::new()
            };
            println!(
                "  {DIM}{}{RESET} {color}{}{RESET}{size_str}",
                fs_kind_icon(entry.entry.kind),
                entry.path,
            );
        }
        println!();
        println!("  {DIM}{} match(es){RESET}", matches.len());
    }
    println!();

    Ok(())
}
