use pyo3::prelude::*;

// ── Partition Info ──────────────────────────────────────────────────────

#[pyclass(frozen, skip_from_py_object, name = "PartitionInfo")]
#[derive(Clone)]
pub struct PyPartitionInfo {
    #[pyo3(get)]
    pub name: String,
    #[pyo3(get)]
    pub id: i32,
    #[pyo3(get)]
    pub sectors: u64,
    #[pyo3(get)]
    pub size: u64,
    #[pyo3(get)]
    pub compressed_size: u64,
    #[pyo3(get)]
    pub partition_type: String,
}

#[pymethods]
impl PyPartitionInfo {
    fn __repr__(&self) -> String {
        format!(
            "PartitionInfo(name={:?}, id={}, size={}, type={:?})",
            self.name, self.id, self.size, self.partition_type
        )
    }
}

impl From<&dpp::udif::PartitionInfo> for PyPartitionInfo {
    fn from(p: &dpp::udif::PartitionInfo) -> Self {
        PyPartitionInfo {
            name: p.name.clone(),
            id: p.id,
            sectors: p.sectors,
            size: p.size,
            compressed_size: p.compressed_size,
            partition_type: format!("{:?}", p.partition_type),
        }
    }
}

// ── Directory Entry ─────────────────────────────────────────────────────

#[pyclass(frozen, skip_from_py_object, name = "DirEntry")]
#[derive(Clone)]
pub struct PyDirEntry {
    #[pyo3(get)]
    pub name: String,
    #[pyo3(get)]
    pub kind: String,
    #[pyo3(get)]
    pub size: u64,
}

#[pymethods]
impl PyDirEntry {
    fn __repr__(&self) -> String {
        format!(
            "DirEntry(name={:?}, kind={:?}, size={})",
            self.name, self.kind, self.size
        )
    }
}

impl From<&dpp::FsDirEntry> for PyDirEntry {
    fn from(e: &dpp::FsDirEntry) -> Self {
        PyDirEntry {
            name: e.name.clone(),
            kind: format!("{:?}", e.kind).to_lowercase(),
            size: e.size,
        }
    }
}

// ── File Stat ───────────────────────────────────────────────────────────

#[pyclass(frozen, skip_from_py_object, name = "FileStat")]
#[derive(Clone)]
pub struct PyFileStat {
    #[pyo3(get)]
    pub fs_type: String,
    #[pyo3(get)]
    pub id: u64,
    #[pyo3(get)]
    pub kind: String,
    #[pyo3(get)]
    pub size: u64,
    #[pyo3(get)]
    pub uid: u32,
    #[pyo3(get)]
    pub gid: u32,
    #[pyo3(get)]
    pub mode: u16,
    #[pyo3(get)]
    pub create_time: i64,
    #[pyo3(get)]
    pub modify_time: i64,
    #[pyo3(get)]
    pub nlink: Option<u32>,
    #[pyo3(get)]
    pub data_fork_extents: Option<u32>,
    #[pyo3(get)]
    pub resource_fork_size: Option<u64>,
}

#[pymethods]
impl PyFileStat {
    fn __repr__(&self) -> String {
        format!(
            "FileStat(kind={:?}, size={}, mode={:#o})",
            self.kind, self.size, self.mode
        )
    }
}

impl From<&dpp::FsFileStat> for PyFileStat {
    fn from(s: &dpp::FsFileStat) -> Self {
        PyFileStat {
            fs_type: format!("{:?}", s.fs_type).to_lowercase(),
            id: s.id,
            kind: format!("{:?}", s.kind).to_lowercase(),
            size: s.size,
            uid: s.uid,
            gid: s.gid,
            mode: s.mode,
            create_time: s.create_time,
            modify_time: s.modify_time,
            nlink: s.nlink,
            data_fork_extents: s.data_fork_extents,
            resource_fork_size: s.resource_fork_size,
        }
    }
}

// ── Volume Info ─────────────────────────────────────────────────────────

#[pyclass(frozen, skip_from_py_object, name = "VolumeInfo")]
#[derive(Clone)]
pub struct PyVolumeInfo {
    #[pyo3(get)]
    pub fs_type: String,
    #[pyo3(get)]
    pub block_size: u32,
    #[pyo3(get)]
    pub file_count: u64,
    #[pyo3(get)]
    pub directory_count: u64,
    #[pyo3(get)]
    pub name: Option<String>,
    #[pyo3(get)]
    pub symlink_count: Option<u64>,
    #[pyo3(get)]
    pub total_blocks: Option<u32>,
    #[pyo3(get)]
    pub free_blocks: Option<u32>,
    #[pyo3(get)]
    pub version: Option<u16>,
    #[pyo3(get)]
    pub is_hfsx: Option<bool>,
}

