use pyo3::prelude::*;
use pyo3::types::PyBytes;

use crate::error::to_pyerr;
use crate::types::*;

// ── Archive (PBZX/CPIO reader) ──────────────────────────────────────────

/// PBZX/CPIO payload archive.
///
/// Provides access to the decompressed CPIO contents of a PBZX payload.
///
/// Use as a context manager::
///
///     with pkg.payload("component.pkg") as payload:
///         for f in payload.list():
///             print(f.path, f.size)
///         data = payload.extract_file("./usr/bin/tool")
#[pyclass(name = "Archive")]
pub struct PyArchive {
    inner: Option<dpp::pbzx::Archive>,
}

impl PyArchive {
    pub fn from_archive(archive: dpp::pbzx::Archive) -> Self {
        PyArchive {
            inner: Some(archive),
        }
    }

    fn archive(&self) -> PyResult<&dpp::pbzx::Archive> {
        self.inner
            .as_ref()
            .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("Archive is closed"))
    }
}

#[pymethods]
impl PyArchive {
    /// Open a PBZX file from disk.
    #[staticmethod]
    fn open(py: Python<'_>, path: &str) -> PyResult<Self> {
        let path = path.to_string();
        let archive = py
            .detach(|| dpp::pbzx::Archive::open(&path))
            .map_err(|e| to_pyerr(dpp::DppError::Pbzx(e)))?;
        Ok(PyArchive {
            inner: Some(archive),
        })
    }

    /// Create an Archive from raw CPIO data.
    #[staticmethod]
    fn from_cpio(data: &[u8]) -> PyResult<Self> {
        let archive =
            dpp::pbzx::Archive::from_cpio(data).map_err(|e| to_pyerr(dpp::DppError::Pbzx(e)))?;
        Ok(PyArchive {
            inner: Some(archive),
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

    /// List all file entries in the archive.
    fn list(&self) -> PyResult<Vec<PyFileEntry>> {
        let archive = self.archive()?;
        let entries = archive
            .list()
            .map_err(|e| to_pyerr(dpp::DppError::Pbzx(e)))?;
        Ok(entries.iter().map(PyFileEntry::from).collect())
    }

    /// Extract a single file by path, returning its contents as bytes.
    fn extract_file<'py>(&self, py: Python<'py>, path: &str) -> PyResult<Bound<'py, PyBytes>> {
        let archive = self.archive()?;
        let data = archive
            .extract_file(path)
            .map_err(|e| to_pyerr(dpp::DppError::Pbzx(e)))?;
        Ok(PyBytes::new(py, &data))
    }

    /// Extract all files to a directory on disk.
    fn extract_all(&mut self, py: Python<'_>, dest: &str) -> PyResult<PyExtractStats> {
        let dest = dest.to_string();
        let archive = self
            .inner
            .take()
            .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("Archive is closed"))?;
        let result = py.detach(|| archive.extract_all(&dest));
        self.inner = Some(archive);
        let stats = result.map_err(|e| to_pyerr(dpp::DppError::Pbzx(e)))?;
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
        let archive = self
            .inner
            .take()
            .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("Archive is closed"))?;
        let result = py.detach(|| archive.extract_path(&base_path, &dest));
        self.inner = Some(archive);
        let stats = result.map_err(|e| to_pyerr(dpp::DppError::Pbzx(e)))?;
        Ok(PyExtractStats::from(stats))
    }

    /// Size of the decompressed CPIO data.
    #[getter]
    fn decompressed_size(&self) -> PyResult<usize> {
        Ok(self.archive()?.decompressed_size())
    }

    /// Get the raw decompressed CPIO data.
    fn cpio_data<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let archive = self.archive()?;
        Ok(PyBytes::new(py, archive.cpio_data()))
    }

    fn __repr__(&self) -> String {
        match &self.inner {
            Some(a) => format!("Archive(size={})", a.decompressed_size()),
            None => "Archive(closed)".to_string(),
        }
    }
}

// ── CpioBuilder ─────────────────────────────────────────────────────────

/// Builder for creating CPIO archives.
///
/// Example::
///
///     cpio = dpp.CpioBuilder()
///     cpio.add_directory("./usr/bin", mode=0o755)
///     cpio.add_file("./usr/bin/tool", tool_data, mode=0o755)
///     cpio_bytes = cpio.finish()
#[pyclass(name = "CpioBuilder")]
pub struct PyCpioBuilder {
    inner: Option<dpp::pbzx::CpioBuilder>,
}

impl PyCpioBuilder {
    fn builder(&mut self) -> PyResult<&mut dpp::pbzx::CpioBuilder> {
        self.inner.as_mut().ok_or_else(|| {
            pyo3::exceptions::PyRuntimeError::new_err("CpioBuilder already finished")
        })
    }
}

#[pymethods]
impl PyCpioBuilder {
    #[new]
    fn new() -> Self {
        PyCpioBuilder {
            inner: Some(dpp::pbzx::CpioBuilder::new()),
        }
    }

