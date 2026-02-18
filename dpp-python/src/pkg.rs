use std::io::{BufReader, Cursor};

use pyo3::prelude::*;
use pyo3::types::PyBytes;

use crate::error::to_pyerr;
use crate::payload::PyArchive;
use crate::types::*;

// ── PkgReader ───────────────────────────────────────────────────────────

/// Type-erased PKG reader (in-memory or file-backed).
enum PkgInner {
    Memory(dpp::xara::PkgReader<Cursor<Vec<u8>>>),
    File(dpp::xara::PkgReader<BufReader<std::fs::File>>),
}

macro_rules! dispatch_pkg {
    ($self:expr, $method:ident $(, $arg:expr)*) => {
        match $self {
            PkgInner::Memory(pkg) => pkg.$method($($arg),*),
            PkgInner::File(pkg) => pkg.$method($($arg),*),
        }
    };
}

/// macOS PKG package reader.
///
/// Use as a context manager::
///
///     with fs.open_pkg("/path/to/package.pkg") as pkg:
///         print(pkg.components)
///         with pkg.payload("component.pkg") as payload:
///             for f in payload.list():
///                 print(f.path)
#[pyclass(name = "PkgReader")]
pub struct PyPkgReader {
    inner: Option<PkgInner>,
}

impl PyPkgReader {
    pub fn from_memory_reader(pkg: dpp::xara::PkgReader<Cursor<Vec<u8>>>) -> Self {
        PyPkgReader {
            inner: Some(PkgInner::Memory(pkg)),
        }
    }

    pub fn from_file_reader(pkg: dpp::xara::PkgReader<BufReader<std::fs::File>>) -> Self {
        PyPkgReader {
            inner: Some(PkgInner::File(pkg)),
        }
    }

    fn pkg(&mut self) -> PyResult<&mut PkgInner> {
        self.inner
            .as_mut()
            .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("PkgReader is closed"))
    }

    fn pkg_ref(&self) -> PyResult<&PkgInner> {
        self.inner
            .as_ref()
            .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("PkgReader is closed"))
    }
}

#[pymethods]
impl PyPkgReader {
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

    /// Whether this is a product (distribution) package.
    #[getter]
    fn is_product_package(&self) -> PyResult<bool> {
        let pkg = self.pkg_ref()?;
        Ok(dispatch_pkg!(pkg, is_product_package))
    }

    /// List of component package names.
    #[getter]
    fn components(&self) -> PyResult<Vec<String>> {
        let pkg = self.pkg_ref()?;
        Ok(dispatch_pkg!(pkg, components))
    }

    /// List all file paths in the XAR archive.
    fn list_files(&self) -> PyResult<Vec<String>> {
        let pkg = self.pkg_ref()?;
        Ok(dispatch_pkg!(pkg, list_files))
    }

    /// Get the distribution XML, if this is a product package.
    fn distribution(&mut self) -> PyResult<Option<String>> {
        let pkg = self.pkg()?;
        dispatch_pkg!(pkg, distribution).map_err(|e| to_pyerr(dpp::DppError::Xar(e)))
    }

    /// Get the PackageInfo XML for a component.
    fn package_info(&mut self, component: &str) -> PyResult<Option<String>> {
        let pkg = self.pkg()?;
        dispatch_pkg!(pkg, package_info, component).map_err(|e| to_pyerr(dpp::DppError::Xar(e)))
    }

    /// Extract the payload for a component, returning an Archive for reading files.
    fn payload(&mut self, py: Python<'_>, component: &str) -> PyResult<PyArchive> {
        let component = component.to_string();
        let mut pkg_inner = self
            .inner
            .take()
            .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("PkgReader is closed"))?;
        let result = py.detach(|| {
            let data =
                dispatch_pkg!(&mut pkg_inner, payload, &component).map_err(dpp::DppError::Xar)?;
            dpp::pbzx::Archive::from_reader(Cursor::new(data)).map_err(dpp::DppError::Pbzx)
        });
        self.inner = Some(pkg_inner);
        let archive = result.map_err(to_pyerr)?;
        Ok(PyArchive::from_archive(archive))
    }

    /// Extract the raw payload bytes for a component.
    fn payload_bytes<'py>(
        &mut self,
        py: Python<'py>,
        component: &str,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let pkg = self.pkg()?;
        let data =
            dispatch_pkg!(pkg, payload, component).map_err(|e| to_pyerr(dpp::DppError::Xar(e)))?;
        Ok(PyBytes::new(py, &data))
    }

    fn __repr__(&self) -> String {
        if self.inner.is_some() {
            "PkgReader(open)".to_string()
        } else {
            "PkgReader(closed)".to_string()
        }
    }
}

