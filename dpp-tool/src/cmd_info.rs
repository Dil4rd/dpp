use std::path::Path;
use std::time::Instant;

use crate::pipeline::{open_filesystem, open_pipeline};
use crate::style::*;

pub(crate) fn run(
    dmg_path: &Path,
    mode: dpp::ExtractMode,
) -> Result<(), Box<dyn std::error::Error>> {
    let dmg_str = dmg_path.display();
    let t_total = Instant::now();

    // DMG layer
    let mut pipeline = open_pipeline(dmg_path)?;
    let partitions = pipeline.partitions();
    let archive_stats = {
        let archive = udif::DmgArchive::open(dmg_path)?;
        (archive.stats(), archive.compression_info())
    };
    let (stats, comp_info) = archive_stats;

    header(&format!("DMG Pipeline: {dmg_str}"));

    section("DMG (UDIF)");
    kv("Version", &stats.version.to_string());
    kv("Partitions", &partitions.len().to_string());
    kv(
        "Compressed",
        &format!(
            "{} ({})",
            format_size(stats.total_compressed),
            format_size(stats.data_fork_length)
        ),
    );
    kv("Uncompressed", &format_size(stats.total_uncompressed));
    kv_highlight("Space savings", &format!("{:.1}%", stats.space_savings()));

    // Block type summary
    let mut block_types = Vec::new();
    if comp_info.lzfse_blocks > 0 {
        block_types.push(format!("LZFSE: {}", comp_info.lzfse_blocks));
    }
    if comp_info.xz_blocks > 0 {
        block_types.push(format!("XZ: {}", comp_info.xz_blocks));
    }
    if comp_info.zlib_blocks > 0 {
        block_types.push(format!("Zlib: {}", comp_info.zlib_blocks));
    }
    if comp_info.bzip2_blocks > 0 {
        block_types.push(format!("Bzip2: {}", comp_info.bzip2_blocks));
    }
    if comp_info.raw_blocks > 0 {
        block_types.push(format!("Raw: {}", comp_info.raw_blocks));
    }
    if comp_info.zero_fill_blocks > 0 {
        block_types.push(format!("Zero: {}", comp_info.zero_fill_blocks));
    }
    if !block_types.is_empty() {
        kv("Block types", &block_types.join(", "));
    }

    // Partition table
    let (d, r, g) = (dim(), reset(), green());
    println!();
    println!(
        "  {d}{:>4}  {:>12}  {:>12}  {:>7}  Name{r}",
        "ID", "Sectors", "Size", "Ratio"
    );
    println!("  {d}{}{r}", "-".repeat(58));
    for p in &partitions {
        let ratio = if p.size > 0 {
            format!(
                "{:.1}%",
                (1.0 - p.compressed_size as f64 / p.size as f64) * 100.0
            )
        } else {
            "N/A".to_string()
        };
        let name_color = if p.name.contains("Apple_HFS") || p.name.contains("Apple_APFS") {
            g
        } else {
            ""
        };
        println!(
            "  {:>4}  {:>12}  {:>12}  {:>7}  {name_color}{}{r}",
            p.id,
            p.sectors,
            format_size(p.size),
            ratio,
            p.name
        );
    }

    // Filesystem layer (HFS+ or APFS, auto-detected)
    match open_filesystem(&mut pipeline, mode) {
        Ok(mut fs) => {
            let vi = fs.volume_info();

            match vi.fs_type {
                dpp::FsType::HfsPlus => {
                    section("HFS+ Volume");
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
                    if let Some(total_blocks) = vi.total_blocks {
                        kv("Total blocks", &format_commas(total_blocks as u64));
                    }
                    if let Some(free_blocks) = vi.free_blocks {
                        kv("Free blocks", &format_commas(free_blocks as u64));
                    }
                    kv_highlight("Files", &format_commas(vi.file_count));
                    kv_highlight("Folders", &format_commas(vi.directory_count));
                }
                dpp::FsType::Apfs => {
                    section("APFS Volume");
                    if let Some(ref name) = vi.name {
                        kv("Name", name);
                    }
                    kv("Block size", &format!("{} bytes", vi.block_size));
                    kv_highlight("Files", &format_commas(vi.file_count));
                    kv_highlight("Directories", &format_commas(vi.directory_count));
                    if let Some(symlink_count) = vi.symlink_count {
                        kv("Symlinks", &format_commas(symlink_count));
                    }
                }
            }

            // Find packages
            spinner_msg("Scanning for .pkg files");
            let t = Instant::now();
            let entries = fs.walk()?;
            let pkg_files: Vec<_> = entries
                .iter()
                .filter(|e| e.entry.kind == dpp::FsEntryKind::File && e.path.ends_with(".pkg"))
                .collect();
            spinner_done(&format!(" ({})", format_duration(t.elapsed())));

            let b = bold();
            section("Packages");
            if pkg_files.is_empty() {
                println!("  {d}No .pkg files found{r}");
            } else {
                for (i, pkg) in pkg_files.iter().enumerate() {
                    let connector = if i == pkg_files.len() - 1 { ELBOW } else { TEE };
                    println!(
                        "  {d}{connector}{r} {b}{}{r}  {d}{}{r}",
                        pkg.path,
                        format_size(pkg.entry.size),
                    );
                }
            }

            // Summary
            let total_files: u64 = entries
                .iter()
                .filter(|e| e.entry.kind == dpp::FsEntryKind::File)
                .count() as u64;
            let total_size: u64 = entries
                .iter()
                .filter(|e| e.entry.kind == dpp::FsEntryKind::File)
                .map(|e| e.entry.size)
                .sum();
            let total_dirs: u64 = entries
                .iter()
                .filter(|e| e.entry.kind == dpp::FsEntryKind::Directory)
                .count() as u64;

            section("Summary");
            kv("Total files", &format_commas(total_files));
            kv("Total directories", &format_commas(total_dirs));
            kv("Total content size", &format_size(total_size));
        }
        Err(e) => {
            let y = yellow();
            section("Filesystem");
            println!("  {y}No HFS+ or APFS partition found in this DMG: {e}{r}");
        }
    }

    section("Timing");
    kv("Pipeline time", &format_duration(t_total.elapsed()));
    println!();

    Ok(())
}
