use pyo3::prelude::*;
use pyo3::types::PyBytes;

use crate::error::to_pyerr;
use crate::pkg::PyPkgReader;
use crate::types::*;

/// High-level pipeline for reading DMG files.
///
/// Use as a context manager::
///
///     with dpp.open("installer.dmg") as dmg:
///         for p in dmg.partitions:
///             print(p.name, p.size)
///         with dmg.filesystem() as fs:
///             for entry in fs.list_directory("/"):
///                 print(entry.name)
#[pyclass(name = "DmgPipeline")]
pub struct PyDmgPipeline {
    inner: Option<dpp::DmgPipeline>,
}

impl PyDmgPipeline {
    fn pipeline(&mut self) -> PyResult<&mut dpp::DmgPipeline> {
        self.inner
            .as_mut()
            .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("DmgPipeline is closed"))
    }
}

#[pymethods]
impl PyDmgPipeline {
    #[new]
    pub fn new(py: Python<'_>, path: &str) -> PyResult<Self> {
        let path = path.to_string();
        let pipeline = py
            .detach(|| dpp::DmgPipeline::open(&path))
            .map_err(to_pyerr)?;
        Ok(PyDmgPipeline {
            inner: Some(pipeline),
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

    /// List partitions in the DMG.
    #[getter]
    fn partitions(&mut self) -> PyResult<Vec<PyPartitionInfo>> {
        let pipeline = self.pipeline()?;
        Ok(pipeline
            .partitions()
            .iter()
            .map(PyPartitionInfo::from)
            .collect())
    }

    /// Open the filesystem (auto-detects HFS+/APFS).
    ///
    /// Args:
    ///     mode: Extraction mode - "temp_file" (default) or "in_memory".
    #[pyo3(signature = (mode=None))]
    fn filesystem(&mut self, py: Python<'_>, mode: Option<&str>) -> PyResult<PyFilesystemHandle> {
        let extract_mode = parse_extract_mode(mode)?;
        let mut pipeline = self
            .inner
            .take()
            .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("DmgPipeline is closed"))?;
        let result = py.detach(|| pipeline.open_filesystem_with_mode(extract_mode));
        self.inner = Some(pipeline);
        let handle = result.map_err(to_pyerr)?;
        Ok(PyFilesystemHandle {
            inner: Some(handle),
        })
    }

    fn __repr__(&self) -> String {
        if self.inner.is_some() {
            "DmgPipeline(open)".to_string()
        } else {
            "DmgPipeline(closed)".to_string()
        }
    }
}

fn parse_extract_mode(mode: Option<&str>) -> PyResult<dpp::ExtractMode> {
    match mode {
        None | Some("temp_file") => Ok(dpp::ExtractMode::TempFile),
        Some("in_memory") => Ok(dpp::ExtractMode::InMemory),
        Some(other) => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "invalid mode {:?}: expected 'temp_file' or 'in_memory'",
            other
        ))),
    }
}

/// Handle to an opened filesystem (HFS+ or APFS).
///
/// Use as a context manager::
///
///     with dmg.filesystem() as fs:
///         for entry in fs.list_directory("/"):
///             print(entry.name)
#[pyclass(name = "FilesystemHandle")]
pub struct PyFilesystemHandle {
    inner: Option<dpp::FilesystemHandle>,
}

impl PyFilesystemHandle {
    fn handle(&mut self) -> PyResult<&mut dpp::FilesystemHandle> {
        self.inner
            .as_mut()
            .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("FilesystemHandle is closed"))
    }
}

#[pymethods]
impl PyFilesystemHandle {
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

    /// The filesystem type: "hfsplus" or "apfs".
    #[getter]
    fn fs_type(&self) -> PyResult<String> {
        let handle = self.inner.as_ref().ok_or_else(|| {
            pyo3::exceptions::PyRuntimeError::new_err("FilesystemHandle is closed")
        })?;
        Ok(format!("{:?}", handle.fs_type()).to_lowercase())
    }

    /// Volume metadata.
    #[getter]
    fn volume_info(&self) -> PyResult<PyVolumeInfo> {
        let handle = self.inner.as_ref().ok_or_else(|| {
            pyo3::exceptions::PyRuntimeError::new_err("FilesystemHandle is closed")
        })?;
        Ok(PyVolumeInfo::from(&handle.volume_info()))
    }

