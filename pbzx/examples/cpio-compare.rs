//! Comprehensive benchmark for PBZX operations.
//!
//! Usage: cargo run --example cpio-compare --release <pbzx-file>

use std::env;
use std::io::{Cursor, Read};
use std::time::{Duration, Instant};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <pbzx-file>", args[0]);
        std::process::exit(1);
    }

    let path = &args[1];
    println!("╔════════════════════════════════════════════════════════════════╗");
    println!("║              PBZX Library Benchmark Suite                      ║");
    println!("╚════════════════════════════════════════════════════════════════╝\n");

    // Get file size
    let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    println!("Input: {}", path);
    println!("File size: {} bytes ({:.2} MB)\n", file_size, file_size as f64 / 1_000_000.0);

    // ═══════════════════════════════════════════════════════════════════════
    // BENCHMARK 1: PBZX Decompression
    // ═══════════════════════════════════════════════════════════════════════
    println!("┌─────────────────────────────────────────────────────────────────┐");
    println!("│ BENCHMARK 1: PBZX Decompression                                 │");
    println!("└─────────────────────────────────────────────────────────────────┘");

    let start = Instant::now();
    let mut reader = pbzx::open(path).expect("Failed to open PBZX");
    let cpio_data = reader.decompress().expect("Failed to decompress");
    let decompress_time = start.elapsed();

    let ratio = file_size as f64 / cpio_data.len() as f64;
    let throughput = cpio_data.len() as f64 / decompress_time.as_secs_f64() / 1_000_000.0;

    println!("  Compressed:   {:>12} bytes", file_size);
    println!("  Decompressed: {:>12} bytes ({:.2} GB)", cpio_data.len(), cpio_data.len() as f64 / 1_000_000_000.0);
    println!("  Ratio:        {:>12.1}%", ratio * 100.0);
    println!("  Time:         {:>12.2?}", decompress_time);
    println!("  Throughput:   {:>12.1} MB/s (decompressed)\n", throughput);

    // Check CPIO magic
    let magic = if cpio_data.len() >= 6 {
        String::from_utf8_lossy(&cpio_data[0..6]).to_string()
    } else {
        "unknown".to_string()
    };
    println!("  CPIO format:  {}\n", magic);

    // ═══════════════════════════════════════════════════════════════════════
    // BENCHMARK 2: CPIO Listing (custom vs cpio-archive)
    // ═══════════════════════════════════════════════════════════════════════
    println!("┌─────────────────────────────────────────────────────────────────┐");
    println!("│ BENCHMARK 2: CPIO Listing Performance                           │");
    println!("└─────────────────────────────────────────────────────────────────┘");

    let custom_list = bench_custom_list(&cpio_data);
    let crate_list = bench_cpio_archive_list(&cpio_data);

    println!("\n  {:<20} {:>12} {:>12} {:>12}", "Implementation", "Entries", "Time", "Throughput");
    println!("  {:-<56}", "");

    if let Ok((entries, time)) = &custom_list {
        let tp = cpio_data.len() as f64 / time.as_secs_f64() / 1_000_000_000.0;
        println!("  {:<20} {:>12} {:>12.2?} {:>9.2} GB/s", "Custom (pbzx)", entries, time, tp);
    } else if let Err(e) = &custom_list {
        println!("  {:<20} ERROR: {}", "Custom (pbzx)", e);
    }

    if let Ok((entries, time)) = &crate_list {
        let tp = cpio_data.len() as f64 / time.as_secs_f64() / 1_000_000_000.0;
        println!("  {:<20} {:>12} {:>12.2?} {:>9.2} GB/s", "cpio-archive", entries, time, tp);
    } else if let Err(e) = &crate_list {
        println!("  {:<20} ERROR: {}", "cpio-archive", e);
    }

    if let (Ok((_, custom_time)), Ok((_, crate_time))) = (&custom_list, &crate_list) {
        let speedup = crate_time.as_secs_f64() / custom_time.as_secs_f64();
        println!("\n  → Custom is {:.0}x faster for listing\n", speedup);
    }

    // ═══════════════════════════════════════════════════════════════════════
    // BENCHMARK 3: CPIO Building (creating archive from scratch)
    // ═══════════════════════════════════════════════════════════════════════
    println!("┌─────────────────────────────────────────────────────────────────┐");
    println!("│ BENCHMARK 3: CPIO Archive Creation                              │");
    println!("└─────────────────────────────────────────────────────────────────┘");

    let custom_build = bench_custom_cpio_build();
    let crate_build = bench_cpio_archive_build();

    println!("\n  {:<20} {:>12} {:>12} {:>12}", "Implementation", "Size", "Time", "Throughput");
    println!("  {:-<56}", "");

    if let Ok((size, time)) = &custom_build {
        let tp = *size as f64 / time.as_secs_f64() / 1_000_000.0;
        println!("  {:<20} {:>9} B {:>12.2?} {:>9.1} MB/s", "Custom (pbzx)", size, time, tp);
    }

    if let Ok((size, time)) = &crate_build {
        let tp = *size as f64 / time.as_secs_f64() / 1_000_000.0;
        println!("  {:<20} {:>9} B {:>12.2?} {:>9.1} MB/s", "cpio-archive", size, time, tp);
    } else if let Err(e) = &crate_build {
        println!("  {:<20} ERROR: {}", "cpio-archive", e);
    }

    if let (Ok((_, custom_time)), Ok((_, crate_time))) = (&custom_build, &crate_build) {
        let speedup = crate_time.as_secs_f64() / custom_time.as_secs_f64();
        if speedup > 1.0 {
            println!("\n  → Custom is {:.1}x faster for building\n", speedup);
        } else {
            println!("\n  → cpio-archive is {:.1}x faster for building\n", 1.0 / speedup);
        }
    }

    // ═══════════════════════════════════════════════════════════════════════
    // BENCHMARK 4: PBZX Compression
    // ═══════════════════════════════════════════════════════════════════════
    println!("┌─────────────────────────────────────────────────────────────────┐");
    println!("│ BENCHMARK 4: PBZX Compression (16MB sample)                     │");
    println!("└─────────────────────────────────────────────────────────────────┘");

    // Use a 16MB sample of the CPIO data for compression benchmark
    let sample_size = 16 * 1024 * 1024;
    let sample = if cpio_data.len() > sample_size {
        &cpio_data[..sample_size]
    } else {
        &cpio_data[..]
    };

    println!("  Sample size: {} bytes ({:.1} MB)\n", sample.len(), sample.len() as f64 / 1_000_000.0);

    for level in [0, 3, 6, 9] {
        let (compressed_size, time) = bench_pbzx_compress(sample, level);
        let ratio = compressed_size as f64 / sample.len() as f64 * 100.0;
        let throughput = sample.len() as f64 / time.as_secs_f64() / 1_000_000.0;
        println!("  Level {}: {:>8} bytes ({:>5.1}%) in {:>8.2?} ({:>6.1} MB/s)",
                 level, compressed_size, ratio, time, throughput);
    }

    // ═══════════════════════════════════════════════════════════════════════
    // SUMMARY
    // ═══════════════════════════════════════════════════════════════════════
    println!("\n╔════════════════════════════════════════════════════════════════╗");
    println!("║                         SUMMARY                                ║");
    println!("╚════════════════════════════════════════════════════════════════╝\n");

    println!("  Operation          Custom (pbzx)      cpio-archive       Winner");
    println!("  ─────────────────────────────────────────────────────────────────");

    // Decompression (only custom)
    println!("  Decompression      {:>10.2?}         N/A              pbzx", decompress_time);

    // Listing
    if let (Ok((_, ct)), Ok((_, crt))) = (&custom_list, &crate_list) {
        let winner = if ct < crt { "pbzx" } else { "cpio-archive" };
        println!("  List files         {:>10.2?}      {:>10.2?}      {}", ct, crt, winner);
    }

    // Building
    if let (Ok((_, ct)), Ok((_, crt))) = (&custom_build, &crate_build) {
        let winner = if ct < crt { "pbzx" } else { "cpio-archive" };
        println!("  Build CPIO         {:>10.2?}      {:>10.2?}      {}", ct, crt, winner);
    }

    // Compression (only custom)
    let (_, compress_time) = bench_pbzx_compress(sample, 6);
    println!("  Compression (L6)   {:>10.2?}         N/A              pbzx", compress_time);

    println!();
}