#[pymethods]
impl PyVolumeInfo {
    fn __repr__(&self) -> String {
        format!(
            "VolumeInfo(fs_type={:?}, block_size={}, files={}, dirs={})",
            self.fs_type, self.block_size, self.file_count, self.directory_count
        )
    }
}

impl From<&dpp::FsVolumeInfo> for PyVolumeInfo {
    fn from(v: &dpp::FsVolumeInfo) -> Self {
        PyVolumeInfo {
            fs_type: format!("{:?}", v.fs_type).to_lowercase(),
            block_size: v.block_size,
            file_count: v.file_count,
            directory_count: v.directory_count,
            name: v.name.clone(),
            symlink_count: v.symlink_count,
            total_blocks: v.total_blocks,
            free_blocks: v.free_blocks,
            version: v.version,
            is_hfsx: v.is_hfsx,
        }
    }
}

// ── Walk Entry ──────────────────────────────────────────────────────────

#[pyclass(frozen, skip_from_py_object, name = "WalkEntry")]
#[derive(Clone)]
pub struct PyWalkEntry {
    #[pyo3(get)]
    pub path: String,
    #[pyo3(get)]
    pub name: String,
    #[pyo3(get)]
    pub kind: String,
    #[pyo3(get)]
    pub size: u64,
}

#[pymethods]
impl PyWalkEntry {
    fn __repr__(&self) -> String {
        format!(
            "WalkEntry(path={:?}, kind={:?}, size={})",
            self.path, self.kind, self.size
        )
    }
}

impl From<&dpp::FsWalkEntry> for PyWalkEntry {
    fn from(e: &dpp::FsWalkEntry) -> Self {
        PyWalkEntry {
            path: e.path.clone(),
            name: e.entry.name.clone(),
            kind: format!("{:?}", e.entry.kind).to_lowercase(),
            size: e.entry.size,
        }
    }
}

// ── File Entry (CPIO) ───────────────────────────────────────────────────

#[pyclass(frozen, skip_from_py_object, name = "FileEntry")]
#[derive(Clone)]
pub struct PyFileEntry {
    #[pyo3(get)]
    pub path: String,
    #[pyo3(get)]
    pub size: u64,
    #[pyo3(get)]
    pub mode: u32,
    #[pyo3(get)]
    pub mtime: u32,
    #[pyo3(get)]
    pub uid: u32,
    #[pyo3(get)]
    pub gid: u32,
    #[pyo3(get)]
    pub is_dir: bool,
    #[pyo3(get)]
    pub is_symlink: bool,
    #[pyo3(get)]
    pub link_target: Option<String>,
}

#[pymethods]
impl PyFileEntry {
    fn __repr__(&self) -> String {
        let kind = if self.is_dir {
            "dir"
        } else if self.is_symlink {
            "symlink"
        } else {
            "file"
        };
        format!(
            "FileEntry(path={:?}, kind={:?}, size={})",
            self.path, kind, self.size
        )
    }
}

impl From<&dpp::pbzx::FileEntry> for PyFileEntry {
    fn from(e: &dpp::pbzx::FileEntry) -> Self {
        PyFileEntry {
            path: e.path.clone(),
            size: e.size,
            mode: e.mode,
            mtime: e.mtime,
            uid: e.uid,
            gid: e.gid,
            is_dir: e.is_dir,
            is_symlink: e.is_symlink,
            link_target: e.link_target.clone(),
        }
    }
}

// ── Compression Info ────────────────────────────────────────────────────

#[pyclass(frozen, skip_from_py_object, name = "CompressionInfo")]
#[derive(Clone)]
pub struct PyCompressionInfo {
    #[pyo3(get)]
    pub zero_fill_blocks: u32,
    #[pyo3(get)]
    pub raw_blocks: u32,
    #[pyo3(get)]
    pub zlib_blocks: u32,
    #[pyo3(get)]
    pub bzip2_blocks: u32,
    #[pyo3(get)]
    pub lzfse_blocks: u32,
    #[pyo3(get)]
    pub xz_blocks: u32,
    #[pyo3(get)]
    pub adc_blocks: u32,
}

