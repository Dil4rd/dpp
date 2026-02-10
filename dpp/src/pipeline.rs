use std::io::{BufReader, BufWriter, Cursor, Seek};
use std::path::Path;

use crate::error::Result;

/// Extraction mode for partition data
#[derive(Debug, Clone, Copy)]
pub enum ExtractMode {
    /// Stream to temp file on disk (low memory). Default.
    TempFile,
    /// Buffer entire partition in memory. Fast for small DMGs.
    InMemory,
}

impl Default for ExtractMode {
    fn default() -> Self {
        ExtractMode::TempFile
    }
}

/// Main pipeline entry point: DMG → HFS+ → PKG → PBZX
pub struct DmgPipeline {
    archive: udif::DmgArchive,
}

impl DmgPipeline {
    /// Open a DMG file
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let archive = udif::DmgArchive::open(path)?;
        Ok(DmgPipeline { archive })
    }

    /// List partitions in the DMG
    pub fn partitions(&self) -> Vec<udif::PartitionInfo> {
        self.archive.partitions()
    }

    /// Extract the main HFS+ partition and open as a volume.
    /// Uses TempFile mode by default.
    pub fn open_hfs(&mut self) -> Result<HfsHandle> {
        self.open_hfs_with_mode(ExtractMode::default())
    }

    /// Extract with explicit mode
    pub fn open_hfs_with_mode(&mut self, mode: ExtractMode) -> Result<HfsHandle> {
        let partition_id = self
            .archive
            .hfs_partition_id()
            .map_err(|_| crate::error::DppError::NoHfsPartition)?;

        match mode {
            ExtractMode::TempFile => {
                // Stream partition directly to temp file — never holds full partition in RAM
                let mut tmp = tempfile::tempfile()?;
                {
                    let mut writer = BufWriter::new(&mut tmp);
                    self.archive
                        .extract_partition_to(partition_id, &mut writer)?;
                }
                tmp.seek(std::io::SeekFrom::Start(0))?;
                let reader = BufReader::new(tmp);
                let volume = hfsplus::HfsVolume::open(reader)?;
                Ok(HfsHandle {
                    inner: HfsHandleInner::File(volume),
                })
            }
            ExtractMode::InMemory => {
                let partition_data = self
                    .archive
                    .extract_partition(partition_id)?;
                let cursor = Cursor::new(partition_data);
                let volume = hfsplus::HfsVolume::open(cursor)?;
                Ok(HfsHandle {
                    inner: HfsHandleInner::Memory(volume),
                })
            }
        }
    }
}

/// Handle to an opened HFS+ volume.
/// Type-erased over the underlying reader (temp file vs in-memory).
pub struct HfsHandle {
    inner: HfsHandleInner,
}

enum HfsHandleInner {
    File(hfsplus::HfsVolume<BufReader<std::fs::File>>),
    Memory(hfsplus::HfsVolume<Cursor<Vec<u8>>>),
}

// Macro to dispatch to the inner volume
macro_rules! dispatch {
    ($self:expr, $method:ident $(, $arg:expr)*) => {
        match &mut $self.inner {
            HfsHandleInner::File(vol) => vol.$method($($arg),*),
            HfsHandleInner::Memory(vol) => vol.$method($($arg),*),
        }
    };
}

impl HfsHandle {
    /// List a directory
    pub fn list_directory(&mut self, path: &str) -> Result<Vec<hfsplus::DirEntry>> {
        Ok(dispatch!(self, list_directory, path)?)
    }

    /// Read a file into memory
    pub fn read_file(&mut self, path: &str) -> Result<Vec<u8>> {
        Ok(dispatch!(self, read_file, path)?)
    }

    /// Stream a file to a writer (low memory)
    pub fn read_file_to<W: std::io::Write>(
        &mut self,
        path: &str,
        writer: &mut W,
    ) -> Result<u64> {
        Ok(dispatch!(self, read_file_to, path, writer)?)
    }

    /// Get file metadata
    pub fn stat(&mut self, path: &str) -> Result<hfsplus::FileStat> {
        Ok(dispatch!(self, stat, path)?)
    }

    /// Walk all files
    pub fn walk(&mut self) -> Result<Vec<hfsplus::WalkEntry>> {
        Ok(dispatch!(self, walk)?)
    }

    /// Check if a path exists
    pub fn exists(&mut self, path: &str) -> Result<bool> {
        Ok(dispatch!(self, exists, path)?)
    }

    /// Open a .pkg file found on the HFS+ volume (reads into memory)
    pub fn open_pkg(&mut self, pkg_path: &str) -> Result<xara::PkgReader<Cursor<Vec<u8>>>> {
        let data = dispatch!(self, read_file, pkg_path)?;
        let cursor = Cursor::new(data);
        let pkg = xara::PkgReader::open(cursor)?;
        Ok(pkg)
    }

    /// Open a .pkg file by streaming to a temp file (low memory)
    pub fn open_pkg_streaming(
        &mut self,
        pkg_path: &str,
    ) -> Result<xara::PkgReader<BufReader<std::fs::File>>> {
        let mut tmp = tempfile::tempfile()?;
        {
            let mut writer = BufWriter::new(&mut tmp);
            dispatch!(self, read_file_to, pkg_path, &mut writer)?;
        }
        tmp.seek(std::io::SeekFrom::Start(0))?;
        let reader = BufReader::new(tmp);
        let pkg = xara::PkgReader::open(reader)?;
        Ok(pkg)
    }

    /// Access volume header info
    pub fn volume_header(&self) -> &hfsplus::VolumeHeader {
        match &self.inner {
            HfsHandleInner::File(vol) => vol.volume_header(),
            HfsHandleInner::Memory(vol) => vol.volume_header(),
        }
    }
}

/// Convenience: walk a DMG and list all .pkg files found
pub fn find_packages(dmg_path: impl AsRef<Path>) -> Result<Vec<String>> {
    let mut pipeline = DmgPipeline::open(dmg_path)?;
    let mut hfs = pipeline.open_hfs()?;

    let entries = hfs.walk()?;
    let pkgs: Vec<String> = entries
        .into_iter()
        .filter(|e| e.entry.kind == hfsplus::EntryKind::File && e.path.ends_with(".pkg"))
        .map(|e| e.path)
        .collect();

    Ok(pkgs)
}

/// Convenience: extract a PKG payload from a DMG in one call
pub fn extract_pkg_payload(
    dmg_path: impl AsRef<Path>,
    pkg_path: &str,
    component: &str,
) -> Result<pbzx::Archive> {
    let mut pipeline = DmgPipeline::open(dmg_path)?;
    let mut hfs = pipeline.open_hfs()?;
    let mut pkg = hfs.open_pkg(pkg_path)?;
    let payload_data = pkg.payload(component)?;
    let archive = pbzx::Archive::from_reader(Cursor::new(payload_data))?;
    Ok(archive)
}
