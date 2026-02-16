use std::io;
use std::path::Path;
use std::time::Instant;

use crate::pipeline::{open_hfs, open_pipeline};
use crate::style::*;
use crate::{FindArgs, HfsCommand};

pub(crate) fn run(
    cmd: HfsCommand,
    mode: dpp::ExtractMode,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        HfsCommand::Info { dmg } => info(&dmg, mode),
        HfsCommand::Ls { dmg, path } => ls(&dmg, &path, mode),
        HfsCommand::Tree { dmg, path, depth } => tree(&dmg, path.as_deref(), depth, mode),
        HfsCommand::Cat { dmg, path } => cat(&dmg, &path, mode),
        HfsCommand::Stat { dmg, path } => stat(&dmg, &path, mode),
        HfsCommand::Find { dmg, args } => find(&dmg, args, mode),
    }
}

fn info(dmg_path: &Path, mode: dpp::ExtractMode) -> Result<(), Box<dyn std::error::Error>> {
    let dmg_str = dmg_path.display();
    let mut pipeline = open_pipeline(dmg_path)?;
    let hfs = open_hfs(&mut pipeline, mode)?;
    let vh = hfs.volume_header();

    let (d, r) = (dim(), reset());
    header(&format!("HFS+ Volume: {dmg_str}"));

    section("Volume Header");
    let sig = if vh.is_hfsx {
        format!("HFSX {d}(case-sensitive){r}")
    } else {
        format!("HFS+ {d}(case-insensitive){r}")
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

fn ls(
    dmg_path: &Path,
    path: &str,
    mode: dpp::ExtractMode,
) -> Result<(), Box<dyn std::error::Error>> {
    let dmg_str = dmg_path.display();
    let mut pipeline = open_pipeline(dmg_path)?;
    let mut hfs = open_hfs(&mut pipeline, mode)?;

    let mut entries = hfs.list_directory(path)?;
    entries.sort_by(|a, b| {
        let a_dir = a.kind == hfsplus::EntryKind::Directory;
        let b_dir = b.kind == hfsplus::EntryKind::Directory;
        b_dir.cmp(&a_dir).then(a.name.cmp(&b.name))
    });

    let (d, r) = (dim(), reset());
    header(&format!("{dmg_str}:{path}"));
    println!();
    println!("  {d}{:<5} {:>12}  Name{r}", "Kind", "Size");
    println!("  {d}{}{r}", "-".repeat(56));

    for entry in &entries {
        let color = kind_color(entry.kind);
        let icon = kind_icon(entry.kind);
        let size_str = if entry.kind == hfsplus::EntryKind::File {
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
        .filter(|e| e.kind == hfsplus::EntryKind::File)
        .count();
    let dir_count = entries
        .iter()
        .filter(|e| e.kind == hfsplus::EntryKind::Directory)
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
    let mut hfs = open_hfs(&mut pipeline, mode)?;

    header(&format!("Tree: {dmg_str}:{base_path}"));
    println!();

    print_tree(&mut hfs, base_path, "", 0, max_depth)?;
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
    let (d, r, b) = (dim(), reset(), bold());

    if depth > max_depth {
        println!("  {prefix}{d}{TEE} ...{r}");
        return Ok(());
    }

    let mut entries = hfs.list_directory(path)?;
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
            format!("  {d}{}{r}", format_size(entry.size))
        } else {
            String::new()
        };

        println!(
            "  {prefix}{d}{connector}{r} {color}{b}{}{r}{size_str}",
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

fn cat(
    dmg_path: &Path,
    path: &str,
    mode: dpp::ExtractMode,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut pipeline = dpp::DmgPipeline::open(dmg_path)?;
    let mut hfs = pipeline.open_hfs_with_mode(mode)?;

    let mut stdout = io::stdout().lock();
    hfs.read_file_to(path, &mut stdout)?;

    Ok(())
}

fn stat(
    dmg_path: &Path,
    path: &str,
    mode: dpp::ExtractMode,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut pipeline = open_pipeline(dmg_path)?;
    let mut hfs = open_hfs(&mut pipeline, mode)?;

    let stat = hfs.stat(path)?;

    let (d, r) = (dim(), reset());
    header(&format!("stat: {path}"));

    section("Metadata");
    kv("CNID", &stat.cnid.to_string());
    kv("Kind", &format!("{:?}", stat.kind));
    kv(
        "Size",
        &format!("{} ({})", format_size(stat.size), format_commas(stat.size)),
    );
    kv("Permissions", &mode_string(stat.permissions.mode));
    kv(
        "Owner",
        &format!(
            "{}:{}",
            stat.permissions.owner_id, stat.permissions.group_id
        ),
    );
    kv("Data fork extents", &stat.data_fork_extents.to_string());

    if stat.resource_fork_size > 0 {
        kv("Resource fork", &format_size(stat.resource_fork_size));
    }

    kv(
        "Created",
        &format!("{} {d}(HFS+ timestamp){r}", stat.create_date),
    );
    kv(
        "Modified",
        &format!("{} {d}(HFS+ timestamp){r}", stat.modify_date),
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
    let mut type_filter: Option<hfsplus::EntryKind> = match args.file_type {
        None => None,
        Some('f') => Some(hfsplus::EntryKind::File),
        Some('d') => Some(hfsplus::EntryKind::Directory),
        Some('l') => Some(hfsplus::EntryKind::Symlink),
        Some(other) => return Err(format!("unknown type '{other}' (use f, d, or l)").into()),
    };

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

    let (d, r) = (dim(), reset());
    println!();
    if matches.is_empty() {
        println!("  {d}No matches found{r}");
    } else {
        for entry in &matches {
            let color = kind_color(entry.entry.kind);
            let size_str = if entry.entry.kind == hfsplus::EntryKind::File {
                format!("  {d}{}{r}", format_size(entry.entry.size))
            } else {
                String::new()
            };
            println!(
                "  {d}{}{r} {color}{}{r}{size_str}",
                kind_icon(entry.entry.kind),
                entry.path,
            );
        }
        println!();
        println!("  {d}{} match(es){r}", matches.len());
    }
    println!();

    Ok(())
}
