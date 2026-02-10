use std::io;
use std::process;

use crate::style::*;
use crate::pipeline::open_pipeline;

pub(crate) fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.is_empty() {
        print_usage();
        process::exit(1);
    }
    match args[0].as_str() {
        "info" => info(&args[1..]),
        "ls" => ls(&args[1..]),
        "cat" => cat(&args[1..]),
        "-h" | "--help" | "help" => { print_usage(); Ok(()) }
        _ => {
            eprintln!("{RED}Unknown dmg command: {}{RESET}", args[0]);
            print_usage();
            process::exit(1);
        }
    }
}

fn print_usage() {
    eprintln!(
        r#"
{BOLD}dpp-tool dmg{RESET} — DMG/UDIF container commands

{BOLD}COMMANDS:{RESET}
    {GREEN}info{RESET}   <dmg>                    Format & compression stats
    {GREEN}ls{RESET}     <dmg>                    List partitions
    {GREEN}cat{RESET}    <dmg> [partition-id]     Raw partition data to stdout
"#
    );
}

fn info(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.is_empty() {
        eprintln!("Usage: dpp-tool dmg info <dmg-file>");
        process::exit(1);
    }

    let dmg_path = &args[0];
    let archive = udif::DmgArchive::open(dmg_path)?;
    let stats = archive.stats();
    let comp_info = archive.compression_info();

    header(&format!("DMG: {dmg_path}"));

    section("Format");
    kv("Version", &stats.version.to_string());
    kv("Partitions", &stats.partition_count.to_string());
    kv("Sectors", &format_commas(stats.sector_count));

    section("Compression");
    kv("Compressed", &format!("{} ({})", format_size(stats.total_compressed), format_size(stats.data_fork_length)));
    kv("Uncompressed", &format_size(stats.total_uncompressed));
    kv_highlight("Space savings", &format!("{:.1}%", stats.space_savings()));

    // Block type summary
    let mut block_types = Vec::new();
    if comp_info.lzfse_blocks > 0 { block_types.push(format!("LZFSE: {}", comp_info.lzfse_blocks)); }
    if comp_info.lzvn_blocks > 0 { block_types.push(format!("LZVN: {}", comp_info.lzvn_blocks)); }
    if comp_info.zlib_blocks > 0 { block_types.push(format!("Zlib: {}", comp_info.zlib_blocks)); }
    if comp_info.bzip2_blocks > 0 { block_types.push(format!("Bzip2: {}", comp_info.bzip2_blocks)); }
    if comp_info.raw_blocks > 0 { block_types.push(format!("Raw: {}", comp_info.raw_blocks)); }
    if comp_info.zero_fill_blocks > 0 { block_types.push(format!("Zero: {}", comp_info.zero_fill_blocks)); }
    if !block_types.is_empty() {
        kv("Block types", &block_types.join(", "));
    }
    println!();

    Ok(())
}

fn ls(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.is_empty() {
        eprintln!("Usage: dpp-tool dmg ls <dmg-file>");
        process::exit(1);
    }

    let dmg_path = &args[0];
    let pipeline = open_pipeline(dmg_path)?;
    let partitions = pipeline.partitions();

    header(&format!("Partitions: {dmg_path}"));
    println!();
    println!("  {DIM}{:>4}  {:>12}  {:>12}  {:>12}  {:>7}  {}{RESET}", "ID", "Sectors", "Size", "Compressed", "Ratio", "Name");
    println!("  {DIM}{}{RESET}", "-".repeat(72));

    for p in &partitions {
        let ratio = if p.size > 0 {
            format!("{:.1}%", (1.0 - p.compressed_size as f64 / p.size as f64) * 100.0)
        } else {
            "N/A".to_string()
        };
        let name_color = if p.name.contains("Apple_HFS") || p.name.contains("Apple_APFS") {
            GREEN
        } else {
            ""
        };
        println!(
            "  {:>4}  {:>12}  {:>12}  {:>12}  {:>7}  {name_color}{}{RESET}",
            p.id,
            p.sectors,
            format_size(p.size),
            format_size(p.compressed_size),
            ratio,
            p.name
        );
    }

    println!();
    println!(
        "  {DIM}{} partition(s){RESET}",
        partitions.len()
    );
    println!();

    Ok(())
}

fn cat(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.is_empty() {
        eprintln!("Usage: dpp-tool dmg cat <dmg-file> [partition-id]");
        process::exit(1);
    }

    let dmg_path = &args[0];
    let mut archive = udif::DmgArchive::open(dmg_path)?;

    let mut stdout = io::stdout().lock();

    if args.len() > 1 {
        let id: i32 = args[1].parse().map_err(|_| {
            format!("Invalid partition ID: {}", args[1])
        })?;
        archive.extract_partition_to(id, &mut stdout)?;
    } else {
        archive.extract_main_partition_to(&mut stdout)?;
    }

    Ok(())
}
