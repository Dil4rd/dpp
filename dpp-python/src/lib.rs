use pyo3::prelude::*;

mod dmg;
mod error;
mod filesystem;
mod payload;
mod pipeline;
mod pkg;
mod types;

/// Python module for parsing Apple DMG disk images, HFS+/APFS filesystems,
/// PKG installers, and PBZX/CPIO payloads.
#[pymodule]
fn _dpp(m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Register exception types
    error::register(m)?;

    // Top-level convenience functions
    m.add_function(wrap_pyfunction!(py_open, m)?)?;
    m.add_function(wrap_pyfunction!(py_find_packages, m)?)?;
    m.add_function(wrap_pyfunction!(py_extract_pkg_payload, m)?)?;

    // Pipeline classes
    m.add_class::<pipeline::PyDmgPipeline>()?;
    m.add_class::<pipeline::PyFilesystemHandle>()?;

    // DMG classes
    m.add_class::<dmg::PyDmgArchive>()?;
    m.add_class::<dmg::PyDmgBuilder>()?;

    // PKG/XAR classes
    m.add_class::<pkg::PyPkgReader>()?;
    m.add_class::<pkg::PyXarArchive>()?;

    // Payload classes
    m.add_class::<payload::PyArchive>()?;
    m.add_class::<payload::PyCpioBuilder>()?;
    m.add_class::<payload::PyPbzxWriter>()?;

    // Filesystem classes
    m.add_class::<filesystem::PyHfsVolume>()?;
    m.add_class::<filesystem::PyApfsVolume>()?;

    // Data types
    m.add_class::<types::PyPartitionInfo>()?;
    m.add_class::<types::PyDirEntry>()?;
    m.add_class::<types::PyFileStat>()?;
    m.add_class::<types::PyVolumeInfo>()?;
    m.add_class::<types::PyWalkEntry>()?;
    m.add_class::<types::PyFileEntry>()?;
    m.add_class::<types::PyCompressionInfo>()?;
    m.add_class::<types::PyDmgStats>()?;
    m.add_class::<types::PyXarFile>()?;
    m.add_class::<types::PyChunkInfo>()?;
    m.add_class::<types::PyArchiveStats>()?;

    Ok(())
}

/// Open a DMG file for reading. Use as a context manager.
///
/// Args:
///     path: Path to the DMG file.
///
/// Returns:
///     A DmgPipeline instance.
#[pyfunction]
#[pyo3(name = "open")]
fn py_open(py: Python<'_>, path: &str) -> PyResult<pipeline::PyDmgPipeline> {
    pipeline::PyDmgPipeline::new(py, path)
}

/// Find all .pkg files inside a DMG.
///
/// Args:
///     path: Path to the DMG file.
///
/// Returns:
///     List of package file paths found in the DMG.
#[pyfunction]
#[pyo3(name = "find_packages")]
fn py_find_packages(path: &str) -> PyResult<Vec<String>> {
    dpp::pipeline::find_packages(path).map_err(error::to_pyerr)
}

/// Extract a PKG payload from a DMG in one call.
///
/// Args:
///     dmg_path: Path to the DMG file.
///     pkg_path: Path to the .pkg file inside the DMG filesystem.
///     component: Component name within the package.
///
/// Returns:
///     An Archive instance for reading the payload contents.
#[pyfunction]
#[pyo3(name = "extract_pkg_payload")]
fn py_extract_pkg_payload(
    dmg_path: &str,
    pkg_path: &str,
    component: &str,
) -> PyResult<payload::PyArchive> {
    let archive = dpp::pipeline::extract_pkg_payload(dmg_path, pkg_path, component)
        .map_err(error::to_pyerr)?;
    Ok(payload::PyArchive::from_archive(archive))
}
