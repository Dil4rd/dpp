use std::io;
use std::path::Path;
use std::time::Instant;

use crate::pipeline::{open_apfs, open_pipeline};
use crate::style::*;
use crate::{ApfsCommand, FindArgs};

pub(crate) fn run(
    cmd: ApfsCommand,
    mode: dpp::ExtractMode,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        ApfsCommand::Info { dmg } => info(&dmg, mode),
        ApfsCommand::Ls { dmg, path } => ls(&dmg, &path, mode),
        ApfsCommand::Tree { dmg, path, depth } => tree(&dmg, path.as_deref(), depth, mode),
        ApfsCommand::Cat { dmg, path } => cat(&dmg, &path, mode),
        ApfsCommand::Stat { dmg, path } => stat(&dmg, &path, mode),
        ApfsCommand::Find { dmg, args } => find(&dmg, args, mode),
    }
}

fn info(dmg_path: &Path, mode: dpp::ExtractMode) -> Result<(), Box<dyn std::error::Error>> {
    let dmg_str = dmg_path.display();
    let mut pipeline = open_pipeline(dmg_path)?;
    let apfs = open_apfs(&mut pipeline, mode)?;
    let vi = apfs.volume_info();

    header(&format!("APFS Volume: {dmg_str}"));

    section("Volume Info");
    kv("Name", &vi.name);
    kv("Block size", &format!("{} bytes", vi.block_size));
    kv_highlight("Files", &format_commas(vi.num_files));
    kv_highlight("Directories", &format_commas(vi.num_directories));
    kv("Symlinks", &format_commas(vi.num_symlinks));
    println!();

    Ok(())
}

fn ls(
    dmg_path: &Path,
    path: &str,
    mode: dpp::ExtractMode,
) -> Result<(), Box<dyn std::error::Error>> {
    let dmg_str = dmg_path.display();
    let mut pipeline = open_pipeline(dmg_path)?;
    let mut apfs = open_apfs(&mut pipeline, mode)?;

    let mut entries = apfs.list_directory(path)?;
    entries.sort_by(|a, b| {
        let a_dir = a.kind == apfs::EntryKind::Directory;
        let b_dir = b.kind == apfs::EntryKind::Directory;
        b_dir.cmp(&a_dir).then(a.name.cmp(&b.name))
    });

    let (d, r) = (dim(), reset());
    header(&format!("{dmg_str}:{path}"));
    println!();
    println!("  {d}{:<5} {:>12}  Name{r}", "Kind", "Size");
    println!("  {d}{}{r}", "-".repeat(56));

    for entry in &entries {
        let color = apfs_kind_color(entry.kind);
        let icon = apfs_kind_icon(entry.kind);
        let size_str = if entry.kind == apfs::EntryKind::File {
            format_size(entry.size)
        } else {
            "-".to_string()
        };

        println!(
            "  {d}{icon}{r}   {:>12}  {color}{}{r}",
            size_str, entry.name
        );
    }

    println!();
    let file_count = entries
        .iter()
        .filter(|e| e.kind == apfs::EntryKind::File)
        .count();
    let dir_count = entries
        .iter()
        .filter(|e| e.kind == apfs::EntryKind::Directory)
        .count();
    println!(
        "  {d}{} file(s), {} directory(ies){r}",
        file_count, dir_count
    );
    println!();

    Ok(())
}

fn tree(
    dmg_path: &Path,
    path: Option<&str>,
    max_depth: usize,
    mode: dpp::ExtractMode,
) -> Result<(), Box<dyn std::error::Error>> {
    let dmg_str = dmg_path.display();
    let base_path = path.unwrap_or("/");

    let mut pipeline = open_pipeline(dmg_path)?;
    let mut apfs = open_apfs(&mut pipeline, mode)?;

    header(&format!("Tree: {dmg_str}:{base_path}"));
    println!();

    print_tree(&mut apfs, base_path, "", 0, max_depth)?;
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
    let (d, r, b) = (dim(), reset(), bold());

    if depth > max_depth {
        println!("  {prefix}{d}{TEE} ...{r}");
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
            format!("  {d}{}{r}", format_size(entry.size))
        } else {
            String::new()
        };

        println!(
            "  {prefix}{d}{connector}{r} {color}{b}{}{r}{size_str}",
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

fn cat(
    dmg_path: &Path,
    path: &str,
    mode: dpp::ExtractMode,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut pipeline = dpp::DmgPipeline::open(dmg_path)?;
    let mut apfs = pipeline.open_apfs_with_mode(mode)?;

    let mut stdout = io::stdout().lock();
    apfs.read_file_to(path, &mut stdout)?;

    Ok(())
}

fn stat(
    dmg_path: &Path,
    path: &str,
    mode: dpp::ExtractMode,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut pipeline = open_pipeline(dmg_path)?;
    let mut apfs = open_apfs(&mut pipeline, mode)?;

    let stat = apfs.stat(path)?;

    let (d, r) = (dim(), reset());
    header(&format!("stat: {path}"));

    section("Metadata");
    kv("OID", &stat.oid.to_string());
    kv("Kind", &format!("{:?}", stat.kind));
    kv(
        "Size",
        &format!("{} ({})", format_size(stat.size), format_commas(stat.size)),
    );
    kv("Permissions", &mode_string(stat.mode));
    kv("Owner", &format!("{}:{}", stat.uid, stat.gid));
    kv("Links", &stat.nlink.to_string());
    kv(
        "Created",
        &format!("{} {d}(APFS nanosecond timestamp){r}", stat.create_time),
    );
    kv(
        "Modified",
        &format!("{} {d}(APFS nanosecond timestamp){r}", stat.modify_time),
    );
    println!();

    Ok(())
}

fn find(
    dmg_path: &Path,
    args: FindArgs,
    mode: dpp::ExtractMode,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut name_pattern = args.name;
    let mut type_filter: Option<apfs::EntryKind> = match args.file_type {
        None => None,
        Some('f') => Some(apfs::EntryKind::File),
        Some('d') => Some(apfs::EntryKind::Directory),
        Some('l') => Some(apfs::EntryKind::Symlink),
        Some(other) => return Err(format!("unknown type '{other}' (use f, d, or l)").into()),
    };

    // Default: find *.pkg files
    if name_pattern.is_none() && type_filter.is_none() {
        name_pattern = Some("*.pkg".to_string());
        type_filter = Some(apfs::EntryKind::File);
    }

    let mut pipeline = open_pipeline(dmg_path)?;
    let mut apfs = open_apfs(&mut pipeline, mode)?;

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

    let (d, r) = (dim(), reset());
    println!();
    if matches.is_empty() {
        println!("  {d}No matches found{r}");
    } else {
        for entry in &matches {
            let color = apfs_kind_color(entry.entry.kind);
            let size_str = if entry.entry.kind == apfs::EntryKind::File {
                format!("  {d}{}{r}", format_size(entry.entry.size))
            } else {
                String::new()
            };
            println!(
                "  {d}{}{r} {color}{}{r}{size_str}",
                apfs_kind_icon(entry.entry.kind),
                entry.path,
            );
        }
        println!();
        println!("  {d}{} match(es){r}", matches.len());
    }
    println!();

    Ok(())
}