// ── XarArchive ──────────────────────────────────────────────────────────

/// XAR archive reader.
///
/// Use as a context manager::
///
///     with dpp.XarArchive.open("/path/to/archive.xar") as xar:
///         for f in xar.files:
///             print(f.name, f.path, f.file_type)
///         data = xar.read_file(xar.files[0])
#[pyclass(name = "XarArchive")]
pub struct PyXarArchive {
    inner: Option<dpp::xara::XarArchive<BufReader<std::fs::File>>>,
}

#[pymethods]
impl PyXarArchive {
    /// Open a XAR archive from a file path.
    #[staticmethod]
    fn open(path: &str) -> PyResult<Self> {
        let file = std::fs::File::open(path).map_err(|e| to_pyerr(dpp::DppError::Io(e)))?;
        let reader = BufReader::new(file);
        let archive =
            dpp::xara::XarArchive::open(reader).map_err(|e| to_pyerr(dpp::DppError::Xar(e)))?;
        Ok(PyXarArchive {
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

    /// List of files in the archive.
    #[getter]
    fn files(&self) -> PyResult<Vec<PyXarFile>> {
        let archive = self
            .inner
            .as_ref()
            .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("XarArchive is closed"))?;
        Ok(archive.files().iter().map(PyXarFile::from).collect())
    }

    /// Find a file by path.
    fn find(&self, path: &str) -> PyResult<Option<PyXarFile>> {
        let archive = self
            .inner
            .as_ref()
            .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("XarArchive is closed"))?;
        Ok(archive.find(path).map(PyXarFile::from))
    }

    /// Read a file's contents by index.
    fn read_file<'py>(&mut self, py: Python<'py>, index: usize) -> PyResult<Bound<'py, PyBytes>> {
        let archive = self
            .inner
            .as_mut()
            .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("XarArchive is closed"))?;
        let files = archive.files().to_vec();
        let file = files.get(index).ok_or_else(|| {
            pyo3::exceptions::PyIndexError::new_err(format!("file index {} out of range", index))
        })?;
        let data = archive
            .read_file(file)
            .map_err(|e| to_pyerr(dpp::DppError::Xar(e)))?;
        Ok(PyBytes::new(py, &data))
    }

    /// Extract all files to a directory on disk.
    fn extract_all(&mut self, py: Python<'_>, dest: &str) -> PyResult<PyExtractStats> {
        let dest = dest.to_string();
        let mut archive = self
            .inner
            .take()
            .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("XarArchive is closed"))?;
        let result = py.detach(|| archive.extract_all(&dest));
        self.inner = Some(archive);
        let stats = result.map_err(|e| to_pyerr(dpp::DppError::Xar(e)))?;
        Ok(PyExtractStats::from(stats))
    }

    /// Extract files under a base path to a directory on disk.
    /// The base prefix is stripped from output paths.
    fn extract_path(
        &mut self,
        py: Python<'_>,
        base_path: &str,
        dest: &str,
    ) -> PyResult<PyExtractStats> {
        let base_path = base_path.to_string();
        let dest = dest.to_string();
        let mut archive = self
            .inner
            .take()
            .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("XarArchive is closed"))?;
        let result = py.detach(|| archive.extract_path(&base_path, &dest));
        self.inner = Some(archive);
        let stats = result.map_err(|e| to_pyerr(dpp::DppError::Xar(e)))?;
        Ok(PyExtractStats::from(stats))
    }

    fn __repr__(&self) -> String {
        match &self.inner {
            Some(archive) => format!("XarArchive(files={})", archive.files().len()),
            None => "XarArchive(closed)".to_string(),
        }
    }
}
