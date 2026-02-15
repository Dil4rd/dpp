use pyo3::prelude::*;
use pyo3::types::PyBytes;

use crate::error::to_pyerr;
use crate::types::*;

/// Lower-level DMG archive reader.
///
/// Provides direct partition access, stats, and compression info.
///
/// Use as a context manager::
///
///     with dpp.DmgArchive.open("file.dmg") as archive:
///         print(archive.stats)
///         data = archive.extract_partition(0)
#[pyclass(name = "DmgArchive")]
pub struct PyDmgArchive {
    inner: Option<dpp::udif::DmgArchive>,
}

impl PyDmgArchive {
    fn archive(&mut self) -> PyResult<&mut dpp::udif::DmgArchive> {
        self.inner
            .as_mut()
            .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("DmgArchive is closed"))
    }

    fn archive_ref(&self) -> PyResult<&dpp::udif::DmgArchive> {
        self.inner
            .as_ref()
            .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("DmgArchive is closed"))
    }
}

#[pymethods]
impl PyDmgArchive {
    /// Open a DMG file.
    #[staticmethod]
    fn open(path: &str) -> PyResult<Self> {
        let archive = dpp::udif::DmgArchive::open(path).map_err(|e| {
            to_pyerr(dpp::DppError::Dmg(e))
        })?;
        Ok(PyDmgArchive {
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

    /// DMG statistics.
    #[getter]
    fn stats(&self) -> PyResult<PyDmgStats> {
        let archive = self.archive_ref()?;
        Ok(PyDmgStats::from(&archive.stats()))
    }

    /// Compression block counts by type.
    #[getter]
    fn compression_info(&self) -> PyResult<PyCompressionInfo> {
        let archive = self.archive_ref()?;
        Ok(PyCompressionInfo::from(&archive.compression_info()))
    }

    /// List partitions.
    #[getter]
    fn partitions(&self) -> PyResult<Vec<PyPartitionInfo>> {
        let archive = self.archive_ref()?;
        Ok(archive
            .partitions()
            .iter()
            .map(PyPartitionInfo::from)
            .collect())
    }

    /// Extract a partition by ID, returning its data as bytes.
    fn extract_partition<'py>(
        &mut self,
        py: Python<'py>,
        id: i32,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let archive = self.archive()?;
        let data = archive.extract_partition(id).map_err(|e| {
            to_pyerr(dpp::DppError::Dmg(e))
        })?;
        Ok(PyBytes::new(py, &data))
    }

    /// Extract a partition by name, returning its data as bytes.
    fn extract_partition_by_name<'py>(
        &mut self,
        py: Python<'py>,
        name: &str,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let archive = self.archive()?;
        let data = archive.extract_partition_by_name(name).map_err(|e| {
            to_pyerr(dpp::DppError::Dmg(e))
        })?;
        Ok(PyBytes::new(py, &data))
    }

    /// Extract a partition to a file on disk.
    fn extract_partition_to(&mut self, id: i32, path: &str) -> PyResult<()> {
        let archive = self.archive()?;
        archive.extract_partition_to_file(id, path).map_err(|e| {
            to_pyerr(dpp::DppError::Dmg(e))
        })
    }

    /// Extract the main partition, returning its data as bytes.
    fn extract_main_partition<'py>(
        &mut self,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let archive = self.archive()?;
        let data = archive.extract_main_partition().map_err(|e| {
            to_pyerr(dpp::DppError::Dmg(e))
        })?;
        Ok(PyBytes::new(py, &data))
    }

    /// Extract the main partition to a file on disk.
    fn extract_main_partition_to(&mut self, path: &str) -> PyResult<()> {
        let archive = self.archive()?;
        archive.extract_main_partition_to_file(path).map_err(|e| {
            to_pyerr(dpp::DppError::Dmg(e))
        })
    }

    fn __repr__(&self) -> String {
        if self.inner.is_some() {
            "DmgArchive(open)".to_string()
        } else {
            "DmgArchive(closed)".to_string()
        }
    }
}

/// DMG file builder.
///
/// Example::
///
///     builder = dpp.DmgBuilder()
///     builder.compression = "zlib"
///     builder.add_partition("disk image", partition_data)
///     builder.build("output.dmg")
#[pyclass(name = "DmgBuilder")]
pub struct PyDmgBuilder {
    compression: String,
    compression_level: u32,
    chunk_size: Option<usize>,
    skip_checksums: bool,
    partitions: Vec<(String, Vec<u8>)>,
}

#[pymethods]
impl PyDmgBuilder {
    #[new]
    fn new() -> Self {
        PyDmgBuilder {
            compression: "zlib".to_string(),
            compression_level: 6,
            chunk_size: None,
            skip_checksums: false,
            partitions: Vec::new(),
        }
    }

    /// Compression method: "raw", "zlib", "bzip2", or "lzfse".
    #[getter]
    fn get_compression(&self) -> &str {
        &self.compression
    }

    #[setter]
    fn set_compression(&mut self, value: &str) -> PyResult<()> {
        parse_compression(value)?;
        self.compression = value.to_string();
        Ok(())
    }

    /// Compression level (0-9).
    #[getter]
    fn get_compression_level(&self) -> u32 {
        self.compression_level
    }

    #[setter]
    fn set_compression_level(&mut self, value: u32) {
        self.compression_level = value;
    }

    /// Chunk size in bytes (default: library default).
    #[getter]
    fn get_chunk_size(&self) -> Option<usize> {
        self.chunk_size
    }

    #[setter]
    fn set_chunk_size(&mut self, value: usize) {
        self.chunk_size = Some(value);
    }

    /// Whether to skip checksum generation.
    #[getter]
    fn get_skip_checksums(&self) -> bool {
        self.skip_checksums
    }

    #[setter]
    fn set_skip_checksums(&mut self, value: bool) {
        self.skip_checksums = value;
    }

    /// Add a partition with the given name and data.
    fn add_partition(&mut self, name: &str, data: &[u8]) {
        self.partitions.push((name.to_string(), data.to_vec()));
    }

    /// Build the DMG and write it to the given path.
    fn build(&self, path: &str) -> PyResult<()> {
        let compression = parse_compression(&self.compression)?;
        let mut builder = dpp::udif::DmgBuilder::new()
            .compression(compression)
            .compression_level(self.compression_level)
            .skip_checksums(self.skip_checksums);

        if let Some(chunk_size) = self.chunk_size {
            builder = builder.chunk_size(chunk_size);
        }

        for (name, data) in &self.partitions {
            builder = builder.add_partition(name, data.clone());
        }

        builder.build(path).map_err(|e| to_pyerr(dpp::DppError::Dmg(e)))
    }

    fn __repr__(&self) -> String {
        format!(
            "DmgBuilder(compression={:?}, partitions={})",
            self.compression,
            self.partitions.len()
        )
    }
}

fn parse_compression(method: &str) -> PyResult<dpp::udif::CompressionMethod> {
    match method {
        "raw" => Ok(dpp::udif::CompressionMethod::Raw),
        "zlib" => Ok(dpp::udif::CompressionMethod::Zlib),
        "bzip2" => Ok(dpp::udif::CompressionMethod::Bzip2),
        "lzfse" => Ok(dpp::udif::CompressionMethod::Lzfse),
        other => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "invalid compression {:?}: expected 'raw', 'zlib', 'bzip2', or 'lzfse'",
            other
        ))),
    }
}