#[pymethods]
impl PyCompressionInfo {
    fn __repr__(&self) -> String {
        format!(
            "CompressionInfo(zlib={}, bzip2={}, lzfse={}, xz={}, raw={}, zero={})",
            self.zlib_blocks,
            self.bzip2_blocks,
            self.lzfse_blocks,
            self.xz_blocks,
            self.raw_blocks,
            self.zero_fill_blocks,
        )
    }
}

impl From<&dpp::udif::CompressionInfo> for PyCompressionInfo {
    fn from(c: &dpp::udif::CompressionInfo) -> Self {
        PyCompressionInfo {
            zero_fill_blocks: c.zero_fill_blocks,
            raw_blocks: c.raw_blocks,
            zlib_blocks: c.zlib_blocks,
            bzip2_blocks: c.bzip2_blocks,
            lzfse_blocks: c.lzfse_blocks,
            xz_blocks: c.xz_blocks,
            adc_blocks: c.adc_blocks,
        }
    }
}

// ── DMG Stats ───────────────────────────────────────────────────────────

#[pyclass(frozen, skip_from_py_object, name = "DmgStats")]
#[derive(Clone)]
pub struct PyDmgStats {
    #[pyo3(get)]
    pub version: u32,
    #[pyo3(get)]
    pub sector_count: u64,
    #[pyo3(get)]
    pub partition_count: usize,
    #[pyo3(get)]
    pub total_uncompressed: u64,
    #[pyo3(get)]
    pub total_compressed: u64,
    #[pyo3(get)]
    pub data_fork_length: u64,
    #[pyo3(get)]
    pub compression_ratio: f64,
    #[pyo3(get)]
    pub space_savings: f64,
}

#[pymethods]
impl PyDmgStats {
    fn __repr__(&self) -> String {
        format!(
            "DmgStats(partitions={}, uncompressed={}, compressed={}, ratio={:.2})",
            self.partition_count,
            self.total_uncompressed,
            self.total_compressed,
            self.compression_ratio,
        )
    }
}

impl From<&dpp::udif::DmgStats> for PyDmgStats {
    fn from(s: &dpp::udif::DmgStats) -> Self {
        PyDmgStats {
            version: s.version,
            sector_count: s.sector_count,
            partition_count: s.partition_count,
            total_uncompressed: s.total_uncompressed,
            total_compressed: s.total_compressed,
            data_fork_length: s.data_fork_length,
            compression_ratio: s.compression_ratio(),
            space_savings: s.space_savings(),
        }
    }
}

// ── XAR File ────────────────────────────────────────────────────────────

#[pyclass(frozen, skip_from_py_object, name = "XarFile")]
#[derive(Clone)]
pub struct PyXarFile {
    #[pyo3(get)]
    pub id: u64,
    #[pyo3(get)]
    pub name: String,
    #[pyo3(get)]
    pub path: String,
    #[pyo3(get)]
    pub file_type: String,
    #[pyo3(get)]
    pub link: Option<String>,
    #[pyo3(get)]
    pub size: Option<u64>,
    #[pyo3(get)]
    pub compressed_size: Option<u64>,
}

#[pymethods]
impl PyXarFile {
    fn __repr__(&self) -> String {
        format!(
            "XarFile(name={:?}, path={:?}, type={:?})",
            self.name, self.path, self.file_type
        )
    }
}

impl From<&dpp::xara::XarFile> for PyXarFile {
    fn from(f: &dpp::xara::XarFile) -> Self {
        let file_type = match f.file_type {
            dpp::xara::XarFileType::File => "file",
            dpp::xara::XarFileType::Directory => "directory",
            dpp::xara::XarFileType::Symlink => "symlink",
        };
        PyXarFile {
            id: f.id,
            name: f.name.clone(),
            path: f.path.clone(),
            file_type: file_type.to_string(),
            link: f.link.clone(),
            size: f.data.as_ref().map(|d| d.size),
            compressed_size: f.data.as_ref().map(|d| d.length),
        }
    }
}

#[cfg(test)]
mod xar_file_tests {
    use super::PyXarFile;

    #[test]
    fn conversion_preserves_symlink_target() {
        let source = dpp::xara::XarFile {
            id: 1,
            name: "link".to_string(),
            path: "dir/link".to_string(),
            file_type: dpp::xara::XarFileType::Symlink,
            link: Some("../target".to_string()),
            data: None,
            children: Vec::new(),
            parent: None,
        };

        let converted = PyXarFile::from(&source);
        assert_eq!(converted.link.as_deref(), Some("../target"));
    }
}

// ── Chunk Info (PBZX) ───────────────────────────────────────────────────

