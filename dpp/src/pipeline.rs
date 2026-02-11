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

/// Main pipeline entry point: DMG → HFS+/APFS → PKG → PBZX
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

    /// Extract the main APFS partition and open as a volume.
    /// Uses TempFile mode by default.
    pub fn open_apfs(&mut self) -> Result<ApfsHandle> {
        self.open_apfs_with_mode(ExtractMode::default())
    }

    /// Extract APFS with explicit mode
    pub fn open_apfs_with_mode(&mut self, mode: ExtractMode) -> Result<ApfsHandle> {
        let partition_id = self.apfs_partition_id()?;

        match mode {
            ExtractMode::TempFile => {
                let mut tmp = tempfile::tempfile()?;
                {
                    let mut writer = BufWriter::new(&mut tmp);
                    self.archive
                        .extract_partition_to(partition_id, &mut writer)?;
                }
                tmp.seek(std::io::SeekFrom::Start(0))?;
                let reader = BufReader::new(tmp);
                let volume = apfs::ApfsVolume::open(reader)?;
                Ok(ApfsHandle {
                    inner: ApfsHandleInner::File(volume),
                })
            }
            ExtractMode::InMemory => {
                let partition_data = self
                    .archive
                    .extract_partition(partition_id)?;
                let cursor = Cursor::new(partition_data);
                let volume = apfs::ApfsVolume::open(cursor)?;
                Ok(ApfsHandle {
                    inner: ApfsHandleInner::Memory(volume),
                })
            }
        }
    }

    /// Auto-detect and open the filesystem partition (HFS+ or APFS).
    /// Tries HFS+ first, then falls back to APFS.
    pub fn open_filesystem(&mut self) -> Result<FilesystemHandle> {
        match self.open_hfs() {
            Ok(hfs) => Ok(FilesystemHandle::Hfs(hfs)),
            Err(crate::error::DppError::NoHfsPartition) => {
                match self.open_apfs() {
                    Ok(apfs) => Ok(FilesystemHandle::Apfs(apfs)),
                    Err(crate::error::DppError::NoApfsPartition) => {
                        Err(crate::error::DppError::NoFilesystemPartition)
                    }
                    Err(e) => Err(e),
                }
            }
            Err(e) => Err(e),
        }
    }

    /// Find the partition ID of the APFS partition.
    fn apfs_partition_id(&self) -> Result<i32> {
        let partitions = self.archive.partitions();
        let partition = partitions
            .iter()
            .filter(|p| p.partition_type == udif::PartitionType::Apfs)
            .max_by_key(|p| p.size)
            .ok_or(crate::error::DppError::NoApfsPartition)?;
        Ok(partition.id)
    }
}

// ── HFS+ Handle ─────────────────────────────────────────────────────────

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

// ── APFS Handle ─────────────────────────────────────────────────────────

/// Handle to an opened APFS volume.
/// Type-erased over the underlying reader (temp file vs in-memory).
pub struct ApfsHandle {
    inner: ApfsHandleInner,
}

enum ApfsHandleInner {
    File(apfs::ApfsVolume<BufReader<std::fs::File>>),
    Memory(apfs::ApfsVolume<Cursor<Vec<u8>>>),
}

macro_rules! dispatch_apfs {
    ($self:expr, $method:ident $(, $arg:expr)*) => {
        match &mut $self.inner {
            ApfsHandleInner::File(vol) => vol.$method($($arg),*),
            ApfsHandleInner::Memory(vol) => vol.$method($($arg),*),
        }
    };
}

impl ApfsHandle {
    /// List a directory
    pub fn list_directory(&mut self, path: &str) -> Result<Vec<apfs::DirEntry>> {
        Ok(dispatch_apfs!(self, list_directory, path)?)
    }

    /// Read a file into memory
    pub fn read_file(&mut self, path: &str) -> Result<Vec<u8>> {
        Ok(dispatch_apfs!(self, read_file, path)?)
    }

