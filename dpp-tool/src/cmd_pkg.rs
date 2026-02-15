use std::io;
use std::path::Path;
use std::time::Instant;

use crate::style::*;
use crate::pipeline::{open_pipeline, open_filesystem};
use crate::{PkgCommand, FindArgs};

/// Build a sort key that produces depth-first tree order (dirs before files at each level).
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

pub(crate) fn run(cmd: PkgCommand, mode: dpp::ExtractMode) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        PkgCommand::Info { dmg, pkg_path } => info(&dmg, &pkg_path, mode),
        PkgCommand::Ls { dmg, pkg_path } => ls(&dmg, &pkg_path, mode),
        PkgCommand::Find { dmg, pkg_path, args } => find(&dmg, &pkg_path, args, mode),
        PkgCommand::Cat { dmg, pkg_path, file } => cat(&dmg, &pkg_path, &file, mode),
    }
}

fn info(dmg_path: &Path, pkg_path: &str, mode: dpp::ExtractMode) -> Result<(), Box<dyn std::error::Error>> {
    let mut pipeline = open_pipeline(dmg_path)?;
    let mut fs = open_filesystem(&mut pipeline, mode)?;

    spinner_msg(&format!("Opening {pkg_path}"));
    let t = Instant::now();
    let pkg = fs.open_pkg(pkg_path)?;
    spinner_done(&format!(" ({})", format_duration(t.elapsed())));

    let (d, r, b, g, y) = (dim(), reset(), bold(), green(), yellow());

    header(&format!("PKG: {pkg_path}"));

    section("Package");
    let pkg_type = if pkg.is_product_package() {
        format!("{g}Product package{r} {d}(multi-component){r}")
    } else {
        format!("{y}Component package{r} {d}(single){r}")
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
                format!("{d}(root){r}")
            } else {
                format!("{b}{comp}{r}")
            };

            let payload_path = if comp.is_empty() {
                "Payload".to_string()
            } else {
                format!("{comp}/Payload")
            };
            let payload_info = if let Some(payload_file) = pkg.xar().find(&payload_path) {
                if let Some(data) = &payload_file.data {
                    format!("  {d}{} compressed, {} uncompressed{r}", format_size(data.length), format_size(data.size))
                } else {
                    String::new()
                }
            } else {
                String::new()
            };

            println!("  {d}{connector}{r} {name}{payload_info}");
        }
    }
    println!();

    Ok(())
}

fn ls(dmg_path: &Path, pkg_path: &str, mode: dpp::ExtractMode) -> Result<(), Box<dyn std::error::Error>> {
    let mut pipeline = open_pipeline(dmg_path)?;
    let mut fs = open_filesystem(&mut pipeline, mode)?;

    spinner_msg(&format!("Opening {pkg_path}"));
    let t = Instant::now();
    let pkg = fs.open_pkg(pkg_path)?;
    spinner_done(&format!(" ({})", format_duration(t.elapsed())));

    let (d, r) = (dim(), reset());
    let bl = blue();
    let cn = cyan();

    header(&format!("PKG: {pkg_path}"));

    section("XAR Contents");
    let files = pkg.xar().files();
    let mut sorted: Vec<_> = files.iter().collect();
    sorted.sort_by(|a, b| {
        let a_is_dir = a.file_type == xara::XarFileType::Directory;
        let b_is_dir = b.file_type == xara::XarFileType::Directory;
        let a_key = tree_sort_key(&a.path, a_is_dir);
        let b_key = tree_sort_key(&b.path, b_is_dir);
        a_key.cmp(&b_key)
    });
    for file in &sorted {
        let size_str = match &file.data {
            Some(data) => format_size(data.size),
            None => format!("{d}dir{r}"),
        };
        let type_color = match file.file_type {
            xara::XarFileType::Directory => bl,
            xara::XarFileType::Symlink => cn,
            xara::XarFileType::File => "",
        };
        println!(
            "  {type_color}{:<50}{r} {:>12}",
            file.path,
            size_str
        );
    }

    println!();
    let file_count = sorted.iter().filter(|f| f.file_type == xara::XarFileType::File).count();
    let dir_count = sorted.iter().filter(|f| f.file_type == xara::XarFileType::Directory).count();
    println!(
        "  {d}{} file(s), {} directory(ies){r}",
        file_count, dir_count
    );
    println!();

    Ok(())
}

fn find(dmg_path: &Path, pkg_path: &str, args: FindArgs, mode: dpp::ExtractMode) -> Result<(), Box<dyn std::error::Error>> {
    let mut name_pattern = args.name;
    let mut type_filter: Option<xara::XarFileType> = match args.file_type {
        None => None,
        Some('f') => Some(xara::XarFileType::File),
        Some('d') => Some(xara::XarFileType::Directory),
        Some('l') => Some(xara::XarFileType::Symlink),
        Some(other) => return Err(format!("unknown type '{other}' (use f, d, or l)").into()),
    };

    // Default: find components (*.pkg directories)
    if name_pattern.is_none() && type_filter.is_none() {
        name_pattern = Some("*.pkg".to_string());
        type_filter = Some(xara::XarFileType::Directory);
    }

    let mut pipeline = open_pipeline(dmg_path)?;
    let mut fs = open_filesystem(&mut pipeline, mode)?;

    spinner_msg(&format!("Opening {pkg_path}"));
    let t = Instant::now();
    let pkg = fs.open_pkg(pkg_path)?;
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

    let (d, r) = (dim(), reset());
    let (bl, cn, w) = (blue(), cyan(), white());
    println!();
    if matches.is_empty() {
        println!("  {d}No matches found{r}");
    } else {
        for file in &matches {
            let type_color = match file.file_type {
                xara::XarFileType::Directory => bl,
                xara::XarFileType::Symlink => cn,
                xara::XarFileType::File => w,
            };
            let icon = match file.file_type {
                xara::XarFileType::Directory => "dir",
                xara::XarFileType::File => "   ",
                xara::XarFileType::Symlink => "lnk",
            };
            let size_str = match &file.data {
                Some(data) => format!("  {d}{}{r}", format_size(data.size)),
                None => String::new(),
            };
            println!(
                "  {d}{icon}{r} {type_color}{}{r}{size_str}",
                file.path,
            );
        }
        println!();
        println!("  {d}{} match(es){r}", matches.len());
    }
    println!();

    Ok(())
}

fn cat(dmg_path: &Path, pkg_path: &str, file_path: &str, mode: dpp::ExtractMode) -> Result<(), Box<dyn std::error::Error>> {
    let mut pipeline = dpp::DmgPipeline::open(dmg_path)?;
    let mut fs = pipeline.open_filesystem_with_mode(mode)?;
    let mut pkg = fs.open_pkg(pkg_path)?;

    let xar_file = pkg.xar().find(file_path)
        .ok_or_else(|| format!("File not found in XAR: {file_path}"))?
        .clone();

    let mut stdout = io::stdout().lock();
    pkg.xar_mut().read_file_to(&xar_file, &mut stdout)?;

    Ok(())
}