    /// Add a file to the archive.
    ///
    /// Args:
    ///     path: File path within the archive (e.g. "./usr/bin/tool").
    ///     content: File content as bytes.
    ///     mode: Unix file mode (default: 0o644).
    #[pyo3(signature = (path, content, mode=0o644))]
    fn add_file(&mut self, path: &str, content: &[u8], mode: u32) -> PyResult<()> {
        self.builder()?.add_file(path, content, mode);
        Ok(())
    }

    /// Add a directory to the archive.
    ///
    /// Args:
    ///     path: Directory path within the archive (e.g. "./usr/bin").
    ///     mode: Unix directory mode (default: 0o755).
    #[pyo3(signature = (path, mode=0o755))]
    fn add_directory(&mut self, path: &str, mode: u32) -> PyResult<()> {
        self.builder()?.add_directory(path, mode);
        Ok(())
    }

    /// Add a symlink to the archive.
    ///
    /// Args:
    ///     path: Symlink path within the archive.
    ///     target: Symlink target path.
    ///     mode: Unix mode (default: 0o777).
    #[pyo3(signature = (path, target, mode=0o777))]
    fn add_symlink(&mut self, path: &str, target: &str, mode: u32) -> PyResult<()> {
        self.builder()?.add_symlink(path, target, mode);
        Ok(())
    }

    /// Finalize and return the CPIO archive data.
    fn finish<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let builder = self.inner.take().ok_or_else(|| {
            pyo3::exceptions::PyRuntimeError::new_err("CpioBuilder already finished")
        })?;
        let data = builder.finish();
        Ok(PyBytes::new(py, &data))
    }

    /// Number of bytes written so far.
    #[getter]
    fn len(&mut self) -> PyResult<usize> {
        Ok(self.builder()?.len())
    }

    fn __repr__(&self) -> String {
        match &self.inner {
            Some(b) => format!("CpioBuilder(len={})", b.len()),
            None => "CpioBuilder(finished)".to_string(),
        }
    }
}

// ── PbzxWriter ──────────────────────────────────────────────────────────

/// PBZX archive writer.
///
/// Example::
///
///     writer = dpp.PbzxWriter("output.pbzx")
///     writer.write_cpio(cpio_data)
///     writer.finish()
#[pyclass(name = "PbzxWriter")]
pub struct PyPbzxWriter {
    inner: Option<dpp::pbzx::PbzxWriter<std::io::BufWriter<std::fs::File>>>,
}

impl PyPbzxWriter {
    fn writer(
        &mut self,
    ) -> PyResult<&mut dpp::pbzx::PbzxWriter<std::io::BufWriter<std::fs::File>>> {
        self.inner
            .as_mut()
            .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("PbzxWriter already finished"))
    }
}

#[pymethods]
impl PyPbzxWriter {
    /// Create a new PBZX writer targeting the given file path.
    ///
    /// Args:
    ///     path: Output file path.
    ///     chunk_size: Chunk size in bytes (default: 16MB).
    ///     compression_level: XZ compression level 0-9 (default: 6).
    #[new]
    #[pyo3(signature = (path, chunk_size=None, compression_level=None))]
    fn new(
        path: &str,
        chunk_size: Option<usize>,
        compression_level: Option<u32>,
    ) -> PyResult<Self> {
        let file = std::fs::File::create(path).map_err(|e| to_pyerr(dpp::DppError::Io(e)))?;
        let writer = std::io::BufWriter::new(file);
        let mut pbzx = dpp::pbzx::PbzxWriter::new(writer);
        if let Some(cs) = chunk_size {
            pbzx = pbzx.chunk_size(cs);
        }
        if let Some(cl) = compression_level {
            pbzx = pbzx.compression_level(cl);
        }
        Ok(PyPbzxWriter { inner: Some(pbzx) })
    }

    /// Write CPIO data to the PBZX archive.
    fn write_cpio(&mut self, data: &[u8]) -> PyResult<()> {
        self.writer()?
            .write_cpio(data)
            .map_err(|e| to_pyerr(dpp::DppError::Pbzx(e)))
    }

    /// Total bytes written so far.
    #[getter]
    fn total_written(&mut self) -> PyResult<u64> {
        Ok(self.writer()?.total_written())
    }

    /// Finalize the PBZX archive. Must be called when done writing.
    fn finish(&mut self) -> PyResult<()> {
        let writer = self.inner.take().ok_or_else(|| {
            pyo3::exceptions::PyRuntimeError::new_err("PbzxWriter already finished")
        })?;
        writer
            .finish()
            .map_err(|e| to_pyerr(dpp::DppError::Pbzx(e)))?;
        Ok(())
    }

    fn __repr__(&self) -> String {
        if self.inner.is_some() {
            "PbzxWriter(open)".to_string()
        } else {
            "PbzxWriter(finished)".to_string()
        }
    }
}
