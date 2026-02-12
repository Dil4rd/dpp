use std::io::Cursor;
use std::process;
use std::time::Instant;

use crate::style::*;

pub(crate) fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.is_empty() {
        eprintln!("Usage: dpp-tool bench <dmg-file>");
        process::exit(1);
    }

    let dmg_path = &args[0];

    header(&format!("Benchmark: {dmg_path}"));

    // Stage 1: DMG open
    section("Stage 1: DMG Open (UDIF parse)");
    let t = Instant::now();
    let mut pipeline = dpp::DmgPipeline::open(dmg_path)?;
    let dmg_time = t.elapsed();
    let partitions = pipeline.partitions();
    kv("Time", &format_duration(dmg_time));
    kv("Partitions found", &partitions.len().to_string());

    // Find main partition size
    let main_partition = partitions.iter()
        .find(|p| p.name.contains("Apple_HFS") || p.name.contains("Apple_APFS"))
        .or_else(|| partitions.iter().max_by_key(|p| p.size));

    if let Some(mp) = main_partition {
        kv("Main partition", &format!("{} ({})", mp.name, format_size(mp.size)));
    }

    // Determine filesystem type label from partition metadata
    let has_hfs = partitions.iter()
        .any(|p| matches!(p.partition_type, udif::PartitionType::Hfs | udif::PartitionType::Hfsx));
    let fs_label = if has_hfs { "HFS+" } else { "APFS" };

    // Stage 2: Filesystem extraction
    section(&format!("Stage 2: {fs_label} Extraction (decompress + parse)"));
    let t = Instant::now();
    let fs_result = pipeline.open_filesystem();
    let fs_time = t.elapsed();

    match fs_result {
        Ok(mut fs) => {
            let vi = fs.volume_info();
            kv("Time", &format_duration(fs_time));
            kv("Block size", &format!("{} bytes", vi.block_size));
            kv("Files", &format_commas(vi.file_count));
            kv("Directories", &format_commas(vi.directory_count));

            if let Some(mp) = main_partition {
                if mp.size > 0 && fs_time.as_secs_f64() > 0.0 {
                    let throughput = mp.size as f64 / fs_time.as_secs_f64() / (1024.0 * 1024.0);
                    kv_highlight("Throughput", &format!("{:.1} MB/s", throughput));
                }
            }

            // Stage 3: Filesystem walk
            section("Stage 3: Filesystem Walk (B-tree traversal)");
            let t = Instant::now();
            let entries = fs.walk()?;
            let walk_time = t.elapsed();
            let file_count = entries.iter().filter(|e| e.entry.kind == dpp::FsEntryKind::File).count();
            let dir_count = entries.iter().filter(|e| e.entry.kind == dpp::FsEntryKind::Directory).count();
            let total_size: u64 = entries.iter().map(|e| e.entry.size).sum();
            kv("Time", &format_duration(walk_time));
            kv("Files", &format_commas(file_count as u64));
            kv("Directories", &format_commas(dir_count as u64));
            kv("Total content", &format_size(total_size));

            if !entries.is_empty() && walk_time.as_secs_f64() > 0.0 {
                kv_highlight("Entries/sec", &format!("{:.0}", entries.len() as f64 / walk_time.as_secs_f64()));
            }

            // Stage 4: PKG discovery
            let pkg_files: Vec<_> = entries
                .iter()
                .filter(|e| e.entry.kind == dpp::FsEntryKind::File && e.path.ends_with(".pkg"))
                .collect();

            if !pkg_files.is_empty() {
                section("Stage 4: PKG Open (XAR parse)");
                let pkg_path = &pkg_files[0].path;
                let t = Instant::now();
                let pkg = fs.open_pkg(pkg_path)?;
                let pkg_time = t.elapsed();
                kv("Time", &format_duration(pkg_time));
                kv("Package", pkg_path);
                kv("Type", if pkg.is_product_package() { "product" } else { "component" });
                kv("Components", &pkg.components().len().to_string());
                kv("XAR entries", &pkg.xar().files().len().to_string());

                // Stage 5: PBZX payload
                let components = pkg.components();
                if !components.is_empty() {
                    section("Stage 5: PBZX Payload (XZ decompress + CPIO parse)");
                    let comp = &components[0];
                    let payload_path = if comp.is_empty() {
                        "Payload".to_string()
                    } else {
                        format!("{comp}/Payload")
                    };

                    // Find payload size
                    if let Some(payload_file) = pkg.xar().find(&payload_path) {
                        if let Some(data) = &payload_file.data {
                            kv("Component", if comp.is_empty() { "(root)" } else { comp });
                            kv("Compressed payload", &format_size(data.length));
                            kv("Uncompressed payload", &format_size(data.size));
                        }
                    }

                    let mut pkg_mut = fs.open_pkg(&pkg_files[0].path)?;
                    let t = Instant::now();
                    let payload = pkg_mut.payload(comp)?;
                    let payload_time = t.elapsed();
                    kv("Payload read time", &format_duration(payload_time));

                    if payload.len() >= 4 && &payload[..4] == b"pbzx" {
                        let t = Instant::now();
                        let archive = pbzx::Archive::from_reader(Cursor::new(&payload))?;
                        let pbzx_entries = archive.list()?;
                        let pbzx_time = t.elapsed();

                        kv("PBZX parse time", &format_duration(pbzx_time));
                        kv("CPIO entries", &format_commas(pbzx_entries.len() as u64));

                        if !payload.is_empty() && pbzx_time.as_secs_f64() > 0.0 {
                            let throughput = payload.len() as f64 / pbzx_time.as_secs_f64() / (1024.0 * 1024.0);
                            kv_highlight("PBZX throughput", &format!("{:.1} MB/s", throughput));
                        }
                    }
                }
            }

            // Summary
            section("Pipeline Summary");
            let total = dmg_time + fs_time + walk_time;
            println!();
            println!("  {DIM}Stage{RESET}                        {DIM}Time{RESET}          {DIM}%{RESET}");
            println!("  {DIM}{}{RESET}", "-".repeat(50));

            let extraction_label = format!("{fs_label} extraction");
            let stages = [
                ("DMG open", dmg_time),
                (extraction_label.as_str(), fs_time),
                ("Filesystem walk", walk_time),
            ];

            let bar_total = 40;
            for (name, time) in &stages {
                let pct = time.as_secs_f64() / total.as_secs_f64() * 100.0;
                let bar_len = (pct / 100.0 * bar_total as f64) as usize;
                let bar: String = (0..bar_len).map(|_| '#').collect();
                let color = if pct > 50.0 { RED } else if pct > 20.0 { YELLOW } else { GREEN };
                println!(
                    "  {:<25} {:>10}  {color}{:>5.1}%{RESET}  {color}{bar}{RESET}",
                    name,
                    format_duration(*time),
                    pct,
                );
            }
            println!("  {DIM}{}{RESET}", "-".repeat(50));
            println!(
                "  {BOLD}{:<25}{RESET} {:>10}",
                "Total",
                format_duration(total),
            );
            println!();
        }
        Err(dpp::DppError::NoFilesystemPartition) => {
            println!("  {YELLOW}No HFS+ or APFS partition found. Skipping filesystem stages.{RESET}");

            section("Pipeline Summary");
            println!();
            kv("DMG open", &format_duration(dmg_time));
            println!("  {DIM}(Filesystem stages skipped — no compatible partition){RESET}");
            println!();
        }
        Err(e) => return Err(e.into()),
    }

    Ok(())
}
