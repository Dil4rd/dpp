//! UDIF command-line tool
//!
//! A cross-platform utility for working with Apple disk images (DMG files, aka UDIF).
//!
//! # Usage
//!
//! ```text
//! udif-tool info <dmg-file>           Show DMG information
//! udif-tool list <dmg-file>           List partitions
//! udif-tool extract <dmg-file> <dir>  Extract main partition to directory
//! udif-tool extract-all <dmg-file> <output>  Extract full disk image
//! udif-tool create <output.dmg> <input>  Create DMG from raw disk image
//! ```

use std::env;
use std::fs;
use std::path::Path;
use std::process;

use udif::{CompressionMethod, DmgArchive, DmgBuilder, DmgReaderOptions, Result};

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage();
        process::exit(1);
    }

    let result = match args[1].as_str() {
        "info" => cmd_info(&args[2..]),
        "list" => cmd_list(&args[2..]),
        "extract" => cmd_extract(&args[2..]),
        "extract-all" => cmd_extract_all(&args[2..]),
        "extract-partition" => cmd_extract_partition(&args[2..]),
        "create" => cmd_create(&args[2..]),
        "check" => cmd_check(&args[2..]),
        "-h" | "--help" | "help" => {
            print_usage();
            Ok(())
        }
        _ => {
            eprintln!("Unknown command: {}", args[1]);
            print_usage();
            process::exit(1);
        }
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}

fn print_usage() {
    eprintln!(
        r#"UDIF Tool - Cross-platform Apple disk image utility

USAGE:
    udif-tool <COMMAND> [OPTIONS]

COMMANDS:
    info <dmg-file>                    Show DMG information and statistics
    list <dmg-file>                    List all partitions
    extract <dmg-file> <output-file>   Extract main partition to raw disk image
    extract-all <dmg-file> <output>    Extract complete disk image
    extract-partition <dmg-file> <id> <output>  Extract specific partition
    create <output.dmg> <input-file>   Create DMG from raw disk image
    check <file>                       Check if file is a valid DMG

OPTIONS:
    -h, --help    Print help information
    -f, --fast    Skip checksum generation/verification

EXAMPLES:
    udif-tool info image.dmg
    udif-tool list image.dmg
    udif-tool extract image.dmg partition.raw
    udif-tool create output.dmg input.raw
"#
    );
}

fn cmd_info(args: &[String]) -> Result<()> {
    if args.is_empty() {
        eprintln!("Usage: udif-tool info <dmg-file>");
        process::exit(1);
    }

    let path = &args[0];
    let archive = DmgArchive::open(path)?;
    let stats = archive.stats();
    let comp_info = archive.compression_info();
    let koly = archive.koly();

    println!("DMG Information: {}", path);
    println!("{}", "=".repeat(60));
    println!();
    println!("Header:");
    println!("  Version:          {}", stats.version);
    println!("  Sector count:     {}", stats.sector_count);
    println!("  Data fork length: {} bytes", stats.data_fork_length);
    println!("  Segment:          {}/{}", koly.segment_number, koly.segment_count);
    println!();

    println!("Size:");
    println!("  Uncompressed:     {} bytes ({:.2} MB)", stats.total_uncompressed, stats.total_uncompressed as f64 / 1024.0 / 1024.0);
    println!("  Compressed:       {} bytes ({:.2} MB)", stats.total_compressed, stats.total_compressed as f64 / 1024.0 / 1024.0);
    println!("  Compression:      {:.1}%", stats.space_savings());
    println!();

    println!("Partitions:         {}", stats.partition_count);
    println!();

    println!("Block types used:");
    if comp_info.zero_fill_blocks > 0 {
        println!("  Zero-fill:        {} blocks", comp_info.zero_fill_blocks);
    }
    if comp_info.raw_blocks > 0 {
        println!("  Raw:              {} blocks", comp_info.raw_blocks);
    }
    if comp_info.zlib_blocks > 0 {
        println!("  Zlib:             {} blocks", comp_info.zlib_blocks);
    }
    if comp_info.bzip2_blocks > 0 {
        println!("  Bzip2:            {} blocks", comp_info.bzip2_blocks);
    }
    if comp_info.lzfse_blocks > 0 {
        println!("  LZFSE:            {} blocks", comp_info.lzfse_blocks);
    }
    if comp_info.xz_blocks > 0 {
        println!("  XZ:               {} blocks", comp_info.xz_blocks);
    }
    if comp_info.adc_blocks > 0 {
        println!("  ADC:              {} blocks", comp_info.adc_blocks);
    }

    Ok(())
}

fn cmd_list(args: &[String]) -> Result<()> {
    if args.is_empty() {
        eprintln!("Usage: udif-tool list <dmg-file>");
        process::exit(1);
    }

    let path = &args[0];
    let archive = DmgArchive::open(path)?;
    let partitions = archive.partitions();

    println!("Partitions in {}:", path);
    println!("{}", "=".repeat(80));
    println!("{:>4}  {:>12}  {:>12}  {:>8}  {}", "ID", "Sectors", "Size", "Ratio", "Name");
    println!("{}", "-".repeat(80));

    for p in partitions {
        let ratio = if p.size > 0 {
            format!("{:.1}%", (1.0 - p.compressed_size as f64 / p.size as f64) * 100.0)
        } else {
            "N/A".to_string()
        };

        let size_str = format_size(p.size);

        println!(
            "{:>4}  {:>12}  {:>12}  {:>8}  {}",
            p.id, p.sectors, size_str, ratio, p.name
        );
    }

    Ok(())
}

