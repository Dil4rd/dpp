use std::io::BufReader;

use pyo3::prelude::*;
use pyo3::types::PyBytes;

use crate::error::to_pyerr;
use crate::types::*;

// ── HfsVolume ───────────────────────────────────────────────────────────

/// Standalone HFS+ volume reader for raw partition images.
///
/// Use as a context manager::
///
///     with dpp.HfsVolume.open("/path/to/partition.img") as vol:
///         for entry in vol.list_directory("/"):
///             print(entry.name)
///         data = vol.read_file("/some/file")
#[pyclass(name = "HfsVolume")]
pub struct PyHfsVolume {
    inner: Option<dpp::hfsplus::HfsVolume<BufReader<std::fs::File>>>,
}

impl PyHfsVolume {
    fn vol(&mut self) -> PyResult<&mut dpp::hfsplus::HfsVolume<BufReader<std::fs::File>>> {
        self.inner
            .as_mut()
            .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("HfsVolume is closed"))
    }
}

#[pymethods]
impl PyHfsVolume {
    /// Open an HFS+ volume from a raw partition image.
    #[staticmethod]
    fn open(path: &str) -> PyResult<Self> {
        let file = std::fs::File::open(path).map_err(|e| to_pyerr(dpp::DppError::Io(e)))?;
        let reader = BufReader::new(file);
        let volume =
            dpp::hfsplus::HfsVolume::open(reader).map_err(|e| to_pyerr(dpp::DppError::Hfs(e)))?;
        Ok(PyHfsVolume {
            inner: Some(volume),
        })
    }

    fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    #[pyo3(signature = (_exc_type=None, _exc_val=None, _exc_tb=None))]
    fn __exit__(
        &mut self,
        _exc_type: Option<&Bound<'_, PyAny>>,
        _exc_val: Option<&Bound<'_, PyAny>>,
        _exc_tb: Option<&Bound<'_, PyAny>>,
    ) -> bool {
        self.inner.take();
        false
    }

    /// List entries in a directory.
    fn list_directory(&mut self, path: &str) -> PyResult<Vec<PyDirEntry>> {
        let vol = self.vol()?;
        let entries = vol
            .list_directory(path)
            .map_err(|e| to_pyerr(dpp::DppError::Hfs(e)))?;
        Ok(entries
            .iter()
            .map(|e| PyDirEntry {
                name: e.name.clone(),
                kind: format!("{:?}", e.kind).to_lowercase(),
                size: e.size,
            })
            .collect())
    }

    /// Read a file into bytes.
    fn read_file<'py>(&mut self, py: Python<'py>, path: &str) -> PyResult<Bound<'py, PyBytes>> {
        let vol = self.vol()?;
        let data = vol
            .read_file(path)
            .map_err(|e| to_pyerr(dpp::DppError::Hfs(e)))?;
        Ok(PyBytes::new(py, &data))
    }

    /// Get file metadata.
    fn stat(&mut self, path: &str) -> PyResult<PyFileStat> {
        let vol = self.vol()?;
        let stat = vol
            .stat(path)
            .map_err(|e| to_pyerr(dpp::DppError::Hfs(e)))?;
        let fs_stat = dpp::FsFileStat::from(&stat);
        Ok(PyFileStat::from(&fs_stat))
    }

    /// Walk all entries in the volume.
    fn walk(&mut self) -> PyResult<Vec<PyWalkEntry>> {
        let vol = self.vol()?;
        let entries = vol.walk().map_err(|e| to_pyerr(dpp::DppError::Hfs(e)))?;
        Ok(entries
            .iter()
            .map(|e| {
                let fs_walk = dpp::FsWalkEntry::from(e);
                PyWalkEntry::from(&fs_walk)
            })
            .collect())
    }

    /// Check if a path exists.
    fn exists(&mut self, path: &str) -> PyResult<bool> {
        let vol = self.vol()?;
        vol.exists(path)
            .map_err(|e| to_pyerr(dpp::DppError::Hfs(e)))
    }

    fn __repr__(&self) -> String {
        if self.inner.is_some() {
            "HfsVolume(open)".to_string()
        } else {
            "HfsVolume(closed)".to_string()
        }
    }
}

// ── ApfsVolume ──────────────────────────────────────────────────────────