    /// Stream a file to a writer (low memory)
    pub fn read_file_to<W: std::io::Write>(
        &mut self,
        path: &str,
        writer: &mut W,
    ) -> Result<u64> {
        Ok(dispatch_apfs!(self, read_file_to, path, writer)?)
    }

    /// Get file metadata
    pub fn stat(&mut self, path: &str) -> Result<apfs::FileStat> {
        Ok(dispatch_apfs!(self, stat, path)?)
    }

    /// Walk all files
    pub fn walk(&mut self) -> Result<Vec<apfs::WalkEntry>> {
        Ok(dispatch_apfs!(self, walk)?)
    }

    /// Check if a path exists
    pub fn exists(&mut self, path: &str) -> Result<bool> {
        Ok(dispatch_apfs!(self, exists, path)?)
    }

    /// Get volume information
    pub fn volume_info(&self) -> &apfs::VolumeInfo {
        match &self.inner {
            ApfsHandleInner::File(vol) => vol.volume_info(),
            ApfsHandleInner::Memory(vol) => vol.volume_info(),
        }
    }

    /// Open a .pkg file found on the APFS volume (reads into memory)
    pub fn open_pkg(&mut self, pkg_path: &str) -> Result<xara::PkgReader<Cursor<Vec<u8>>>> {
        let data = dispatch_apfs!(self, read_file, pkg_path)?;
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
            dispatch_apfs!(self, read_file_to, pkg_path, &mut writer)?;
        }
        tmp.seek(std::io::SeekFrom::Start(0))?;
        let reader = BufReader::new(tmp);
        let pkg = xara::PkgReader::open(reader)?;
        Ok(pkg)
    }
}

// ── Unified Filesystem Types ────────────────────────────────────────────

/// Entry kind for unified filesystem entries
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsEntryKind {
    File,
    Directory,
    Symlink,
}

impl From<hfsplus::EntryKind> for FsEntryKind {
    fn from(kind: hfsplus::EntryKind) -> Self {
        match kind {
            hfsplus::EntryKind::File => FsEntryKind::File,
            hfsplus::EntryKind::Directory => FsEntryKind::Directory,
            hfsplus::EntryKind::Symlink => FsEntryKind::Symlink,
        }
    }
}

impl From<apfs::EntryKind> for FsEntryKind {
    fn from(kind: apfs::EntryKind) -> Self {
        match kind {
            apfs::EntryKind::File => FsEntryKind::File,
            apfs::EntryKind::Directory => FsEntryKind::Directory,
            apfs::EntryKind::Symlink => FsEntryKind::Symlink,
        }
    }
}

/// A unified directory entry from either HFS+ or APFS
#[derive(Debug, Clone)]
pub struct FsDirEntry {
    pub name: String,
    pub kind: FsEntryKind,
    pub size: u64,
}

impl From<&hfsplus::DirEntry> for FsDirEntry {
    fn from(e: &hfsplus::DirEntry) -> Self {
        FsDirEntry {
            name: e.name.clone(),
            kind: e.kind.into(),
            size: e.size,
        }
    }
}

impl From<&apfs::DirEntry> for FsDirEntry {
    fn from(e: &apfs::DirEntry) -> Self {
        FsDirEntry {
            name: e.name.clone(),
            kind: e.kind.into(),
            size: e.size,
        }
    }
}

/// A walk entry with full path + directory entry
#[derive(Debug, Clone)]
pub struct FsWalkEntry {
    pub path: String,
    pub entry: FsDirEntry,
}

impl From<&hfsplus::WalkEntry> for FsWalkEntry {
    fn from(e: &hfsplus::WalkEntry) -> Self {
        FsWalkEntry {
            path: e.path.clone(),
            entry: FsDirEntry::from(&e.entry),
        }
    }
}

impl From<&apfs::WalkEntry> for FsWalkEntry {
    fn from(e: &apfs::WalkEntry) -> Self {
        FsWalkEntry {
            path: e.path.clone(),
            entry: FsDirEntry::from(&e.entry),
        }
    }
}