#[pyclass(frozen, skip_from_py_object, name = "ChunkInfo")]
#[derive(Clone)]
pub struct PyChunkInfo {
    #[pyo3(get)]
    pub index: usize,
    #[pyo3(get)]
    pub offset: u64,
    #[pyo3(get)]
    pub compressed_size: u64,
    #[pyo3(get)]
    pub uncompressed_size: u64,
    #[pyo3(get)]
    pub is_compressed: bool,
    #[pyo3(get)]
    pub compression_ratio: f64,
}

#[pymethods]
impl PyChunkInfo {
    fn __repr__(&self) -> String {
        format!(
            "ChunkInfo(index={}, compressed={}, uncompressed={}, ratio={:.2})",
            self.index, self.compressed_size, self.uncompressed_size, self.compression_ratio
        )
    }
}

impl From<&dpp::pbzx::ChunkInfo> for PyChunkInfo {
    fn from(c: &dpp::pbzx::ChunkInfo) -> Self {
        PyChunkInfo {
            index: c.index,
            offset: c.offset,
            compressed_size: c.compressed_size,
            uncompressed_size: c.uncompressed_size,
            is_compressed: c.is_compressed,
            compression_ratio: c.compression_ratio(),
        }
    }
}

// ── Extract Stats ────────────────────────────────────────────────────────

/// Statistics returned after extraction.
#[pyclass(frozen, skip_from_py_object, name = "ExtractStats")]
#[derive(Clone)]
pub struct PyExtractStats {
    #[pyo3(get)]
    pub files: u64,
    #[pyo3(get)]
    pub dirs: u64,
    #[pyo3(get)]
    pub symlinks_skipped: u64,
    #[pyo3(get)]
    pub bytes: u64,
}

#[pymethods]
impl PyExtractStats {
    fn __repr__(&self) -> String {
        format!(
            "ExtractStats(files={}, dirs={}, symlinks_skipped={}, bytes={})",
            self.files, self.dirs, self.symlinks_skipped, self.bytes
        )
    }
}

impl From<dpp::ExtractStats> for PyExtractStats {
    fn from(s: dpp::ExtractStats) -> Self {
        PyExtractStats {
            files: s.files,
            dirs: s.dirs,
            symlinks_skipped: s.symlinks_skipped,
            bytes: s.bytes,
        }
    }
}

impl From<dpp::pbzx::ExtractStats> for PyExtractStats {
    fn from(s: dpp::pbzx::ExtractStats) -> Self {
        PyExtractStats {
            files: s.files,
            dirs: s.dirs,
            symlinks_skipped: s.symlinks_skipped,
            bytes: s.bytes,
        }
    }
}

impl From<dpp::xara::ExtractStats> for PyExtractStats {
    fn from(s: dpp::xara::ExtractStats) -> Self {
        PyExtractStats {
            files: s.files,
            dirs: s.dirs,
            symlinks_skipped: s.symlinks_skipped,
            bytes: s.bytes,
        }
    }
}

// ── Archive Stats (PBZX) ────────────────────────────────────────────────

#[pyclass(frozen, skip_from_py_object, name = "ArchiveStats")]
#[derive(Clone)]
pub struct PyArchiveStats {
    #[pyo3(get)]
    pub chunk_count: usize,
    #[pyo3(get)]
    pub compressed_size: u64,
    #[pyo3(get)]
    pub uncompressed_size: u64,
    #[pyo3(get)]
    pub file_count: usize,
    #[pyo3(get)]
    pub directory_count: usize,
    #[pyo3(get)]
    pub total_file_size: u64,
    #[pyo3(get)]
    pub compression_ratio: f64,
    #[pyo3(get)]
    pub space_savings: f64,
}

#[pymethods]
impl PyArchiveStats {
    fn __repr__(&self) -> String {
        format!(
            "ArchiveStats(chunks={}, files={}, dirs={}, ratio={:.2})",
            self.chunk_count, self.file_count, self.directory_count, self.compression_ratio
        )
    }
}

impl From<&dpp::pbzx::ArchiveStats> for PyArchiveStats {
    fn from(s: &dpp::pbzx::ArchiveStats) -> Self {
        PyArchiveStats {
            chunk_count: s.chunk_count,
            compressed_size: s.compressed_size,
            uncompressed_size: s.uncompressed_size,
            file_count: s.file_count,
            directory_count: s.directory_count,
            total_file_size: s.total_file_size,
            compression_ratio: s.compression_ratio(),
            space_savings: s.space_savings(),
        }
    }
}
