use std::io;
use std::path::Path;

use crate::DmgCommand;
use crate::pipeline::open_pipeline;
use crate::style::*;

pub(crate) fn run(
    cmd: DmgCommand,
    _mode: dpp::ExtractMode,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        DmgCommand::Info { dmg } => info(&dmg),
        DmgCommand::Ls { dmg } => ls(&dmg),
        DmgCommand::Cat { dmg, partition_id } => cat(&dmg, partition_id),
    }
}

fn info(dmg_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let dmg_str = dmg_path.display();
    let archive = udif::DmgArchive::open(dmg_path)?;
    let stats = archive.stats();
    let comp_info = archive.compression_info();

    header(&format!("DMG: {dmg_str}"));

    section("Format");
    kv("Version", &stats.version.to_string());
    kv("Partitions", &stats.partition_count.to_string());
    kv("Sectors", &format_commas(stats.sector_count));

    section("Compression");
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
    println!();

    Ok(())
}

fn ls(dmg_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let dmg_str = dmg_path.display();
    let pipeline = open_pipeline(dmg_path)?;
    let partitions = pipeline.partitions();

    header(&format!("Partitions: {dmg_str}"));
    println!();
    let (d, r) = (dim(), reset());
    let g = green();
    println!(
        "  {d}{:>4}  {:>12}  {:>12}  {:>12}  {:>7}  Name{r}",
        "ID", "Sectors", "Size", "Compressed", "Ratio"
    );
    println!("  {d}{}{r}", "-".repeat(72));

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
            "  {:>4}  {:>12}  {:>12}  {:>12}  {:>7}  {name_color}{}{r}",
            p.id,
            p.sectors,
            format_size(p.size),
            format_size(p.compressed_size),
            ratio,
            p.name
        );
    }

    println!();
    println!("  {d}{} partition(s){r}", partitions.len());
    println!();

    Ok(())
}

fn cat(dmg_path: &Path, partition_id: Option<i32>) -> Result<(), Box<dyn std::error::Error>> {
    let mut archive = udif::DmgArchive::open(dmg_path)?;
    let mut stdout = io::stdout().lock();

    if let Some(id) = partition_id {
        archive.extract_partition_to(id, &mut stdout)?;
    } else {
        archive.extract_main_partition_to(&mut stdout)?;
    }

    Ok(())
}