// ── Unified Filesystem Handle ───────────────────────────────────────────

/// Unified handle to either an HFS+ or APFS volume.
/// Returned by `DmgPipeline::open_filesystem()`.
pub enum FilesystemHandle {
    Hfs(HfsHandle),
    Apfs(ApfsHandle),
}

impl FilesystemHandle {
    /// List a directory, returning unified entries
    pub fn list_directory(&mut self, path: &str) -> Result<Vec<FsDirEntry>> {
        match self {
            FilesystemHandle::Hfs(h) => Ok(h
                .list_directory(path)?
                .iter()
                .map(FsDirEntry::from)
                .collect()),
            FilesystemHandle::Apfs(h) => Ok(h
                .list_directory(path)?
                .iter()
                .map(FsDirEntry::from)
                .collect()),
        }
    }

    /// Read a file into memory
    pub fn read_file(&mut self, path: &str) -> Result<Vec<u8>> {
        match self {
            FilesystemHandle::Hfs(h) => h.read_file(path),
            FilesystemHandle::Apfs(h) => h.read_file(path),
        }
    }

    /// Stream a file to a writer
    pub fn read_file_to<W: std::io::Write>(
        &mut self,
        path: &str,
        writer: &mut W,
    ) -> Result<u64> {
        match self {
            FilesystemHandle::Hfs(h) => h.read_file_to(path, writer),
            FilesystemHandle::Apfs(h) => h.read_file_to(path, writer),
        }
    }

    /// Walk all files, returning unified entries
    pub fn walk(&mut self) -> Result<Vec<FsWalkEntry>> {
        match self {
            FilesystemHandle::Hfs(h) => Ok(h.walk()?.iter().map(FsWalkEntry::from).collect()),
            FilesystemHandle::Apfs(h) => Ok(h.walk()?.iter().map(FsWalkEntry::from).collect()),
        }
    }

    /// Check if a path exists
    pub fn exists(&mut self, path: &str) -> Result<bool> {
        match self {
            FilesystemHandle::Hfs(h) => h.exists(path),
            FilesystemHandle::Apfs(h) => h.exists(path),
        }
    }

    /// Open a .pkg file (reads into memory)
    pub fn open_pkg(&mut self, pkg_path: &str) -> Result<xara::PkgReader<Cursor<Vec<u8>>>> {
        match self {
            FilesystemHandle::Hfs(h) => h.open_pkg(pkg_path),
            FilesystemHandle::Apfs(h) => h.open_pkg(pkg_path),
        }
    }

    /// Open a .pkg file by streaming to a temp file (low memory)
    pub fn open_pkg_streaming(
        &mut self,
        pkg_path: &str,
    ) -> Result<xara::PkgReader<BufReader<std::fs::File>>> {
        match self {
            FilesystemHandle::Hfs(h) => h.open_pkg_streaming(pkg_path),
            FilesystemHandle::Apfs(h) => h.open_pkg_streaming(pkg_path),
        }
    }

    /// Access the inner HFS+ handle, if this is an HFS+ volume
    pub fn as_hfs(&self) -> Option<&HfsHandle> {
        match self {
            FilesystemHandle::Hfs(h) => Some(h),
            _ => None,
        }
    }

    /// Access the inner APFS handle, if this is an APFS volume
    pub fn as_apfs(&self) -> Option<&ApfsHandle> {
        match self {
            FilesystemHandle::Apfs(h) => Some(h),
            _ => None,
        }
    }

    /// Mutable access to the inner HFS+ handle
    pub fn as_hfs_mut(&mut self) -> Option<&mut HfsHandle> {
        match self {
            FilesystemHandle::Hfs(h) => Some(h),
            _ => None,
        }
    }

    /// Mutable access to the inner APFS handle
    pub fn as_apfs_mut(&mut self) -> Option<&mut ApfsHandle> {
        match self {
            FilesystemHandle::Apfs(h) => Some(h),
            _ => None,
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
