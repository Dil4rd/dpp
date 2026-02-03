//! Example CLI tool for PBZX archives.
//!
//! Usage:
//!   pbzx-tool info <archive>      - Show archive information
//!   pbzx-tool list <archive>      - List files in archive
//!   pbzx-tool extract <archive> <dest> - Extract to directory
//!   pbzx-tool cat <archive> <file> - Print file contents

use std::env;
use std::io::{self, Write};
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 3 {
        eprintln!("Usage: {} <command> <archive> [args...]", args[0]);
        eprintln!();
        eprintln!("Commands:");
        eprintln!("  info <archive>              Show archive information");
        eprintln!("  list <archive>              List files in archive");
        eprintln!("  extract <archive> <dest>    Extract to directory");
        eprintln!("  cat <archive> <file>        Print file contents");
        eprintln!("  decompress <archive> <out>  Decompress to raw CPIO");
        process::exit(1);
    }

    let command = &args[1];
    let archive_path = &args[2];

    let result = match command.as_str() {
        "info" => cmd_info(archive_path),
        "list" => cmd_list(archive_path),
        "extract" => {
            if args.len() < 4 {
                eprintln!("Usage: {} extract <archive> <destination>", args[0]);
                process::exit(1);
            }
            cmd_extract(archive_path, &args[3])
        }
        "cat" => {
            if args.len() < 4 {
                eprintln!("Usage: {} cat <archive> <file_path>", args[0]);
                process::exit(1);
            }
            cmd_cat(archive_path, &args[3])
        }
        "decompress" => {
            if args.len() < 4 {
                eprintln!("Usage: {} decompress <archive> <output>", args[0]);
                process::exit(1);
            }
            cmd_decompress(archive_path, &args[3])
        }
        _ => {
            eprintln!("Unknown command: {}", command);
            process::exit(1);
        }
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}

fn cmd_info(path: &str) -> pbzx::Result<()> {
    let stats = pbzx::stats(path)?;

    println!("Archive Information:");
    println!("  Chunks:            {}", stats.chunk_count);
    println!("  Compressed size:   {} bytes", stats.compressed_size);
    println!("  Uncompressed size: {} bytes", stats.uncompressed_size);
    println!(
        "  Compression ratio: {:.1}% space savings",
        stats.space_savings()
    );
    println!();
    println!("Payload Contents:");
    println!("  Files:       {}", stats.file_count);
    println!("  Directories: {}", stats.directory_count);
    println!("  Total size:  {} bytes", stats.total_file_size);

    Ok(())
}

fn cmd_list(path: &str) -> pbzx::Result<()> {
    let archive = pbzx::Archive::open(path)?;
    let entries = archive.list()?;

    for entry in entries {
        let size_str = if entry.is_dir {
            "-".to_string()
        } else {
            format!("{}", entry.size)
        };

        let link_str = if let Some(ref target) = entry.link_target {
            format!(" -> {}", target)
        } else {
            String::new()
        };

        println!(
            "{} {:>10} {}{}",
            entry.mode_string(),
            size_str,
            entry.path,
            link_str
        );
    }

    Ok(())
}

fn cmd_extract(archive_path: &str, dest: &str) -> pbzx::Result<()> {
    let archive = pbzx::Archive::open(archive_path)?;
    let extracted = archive.extract_all(dest)?;

    println!("Extracted {} files to {}", extracted.len(), dest);
    Ok(())
}

fn cmd_cat(archive_path: &str, file_path: &str) -> pbzx::Result<()> {
    let archive = pbzx::Archive::open(archive_path)?;
    let data = archive.extract_file(file_path)?;

    io::stdout().write_all(&data)?;
    Ok(())
}

fn cmd_decompress(archive_path: &str, output_path: &str) -> pbzx::Result<()> {
    use std::fs::File;
    use std::io::BufWriter;

    let mut reader = pbzx::open(archive_path)?;
    let mut file = BufWriter::new(File::create(output_path)?);

    let bytes = reader.decompress_to(&mut file)?;
    println!("Decompressed {} bytes to {}", bytes, output_path);

    Ok(())
}
