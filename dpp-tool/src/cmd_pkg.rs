use std::io;
use std::process;
use std::time::Instant;

use crate::style::*;
use crate::pipeline::{open_pipeline, open_hfs};

/// Build a sort key that produces depth-first tree order (dirs before files at each level).
/// Each path component is prefixed with '\x00' for directories or '\x01' for files,
/// so directories sort before sibling files and a directory is immediately followed
/// by its descendants.
fn tree_sort_key(path: &str, is_dir: bool) -> String {
    let parts: Vec<&str> = path.split('/').collect();
    let mut key_parts = Vec::with_capacity(parts.len());
    for (i, part) in parts.iter().enumerate() {
        let is_last = i == parts.len() - 1;
        let prefix = if !is_last || is_dir { "\x00" } else { "\x01" };
        key_parts.push(format!("{prefix}{part}"));
    }
    key_parts.join("/")
}

pub(crate) fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.is_empty() {
        print_usage();
        process::exit(1);
    }
    match args[0].as_str() {
        "info" => info(&args[1..]),
        "ls" => ls(&args[1..]),
        "find" => find(&args[1..]),
        "cat" => cat(&args[1..]),
        "-h" | "--help" | "help" => { print_usage(); Ok(()) }
        _ => {
            eprintln!("{RED}Unknown pkg command: {}{RESET}", args[0]);
            print_usage();
            process::exit(1);
        }
    }
}

fn print_usage() {
    eprintln!(
        r#"
{BOLD}dpp-tool pkg{RESET} — PKG/XAR archive commands

{BOLD}COMMANDS:{RESET}
    {GREEN}info{RESET}   <dmg> <pkg-path>                              Package stats
    {GREEN}ls{RESET}     <dmg> <pkg-path>                              List XAR contents
    {GREEN}find{RESET}   <dmg> <pkg-path> [-name pat] [-type f|d|l]    Find entries (default: *.pkg components)
    {GREEN}cat{RESET}    <dmg> <pkg-path> <file>                       XAR entry to stdout
"#
    );
}

fn info(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.len() < 2 {
        eprintln!("Usage: dpp-tool pkg info <dmg-file> <pkg-path>");
        process::exit(1);
    }

    let dmg_path = &args[0];
    let pkg_path = &args[1];

    let mut pipeline = open_pipeline(dmg_path)?;
    let mut hfs = open_hfs(&mut pipeline)?;

    spinner_msg(&format!("Opening {pkg_path}"));
    let t = Instant::now();
    let pkg = hfs.open_pkg(pkg_path)?;
    spinner_done(&format!(" ({})", format_duration(t.elapsed())));

    header(&format!("PKG: {pkg_path}"));

    section("Package");
    let pkg_type = if pkg.is_product_package() {
        format!("{GREEN}Product package{RESET} {DIM}(multi-component){RESET}")
    } else {
        format!("{YELLOW}Component package{RESET} {DIM}(single){RESET}")
    };
    kv("Type", &pkg_type);
    kv("Components", &pkg.components().len().to_string());

    let xar_header = pkg.xar().header();
    kv("XAR version", &xar_header.version.to_string());
    kv("Checksum", &format!("{:?}", xar_header.checksum_algo));
    kv("TOC size", &format!(
        "{} compressed, {} uncompressed",
        format_size(xar_header.toc_compressed_len),
        format_size(xar_header.toc_uncompressed_len),
    ));

    let files = pkg.xar().files();
    let file_count = files.iter().filter(|f| f.file_type == xara::XarFileType::File).count();
    let dir_count = files.iter().filter(|f| f.file_type == xara::XarFileType::Directory).count();
    let symlink_count = files.iter().filter(|f| f.file_type == xara::XarFileType::Symlink).count();

    let total_compressed: u64 = files.iter()
        .filter_map(|f| f.data.as_ref())
        .map(|d| d.length)
        .sum();
    let total_uncompressed: u64 = files.iter()
        .filter_map(|f| f.data.as_ref())
        .map(|d| d.size)
        .sum();

    section("Contents");
    kv("Entries", &files.len().to_string());
    kv("Files", &file_count.to_string());
    kv("Directories", &dir_count.to_string());
    if symlink_count > 0 {
        kv("Symlinks", &symlink_count.to_string());
    }
    kv("Compressed size", &format_size(total_compressed));
    kv("Uncompressed size", &format_size(total_uncompressed));
    if total_uncompressed > 0 {
        let savings = (1.0 - total_compressed as f64 / total_uncompressed as f64) * 100.0;
        kv_highlight("Space savings", &format!("{:.1}%", savings));
    }

    // Component summary
    let components = pkg.components();
    if !components.is_empty() {
        section("Components");
        for (i, comp) in components.iter().enumerate() {
            let connector = if i == components.len() - 1 { ELBOW } else { TEE };
            let name = if comp.is_empty() {
                format!("{DIM}(root){RESET}")
            } else {
                format!("{BOLD}{comp}{RESET}")
            };

            let payload_path = if comp.is_empty() {
                "Payload".to_string()
            } else {
                format!("{comp}/Payload")
            };
            let payload_info = if let Some(payload_file) = pkg.xar().find(&payload_path) {
                if let Some(data) = &payload_file.data {
                    format!("  {DIM}{} compressed, {} uncompressed{RESET}", format_size(data.length), format_size(data.size))
                } else {
                    String::new()
                }
            } else {
                String::new()
            };

            println!("  {DIM}{connector}{RESET} {name}{payload_info}");
        }
    }
    println!();

    Ok(())
}

