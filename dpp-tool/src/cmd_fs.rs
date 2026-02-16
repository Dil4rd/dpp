use std::io;
use std::path::Path;
use std::time::Instant;

use crate::pipeline::{open_filesystem, open_pipeline};
use crate::style::*;
use crate::{FindArgs, FsCommand};

pub(crate) fn run(
    cmd: FsCommand,
    mode: dpp::ExtractMode,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        FsCommand::Info { dmg } => info(&dmg, mode),
        FsCommand::Ls { dmg, path } => ls(&dmg, &path, mode),
        FsCommand::Tree { dmg, path, depth } => tree(&dmg, path.as_deref(), depth, mode),
        FsCommand::Cat { dmg, path } => cat(&dmg, &path, mode),
        FsCommand::Stat { dmg, path } => stat(&dmg, &path, mode),
        FsCommand::Find { dmg, args } => find(&dmg, args, mode),
    }
}

fn info(dmg_path: &Path, mode: dpp::ExtractMode) -> Result<(), Box<dyn std::error::Error>> {
    let dmg_str = dmg_path.display();
    let mut pipeline = open_pipeline(dmg_path)?;
    let fs = open_filesystem(&mut pipeline, mode)?;
    let vi = fs.volume_info();

    let (d, r) = (dim(), reset());
    let type_label = match vi.fs_type {
        dpp::FsType::HfsPlus => "HFS+ Volume",
        dpp::FsType::Apfs => "APFS Volume",
    };
    header(&format!("{type_label}: {dmg_str}"));

    section("Volume Info");
    if let Some(ref name) = vi.name {
        kv("Name", name);
    }
    if let Some(is_hfsx) = vi.is_hfsx {
        let sig = if is_hfsx {
            format!("HFSX {d}(case-sensitive){r}")
        } else {
            format!("HFS+ {d}(case-insensitive){r}")
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

fn ls(
    dmg_path: &Path,
    path: &str,
    mode: dpp::ExtractMode,
) -> Result<(), Box<dyn std::error::Error>> {
    let dmg_str = dmg_path.display();
    let mut pipeline = open_pipeline(dmg_path)?;
    let mut fs = open_filesystem(&mut pipeline, mode)?;

    let mut entries = fs.list_directory(path)?;
    entries.sort_by(|a, b| {
        let a_dir = a.kind == dpp::FsEntryKind::Directory;
        let b_dir = b.kind == dpp::FsEntryKind::Directory;
        b_dir.cmp(&a_dir).then(a.name.cmp(&b.name))
    });

    let (d, r) = (dim(), reset());
    header(&format!("{dmg_str}:{path}"));
    println!();
    println!("  {d}{:<5} {:>12}  Name{r}", "Kind", "Size");
    println!("  {d}{}{r}", "-".repeat(56));

    for entry in &entries {
        let color = fs_kind_color(entry.kind);
        let icon = fs_kind_icon(entry.kind);
        let size_str = if entry.kind == dpp::FsEntryKind::File {
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
        .filter(|e| e.kind == dpp::FsEntryKind::File)
        .count();
    let dir_count = entries
        .iter()
        .filter(|e| e.kind == dpp::FsEntryKind::Directory)
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
    let mut fs = open_filesystem(&mut pipeline, mode)?;

    header(&format!("Tree: {dmg_str}:{base_path}"));
    println!();

    print_tree(&mut fs, base_path, "", 0, max_depth)?;
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
    let (d, r, b) = (dim(), reset(), bold());

    if depth > max_depth {
        println!("  {prefix}{d}{TEE} ...{r}");
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
            format!("  {d}{}{r}", format_size(entry.size))
        } else {
            String::new()
        };

        println!(
            "  {prefix}{d}{connector}{r} {color}{b}{}{r}{size_str}",
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

fn cat(
    dmg_path: &Path,
    path: &str,
    mode: dpp::ExtractMode,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut pipeline = dpp::DmgPipeline::open(dmg_path)?;
    let mut fs = pipeline.open_filesystem_with_mode(mode)?;

    let mut stdout = io::stdout().lock();
    fs.read_file_to(path, &mut stdout)?;

    Ok(())
}

fn stat(
    dmg_path: &Path,
    path: &str,
    mode: dpp::ExtractMode,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut pipeline = open_pipeline(dmg_path)?;
    let mut fs = open_filesystem(&mut pipeline, mode)?;

    let stat = fs.stat(path)?;

    let (d, r) = (dim(), reset());
    header(&format!("stat: {path}"));

    section("Metadata");
    let id_label = match stat.fs_type {
        dpp::FsType::HfsPlus => "CNID",
        dpp::FsType::Apfs => "OID",
    };
    kv(id_label, &stat.id.to_string());
    kv("Kind", &format!("{:?}", stat.kind));
    kv(
        "Size",
        &format!("{} ({})", format_size(stat.size), format_commas(stat.size)),
    );
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
    kv(
        "Created",
        &format!("{} {d}({time_label}){r}", stat.create_time),
    );
    kv(
        "Modified",
        &format!("{} {d}({time_label}){r}", stat.modify_time),
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
    let mut type_filter = parse_type_filter(args.file_type)?;

    // Default: find *.pkg files
    if name_pattern.is_none() && type_filter.is_none() {
        name_pattern = Some("*.pkg".to_string());
        type_filter = Some(dpp::FsEntryKind::File);
    }

    let mut pipeline = open_pipeline(dmg_path)?;
    let mut fs = open_filesystem(&mut pipeline, mode)?;

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

    let (d, r) = (dim(), reset());
    println!();
    if matches.is_empty() {
        println!("  {d}No matches found{r}");
    } else {
        for entry in &matches {
            let color = fs_kind_color(entry.entry.kind);
            let size_str = if entry.entry.kind == dpp::FsEntryKind::File {
                format!("  {d}{}{r}", format_size(entry.entry.size))
            } else {
                String::new()
            };
            println!(
                "  {d}{}{r} {color}{}{r}{size_str}",
                fs_kind_icon(entry.entry.kind),
                entry.path,
            );
        }
        println!();
        println!("  {d}{} match(es){r}", matches.len());
    }
    println!();

    Ok(())
}

fn parse_type_filter(
    c: Option<char>,
) -> Result<Option<dpp::FsEntryKind>, Box<dyn std::error::Error>> {
    match c {
        None => Ok(None),
        Some('f') => Ok(Some(dpp::FsEntryKind::File)),
        Some('d') => Ok(Some(dpp::FsEntryKind::Directory)),
        Some('l') => Ok(Some(dpp::FsEntryKind::Symlink)),
        Some(other) => Err(format!("unknown type '{other}' (use f, d, or l)").into()),
    }
}
