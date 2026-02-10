use std::time::Instant;

use crate::style::*;

pub(crate) fn open_pipeline(dmg_path: &str) -> Result<dpp::DmgPipeline, Box<dyn std::error::Error>> {
    spinner_msg(&format!("Opening {dmg_path}"));
    let t = Instant::now();
    let pipeline = dpp::DmgPipeline::open(dmg_path)?;
    spinner_done(&format!(" ({})", format_duration(t.elapsed())));
    Ok(pipeline)
}

pub(crate) fn open_hfs(pipeline: &mut dpp::DmgPipeline) -> Result<dpp::HfsHandle, Box<dyn std::error::Error>> {
    spinner_msg("Extracting HFS+ partition");
    let t = Instant::now();
    let hfs = match pipeline.open_hfs() {
        Ok(hfs) => hfs,
        Err(dpp::DppError::NoHfsPartition) => {
            eprintln!(" {RED}failed{RESET}");
            return Err("This DMG contains an APFS partition. APFS is not supported.".into());
        }
        Err(e) => return Err(e.into()),
    };
    spinner_done(&format!(" ({})", format_duration(t.elapsed())));
    Ok(hfs)
}