    /// List entries in a directory.
    fn list_directory(&mut self, path: &str) -> PyResult<Vec<PyDirEntry>> {
        let handle = self.handle()?;
        let entries = handle.list_directory(path).map_err(to_pyerr)?;
        Ok(entries.iter().map(PyDirEntry::from).collect())
    }

    /// Read a file into bytes.
    fn read_file<'py>(&mut self, py: Python<'py>, path: &str) -> PyResult<Bound<'py, PyBytes>> {
        let path = path.to_string();
        let mut handle = self.inner.take().ok_or_else(|| {
            pyo3::exceptions::PyRuntimeError::new_err("FilesystemHandle is closed")
        })?;
        let result = py.detach(|| handle.read_file(&path));
        self.inner = Some(handle);
        let data = result.map_err(to_pyerr)?;
        Ok(PyBytes::new(py, &data))
    }

    /// Get file metadata.
    fn stat(&mut self, path: &str) -> PyResult<PyFileStat> {
        let handle = self.handle()?;
        let stat = handle.stat(path).map_err(to_pyerr)?;
        Ok(PyFileStat::from(&stat))
    }

    /// Walk all entries in the filesystem.
    fn walk(&mut self, py: Python<'_>) -> PyResult<Vec<PyWalkEntry>> {
        let mut handle = self.inner.take().ok_or_else(|| {
            pyo3::exceptions::PyRuntimeError::new_err("FilesystemHandle is closed")
        })?;
        let result = py.detach(|| handle.walk());
        self.inner = Some(handle);
        let entries = result.map_err(to_pyerr)?;
        Ok(entries.iter().map(PyWalkEntry::from).collect())
    }

    /// Check if a path exists.
    fn exists(&mut self, path: &str) -> PyResult<bool> {
        let handle = self.handle()?;
        handle.exists(path).map_err(to_pyerr)
    }

    /// Extract all files to a directory on disk.
    fn extract_all(&mut self, py: Python<'_>, dest: &str) -> PyResult<PyExtractStats> {
        let dest = dest.to_string();
        let mut handle = self.inner.take().ok_or_else(|| {
            pyo3::exceptions::PyRuntimeError::new_err("FilesystemHandle is closed")
        })?;
        let result = py.detach(|| handle.extract_all(&dest));
        self.inner = Some(handle);
        let stats = result.map_err(to_pyerr)?;
        Ok(PyExtractStats::from(stats))
    }

    /// Extract files under a base path to a directory on disk.
    fn extract_path(
        &mut self,
        py: Python<'_>,
        base_path: &str,
        dest: &str,
    ) -> PyResult<PyExtractStats> {
        let base_path = base_path.to_string();
        let dest = dest.to_string();
        let mut handle = self.inner.take().ok_or_else(|| {
            pyo3::exceptions::PyRuntimeError::new_err("FilesystemHandle is closed")
        })?;
        let result = py.detach(|| handle.extract_path(&base_path, &dest));
        self.inner = Some(handle);
        let stats = result.map_err(to_pyerr)?;
        Ok(PyExtractStats::from(stats))
    }

    /// Open a .pkg file from the filesystem.
    ///
    /// Args:
    ///     pkg_path: Path to the .pkg file within the filesystem.
    ///     streaming: If True, stream to temp file (lower memory). Default False.
    #[pyo3(signature = (pkg_path, streaming=false))]
    fn open_pkg(
        &mut self,
        py: Python<'_>,
        pkg_path: &str,
        streaming: bool,
    ) -> PyResult<PyPkgReader> {
        let pkg_path = pkg_path.to_string();
        let mut handle = self.inner.take().ok_or_else(|| {
            pyo3::exceptions::PyRuntimeError::new_err("FilesystemHandle is closed")
        })?;
        if streaming {
            let result = py.detach(|| handle.open_pkg_streaming(&pkg_path));
            self.inner = Some(handle);
            let pkg = result.map_err(to_pyerr)?;
            Ok(PyPkgReader::from_file_reader(pkg))
        } else {
            let result = py.detach(|| handle.open_pkg(&pkg_path));
            self.inner = Some(handle);
            let pkg = result.map_err(to_pyerr)?;
            Ok(PyPkgReader::from_memory_reader(pkg))
        }
    }

    fn __repr__(&self) -> String {
        match &self.inner {
            Some(h) => format!("FilesystemHandle({:?}, open)", h.fs_type()),
            None => "FilesystemHandle(closed)".to_string(),
        }
    }
}