fn bench_custom_list(data: &[u8]) -> Result<(usize, Duration), String> {
    let start = Instant::now();
    let cursor = Cursor::new(data);
    let mut reader = pbzx::CpioReader::new(cursor);
    let entries = reader.list().map_err(|e| e.to_string())?;
    let elapsed = start.elapsed();
    Ok((entries.len(), elapsed))
}

fn bench_cpio_archive_list(data: &[u8]) -> Result<(usize, Duration), String> {
    let start = Instant::now();

    let owned_data = data.to_vec();
    let cursor = Cursor::new(owned_data);
    let mut reader = cpio_archive::reader(cursor)
        .map_err(|e| format!("Failed to create reader: {}", e))?;

    let mut count = 0usize;

    loop {
        let header = match reader.next() {
            Some(Ok(h)) => h,
            Some(Err(e)) => return Err(format!("Error: {}", e)),
            None => break,
        };

        if header.name() == "TRAILER!!!" {
            break;
        }

        count += 1;

        let file_size = header.file_size() as usize;
        if file_size > 0 {
            let mut buf = vec![0u8; file_size];
            reader.read_exact(&mut buf).map_err(|e| format!("Error: {}", e))?;
        }
    }

    let elapsed = start.elapsed();
    Ok((count, elapsed))
}

fn bench_custom_cpio_build() -> Result<(usize, Duration), String> {
    // Build a CPIO with 1000 files of 1KB each
    let start = Instant::now();

    let mut builder = pbzx::CpioBuilder::new();
    let content = vec![0x42u8; 1024]; // 1KB of data

    for i in 0..1000 {
        builder.add_file(&format!("file_{:04}.dat", i), &content, 0o644);
    }

    let data = builder.finish();
    let elapsed = start.elapsed();

    Ok((data.len(), elapsed))
}

fn bench_cpio_archive_build() -> Result<(usize, Duration), String> {
    use cpio_archive::odc::OdcBuilder;

    let start = Instant::now();

    let mut output = Vec::new();
    let mut builder = OdcBuilder::new(&mut output);
    let content = vec![0x42u8; 1024]; // 1KB of data

    for i in 0..1000 {
        builder.append_file_from_data(&format!("file_{:04}.dat", i), content.clone(), 0o644)
            .map_err(|e| format!("Error: {}", e))?;
    }

    builder.finish().map_err(|e| format!("Error: {}", e))?;
    let elapsed = start.elapsed();

    Ok((output.len(), elapsed))
}

fn bench_pbzx_compress(data: &[u8], level: u32) -> (usize, Duration) {
    let start = Instant::now();

    let mut output = Vec::new();
    let mut writer = pbzx::PbzxWriter::new(&mut output)
        .compression_level(level)
        .chunk_size(16 * 1024 * 1024);

    writer.write_cpio(data).expect("Compression failed");
    writer.finish().expect("Finish failed");

    let elapsed = start.elapsed();
    (output.len(), elapsed)
}