fn ls(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.len() < 2 {
        eprintln!("Usage: dpp-tool pkg ls <dmg-file> <pkg-path>");
        process::exit(1);
    }

    let dmg_path = &args[0];
    let pkg_path = &args[1];

    let mut pipeline = open_pipeline(dmg_path)?;
    let mut hfs = open_hfs(&mut pipeline)?;

    spinner_msg(&format!("Opening {pkg_path}"));
    let t = Instant::now();
    let pkg = hfs.open_pkg(pkg_path)?;
    spinner_done(&format!(" ({})", format_duration(t.elapsed())));

    header(&format!("PKG: {pkg_path}"));

    section("XAR Contents");
    let files = pkg.xar().files();
    let mut sorted: Vec<_> = files.iter().collect();
    // Depth-first tree order: directories before files at each level,
    // with a directory immediately followed by its children.
    sorted.sort_by(|a, b| {
        let a_is_dir = a.file_type == xara::XarFileType::Directory;
        let b_is_dir = b.file_type == xara::XarFileType::Directory;
        let a_key = tree_sort_key(&a.path, a_is_dir);
        let b_key = tree_sort_key(&b.path, b_is_dir);
        a_key.cmp(&b_key)
    });
    for file in &sorted {
        let size_str = match &file.data {
            Some(d) => format_size(d.size),
            None => format!("{DIM}dir{RESET}"),
        };
        let type_color = match file.file_type {
            xara::XarFileType::Directory => BLUE,
            xara::XarFileType::Symlink => CYAN,
            xara::XarFileType::File => "",
        };
        println!(
            "  {type_color}{:<50}{RESET} {:>12}",
            file.path,
            size_str
        );
    }

    println!();
    let file_count = sorted.iter().filter(|f| f.file_type == xara::XarFileType::File).count();
    let dir_count = sorted.iter().filter(|f| f.file_type == xara::XarFileType::Directory).count();
    println!(
        "  {DIM}{} file(s), {} directory(ies){RESET}",
        file_count, dir_count
    );
    println!();

    Ok(())
}

fn find(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.len() < 2 {
        eprintln!("Usage: dpp-tool pkg find <dmg> <pkg-path> [-name <pattern>] [-type f|d|l]");
        process::exit(1);
    }

    let dmg_path = &args[0];
    let pkg_path = &args[1];
    let mut name_pattern: Option<String> = None;
    let mut type_filter: Option<xara::XarFileType> = None;

    let mut i = 2;
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
                    "f" => xara::XarFileType::File,
                    "d" => xara::XarFileType::Directory,
                    "l" => xara::XarFileType::Symlink,
                    other => {
                        eprintln!("{RED}error:{RESET} unknown type '{other}' (use f, d, or l)");
                        process::exit(1);
                    }
                });
            }
            other => {
                eprintln!("{RED}error:{RESET} unknown flag: {other}");
                eprintln!("Usage: dpp-tool pkg find <dmg> <pkg-path> [-name <pattern>] [-type f|d|l]");
                process::exit(1);
            }
        }
        i += 1;
    }

    // Default: find components (*.pkg directories)
    if name_pattern.is_none() && type_filter.is_none() {
        name_pattern = Some("*.pkg".to_string());
        type_filter = Some(xara::XarFileType::Directory);
    }

    let mut pipeline = open_pipeline(dmg_path)?;
    let mut hfs = open_hfs(&mut pipeline)?;

    spinner_msg(&format!("Opening {pkg_path}"));
    let t = Instant::now();
    let pkg = hfs.open_pkg(pkg_path)?;
    spinner_done(&format!(" ({})", format_duration(t.elapsed())));

    let files = pkg.xar().files();
    let matches: Vec<_> = files
        .iter()
        .filter(|f| {
            if let Some(ref kind) = type_filter {
                if f.file_type != *kind {
                    return false;
                }
            }
            if let Some(ref pattern) = name_pattern {
                if !glob_match(pattern, &f.name) {
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
        for file in &matches {
            let type_color = match file.file_type {
                xara::XarFileType::Directory => BLUE,
                xara::XarFileType::Symlink => CYAN,
                xara::XarFileType::File => WHITE,
            };
            let icon = match file.file_type {
                xara::XarFileType::Directory => "dir",
                xara::XarFileType::File => "   ",
                xara::XarFileType::Symlink => "lnk",
            };
            let size_str = match &file.data {
                Some(d) => format!("  {DIM}{}{RESET}", format_size(d.size)),
                None => String::new(),
            };
            println!(
                "  {DIM}{icon}{RESET} {type_color}{}{RESET}{size_str}",
                file.path,
            );
        }
        println!();
        println!("  {DIM}{} match(es){RESET}", matches.len());
    }
    println!();

    Ok(())
}

fn cat(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.len() < 3 {
        eprintln!("Usage: dpp-tool pkg cat <dmg-file> <pkg-path> <file>");
        process::exit(1);
    }

    let dmg_path = &args[0];
    let pkg_path = &args[1];
    let file_path = &args[2];

    let mut pipeline = dpp::DmgPipeline::open(dmg_path)?;
    let mut hfs = pipeline.open_hfs()?;
    let mut pkg = hfs.open_pkg(pkg_path)?;

    let xar_file = pkg.xar().find(file_path)
        .ok_or_else(|| format!("File not found in XAR: {file_path}"))?
        .clone();

    let mut stdout = io::stdout().lock();
    pkg.xar_mut().read_file_to(&xar_file, &mut stdout)?;

    Ok(())
}