/// Standalone APFS volume reader for raw partition images.
///
/// Use as a context manager::
///
///     with dpp.ApfsVolume.open("/path/to/partition.img") as vol:
///         for entry in vol.list_directory("/"):
///             print(entry.name)
///         data = vol.read_file("/some/file")
#[pyclass(name = "ApfsVolume")]
pub struct PyApfsVolume {
    inner: Option<dpp::apfs::ApfsVolume<BufReader<std::fs::File>>>,
}

impl PyApfsVolume {
    fn vol(&mut self) -> PyResult<&mut dpp::apfs::ApfsVolume<BufReader<std::fs::File>>> {
        self.inner
            .as_mut()
            .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("ApfsVolume is closed"))
    }
}

#[pymethods]
impl PyApfsVolume {
    /// Open an APFS volume from a raw partition image.
    #[staticmethod]
    fn open(path: &str) -> PyResult<Self> {
        let file = std::fs::File::open(path).map_err(|e| to_pyerr(dpp::DppError::Io(e)))?;
        let reader = BufReader::new(file);
        let volume =
            dpp::apfs::ApfsVolume::open(reader).map_err(|e| to_pyerr(dpp::DppError::Apfs(e)))?;
        Ok(PyApfsVolume {
            inner: Some(volume),
        })
    }

    fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    #[pyo3(signature = (_exc_type=None, _exc_val=None, _exc_tb=None))]
    fn __exit__(
        &mut self,
        _exc_type: Option<&Bound<'_, PyAny>>,
        _exc_val: Option<&Bound<'_, PyAny>>,
        _exc_tb: Option<&Bound<'_, PyAny>>,
    ) -> bool {
        self.inner.take();
        false
    }

    /// Volume metadata.
    #[getter]
    fn volume_info(&mut self) -> PyResult<PyVolumeInfo> {
        let vol = self.vol()?;
        let info = vol.volume_info();
        Ok(PyVolumeInfo {
            fs_type: "apfs".to_string(),
            block_size: info.block_size,
            file_count: info.num_files,
            directory_count: info.num_directories,
            name: Some(info.name.clone()),
            symlink_count: Some(info.num_symlinks),
            total_blocks: None,
            free_blocks: None,
            version: None,
            is_hfsx: None,
        })
    }

    /// List entries in a directory.
    fn list_directory(&mut self, path: &str) -> PyResult<Vec<PyDirEntry>> {
        let vol = self.vol()?;
        let entries = vol
            .list_directory(path)
            .map_err(|e| to_pyerr(dpp::DppError::Apfs(e)))?;
        Ok(entries
            .iter()
            .map(|e| PyDirEntry {
                name: e.name.clone(),
                kind: format!("{:?}", e.kind).to_lowercase(),
                size: e.size,
            })
            .collect())
    }

    /// Read a file into bytes.
    fn read_file<'py>(&mut self, py: Python<'py>, path: &str) -> PyResult<Bound<'py, PyBytes>> {
        let vol = self.vol()?;
        let data = vol
            .read_file(path)
            .map_err(|e| to_pyerr(dpp::DppError::Apfs(e)))?;
        Ok(PyBytes::new(py, &data))
    }

    /// Get file metadata.
    fn stat(&mut self, path: &str) -> PyResult<PyFileStat> {
        let vol = self.vol()?;
        let stat = vol
            .stat(path)
            .map_err(|e| to_pyerr(dpp::DppError::Apfs(e)))?;
        let fs_stat = dpp::FsFileStat::from(&stat);
        Ok(PyFileStat::from(&fs_stat))
    }

    /// Walk all entries in the volume.
    fn walk(&mut self) -> PyResult<Vec<PyWalkEntry>> {
        let vol = self.vol()?;
        let entries = vol.walk().map_err(|e| to_pyerr(dpp::DppError::Apfs(e)))?;
        Ok(entries
            .iter()
            .map(|e| {
                let fs_walk = dpp::FsWalkEntry::from(e);
                PyWalkEntry::from(&fs_walk)
            })
            .collect())
    }

    /// Check if a path exists.
    fn exists(&mut self, path: &str) -> PyResult<bool> {
        let vol = self.vol()?;
        vol.exists(path)
            .map_err(|e| to_pyerr(dpp::DppError::Apfs(e)))
    }

    fn __repr__(&self) -> String {
        if self.inner.is_some() {
            "ApfsVolume(open)".to_string()
        } else {
            "ApfsVolume(closed)".to_string()
        }
    }
}