fn cmd_extract(args: &[String]) -> Result<()> {
    if args.len() < 2 {
        eprintln!("Usage: udif-tool extract <dmg-file> <output-file> [--fast]");
        process::exit(1);
    }

    let dmg_path = &args[0];
    let output_path = &args[1];
    let fast_mode = args.iter().any(|a| a == "--fast" || a == "-f");

    println!("Opening {}...", dmg_path);
    let options = DmgReaderOptions {
        verify_checksums: !fast_mode,
    };
    let mut archive = DmgArchive::open_with_options(dmg_path, options)?;

    println!("Extracting main partition...");
    let data = archive.extract_main_partition()?;

    println!("Writing {} bytes to {}...", data.len(), output_path);
    fs::write(output_path, &data)?;

    println!("Done!");
    Ok(())
}

fn cmd_extract_all(args: &[String]) -> Result<()> {
    if args.len() < 2 {
        eprintln!("Usage: udif-tool extract-all <dmg-file> <output-file> [--fast]");
        process::exit(1);
    }

    let dmg_path = &args[0];
    let output_path = &args[1];
    let fast_mode = args.iter().any(|a| a == "--fast" || a == "-f");

    println!("Opening {}...", dmg_path);
    let options = DmgReaderOptions {
        verify_checksums: !fast_mode,
    };
    let mut archive = DmgArchive::open_with_options(dmg_path, options)?;

    println!("Extracting complete disk image...");
    let data = archive.extract_all()?;

    println!("Writing {} bytes to {}...", data.len(), output_path);
    fs::write(output_path, &data)?;

    println!("Done!");
    Ok(())
}

fn cmd_extract_partition(args: &[String]) -> Result<()> {
    if args.len() < 3 {
        eprintln!("Usage: udif-tool extract-partition <dmg-file> <partition-id> <output-file> [--fast]");
        process::exit(1);
    }

    let dmg_path = &args[0];
    let partition_id: i32 = args[1].parse().map_err(|_| {
        udif::DppError::InvalidPath("partition ID must be a number".to_string())
    })?;
    let output_path = &args[2];
    let fast_mode = args.iter().any(|a| a == "--fast" || a == "-f");

    println!("Opening {}...", dmg_path);
    let options = DmgReaderOptions {
        verify_checksums: !fast_mode,
    };
    let mut archive = DmgArchive::open_with_options(dmg_path, options)?;

    println!("Extracting partition {}...", partition_id);
    let data = archive.extract_partition(partition_id)?;

    println!("Writing {} bytes to {}...", data.len(), output_path);
    fs::write(output_path, &data)?;

    println!("Done!");
    Ok(())
}

fn cmd_create(args: &[String]) -> Result<()> {
    if args.len() < 2 {
        eprintln!("Usage: udif-tool create <output.dmg> <input-file> [--compression <method>] [--fast]");
        eprintln!("  Compression methods: raw, zlib (default), bzip2, lzfse");
        process::exit(1);
    }

    let output_path = &args[0];
    let input_path = &args[1];

    let mut compression = CompressionMethod::Zlib;
    let mut fast_mode = false;
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--compression" | "-c" => {
                if i + 1 < args.len() {
                    compression = match args[i + 1].to_lowercase().as_str() {
                        "raw" | "none" => CompressionMethod::Raw,
                        "zlib" => CompressionMethod::Zlib,
                        "bzip2" | "bz2" => CompressionMethod::Bzip2,
                        "lzfse" => CompressionMethod::Lzfse,
                        _ => {
                            eprintln!("Unknown compression method: {}", args[i + 1]);
                            process::exit(1);
                        }
                    };
                    i += 2;
                } else {
                    eprintln!("Missing compression method");
                    process::exit(1);
                }
            }
            "--fast" | "-f" => {
                fast_mode = true;
                i += 1;
            }
            _ => {
                eprintln!("Unknown option: {}", args[i]);
                process::exit(1);
            }
        }
    }

    println!("Reading {}...", input_path);
    let data = fs::read(input_path)?;

    println!("Creating {} with {:?} compression...", output_path, compression);
    let partition_name = Path::new(input_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("partition");

    DmgBuilder::new()
        .compression(compression)
        .skip_checksums(fast_mode)
        .add_partition(partition_name, data)
        .build(output_path)?;

    println!("Done!");
    Ok(())
}

fn cmd_check(args: &[String]) -> Result<()> {
    if args.is_empty() {
        eprintln!("Usage: udif-tool check <file>");
        process::exit(1);
    }

    let path = &args[0];

    if udif::check_dmg(path) {
        println!("{} is a valid DMG file", path);
    } else {
        println!("{} is NOT a valid DMG file", path);
        process::exit(1);
    }

    Ok(())
}

fn format_size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 * 1024 {
        format!("{:.2} GB", bytes as f64 / 1024.0 / 1024.0 / 1024.0)
    } else if bytes >= 1024 * 1024 {
        format!("{:.2} MB", bytes as f64 / 1024.0 / 1024.0)
    } else if bytes >= 1024 {
        format!("{:.2} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}
