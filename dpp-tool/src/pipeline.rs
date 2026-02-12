use std::time::Instant;

use crate::style::*;

pub(crate) fn open_pipeline(dmg_path: &str) -> Result<dpp::DmgPipeline, Box<dyn std::error::Error>> {
    spinner_msg(&format!("Opening {dmg_path}"));
    let t = Instant::now();
    let pipeline = dpp::DmgPipeline::open(dmg_path)?;
    spinner_done(&format!(" ({})", format_duration(t.elapsed())));
    Ok(pipeline)
}

pub(crate) fn open_hfs(pipeline: &mut dpp::DmgPipeline, mode: dpp::ExtractMode) -> Result<dpp::HfsHandle, Box<dyn std::error::Error>> {
    spinner_msg("Extracting HFS+ partition");
    let t = Instant::now();
    let hfs = match pipeline.open_hfs_with_mode(mode) {
        Ok(hfs) => hfs,
        Err(dpp::DppError::NoHfsPartition) => {
            eprintln!(" {RED}failed{RESET}");
            return Err("This DMG does not contain an HFS+ partition. Try the `apfs` subcommand instead.".into());
        }
        Err(e) => return Err(e.into()),
    };
    spinner_done(&format!(" ({})", format_duration(t.elapsed())));
    Ok(hfs)
}

pub(crate) fn open_filesystem(pipeline: &mut dpp::DmgPipeline, mode: dpp::ExtractMode) -> Result<dpp::FilesystemHandle, Box<dyn std::error::Error>> {
    spinner_msg("Detecting and extracting filesystem");
    let t = Instant::now();
    let fs = match pipeline.open_filesystem_with_mode(mode) {
        Ok(fs) => fs,
        Err(dpp::DppError::NoFilesystemPartition) => {
            eprintln!(" {RED}failed{RESET}");
            return Err("This DMG does not contain an HFS+ or APFS partition.".into());
        }
        Err(e) => return Err(e.into()),
    };
    let type_label = match fs.fs_type() {
        dpp::FsType::HfsPlus => "HFS+",
        dpp::FsType::Apfs => "APFS",
    };
    spinner_done(&format!(" ({}, {})", type_label, format_duration(t.elapsed())));
    Ok(fs)
}

pub(crate) fn open_apfs(pipeline: &mut dpp::DmgPipeline, mode: dpp::ExtractMode) -> Result<dpp::ApfsHandle, Box<dyn std::error::Error>> {
    spinner_msg("Extracting APFS partition");
    let t = Instant::now();
    let apfs = match pipeline.open_apfs_with_mode(mode) {
        Ok(apfs) => apfs,
        Err(dpp::DppError::NoApfsPartition) => {
            eprintln!(" {RED}failed{RESET}");
            return Err("This DMG does not contain an APFS partition. Try the `hfs` subcommand instead.".into());
        }
        Err(e) => return Err(e.into()),
    };
    spinner_done(&format!(" ({})", format_duration(t.elapsed())));
    Ok(apfs)
}
