use std::path::Path;
use std::time::Instant;

use crate::style::*;

/// Extract a filesystem tree (HFS+/APFS) to a local directory.
///
/// Delegates to `FilesystemHandle::extract_path` from the `dpp` library.
pub(crate) fn extract_filesystem(
    fs: &mut dpp::FilesystemHandle,
    base_path: &str,
    output: &Path,
) -> Result<dpp::ExtractStats, Box<dyn std::error::Error>> {
    spinner_msg("Extracting files");
    let t = Instant::now();
    let stats = fs.extract_path(base_path, output)?;
    spinner_done(&format!(" ({})", format_duration(t.elapsed())));

    if stats.symlinks_skipped > 0 {
        warn_msg(&format!(
            "{} symlink(s) skipped during extraction",
            stats.symlinks_skipped
        ));
    }

    Ok(stats)
}

/// Print a summary of extraction statistics.
pub(crate) fn print_extract_summary(stats: &dpp::ExtractStats, output: &Path) {
    let (d, r) = (dim(), reset());
    println!();
    section("Extraction Summary");
    kv("Output", &output.display().to_string());
    kv("Directories", &format_commas(stats.dirs));
    kv_highlight("Files", &format_commas(stats.files));
    if stats.symlinks_skipped > 0 {
        kv("Symlinks skipped", &format_commas(stats.symlinks_skipped));
    }
    kv(
        "Total size",
        &format!(
            "{} {d}({} bytes){r}",
            format_size(stats.bytes),
            format_commas(stats.bytes)
        ),
    );
    println!();
}
