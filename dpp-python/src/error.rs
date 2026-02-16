use pyo3::exceptions::PyException;
use pyo3::prelude::*;

// Define Python exception hierarchy
pyo3::create_exception!(
    _dpp,
    DppError,
    PyException,
    "Base exception for all dpp errors."
);
pyo3::create_exception!(_dpp, IoError, DppError, "I/O error.");
pyo3::create_exception!(
    _dpp,
    InvalidFormatError,
    DppError,
    "Invalid format: bad magic, corrupt data, or invalid headers."
);
pyo3::create_exception!(
    _dpp,
    FileNotFoundError,
    DppError,
    "File or partition not found."
);
pyo3::create_exception!(_dpp, DecompressionError, DppError, "Decompression failure.");
pyo3::create_exception!(
    _dpp,
    UnsupportedError,
    DppError,
    "Unsupported feature or format."
);

/// Register exception types on the module.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("DppError", m.py().get_type::<DppError>())?;
    m.add("IoError", m.py().get_type::<IoError>())?;
    m.add(
        "InvalidFormatError",
        m.py().get_type::<InvalidFormatError>(),
    )?;
    m.add("FileNotFoundError", m.py().get_type::<FileNotFoundError>())?;
    m.add(
        "DecompressionError",
        m.py().get_type::<DecompressionError>(),
    )?;
    m.add("UnsupportedError", m.py().get_type::<UnsupportedError>())?;
    Ok(())
}

/// Convert a `dpp::DppError` into the appropriate Python exception.
pub fn to_pyerr(err: dpp::DppError) -> PyErr {
    use dpp::DppError as E;

    match &err {
        // I/O errors
        E::Io(_) => IoError::new_err(err.to_string()),

        // File/partition not found
        E::FileNotFound(_) | E::NoHfsPartition | E::NoApfsPartition | E::NoFilesystemPartition => {
            FileNotFoundError::new_err(err.to_string())
        }

        // DMG sub-errors: dispatch by variant
        E::Dmg(dmg_err) => dmg_to_pyerr(dmg_err, &err),

        // HFS+ sub-errors
        E::Hfs(hfs_err) => hfs_to_pyerr(hfs_err, &err),

        // APFS sub-errors
        E::Apfs(apfs_err) => apfs_to_pyerr(apfs_err, &err),

        // XAR sub-errors
        E::Xar(xar_err) => xar_to_pyerr(xar_err, &err),

        // PBZX sub-errors
        E::Pbzx(pbzx_err) => pbzx_to_pyerr(pbzx_err, &err),
    }
}

fn dmg_to_pyerr(dmg_err: &dpp::udif::DppError, top: &dpp::DppError) -> PyErr {
    use dpp::udif::DppError as D;
    match dmg_err {
        D::Io(_) => IoError::new_err(top.to_string()),
        D::InvalidMagic
        | D::InvalidKolyHeader(_)
        | D::InvalidPlist(_)
        | D::InvalidBlockMap(_)
        | D::Base64Error(_)
        | D::XmlError(_)
        | D::InvalidPath(_)
        | D::ChecksumMismatch { .. } => InvalidFormatError::new_err(top.to_string()),
        D::Decompression(_) | D::Compression(_) => DecompressionError::new_err(top.to_string()),
        D::UnsupportedCompression(_) | D::Unsupported(_) => {
            UnsupportedError::new_err(top.to_string())
        }
        D::FileNotFound(_) => FileNotFoundError::new_err(top.to_string()),
    }
}

fn hfs_to_pyerr(hfs_err: &dpp::hfsplus::HfsPlusError, top: &dpp::DppError) -> PyErr {
    use dpp::hfsplus::HfsPlusError as H;
    match hfs_err {
        H::Io(_) => IoError::new_err(top.to_string()),
        H::FileNotFound(_) => FileNotFoundError::new_err(top.to_string()),
        H::InvalidSignature(_) | H::InvalidBTree(_) | H::NotADirectory(_) | H::CorruptedData(_) => {
            InvalidFormatError::new_err(top.to_string())
        }
        H::UnsupportedVersion(_) => UnsupportedError::new_err(top.to_string()),
    }
}

fn apfs_to_pyerr(apfs_err: &dpp::apfs::ApfsError, top: &dpp::DppError) -> PyErr {
    use dpp::apfs::ApfsError as A;
    match apfs_err {
        A::Io(_) => IoError::new_err(top.to_string()),
        A::FileNotFound(_) => FileNotFoundError::new_err(top.to_string()),
        A::InvalidMagic(_)
        | A::InvalidChecksum
        | A::InvalidBTree(_)
        | A::NotADirectory(_)
        | A::CorruptedData(_)
        | A::NoVolume => InvalidFormatError::new_err(top.to_string()),
    }
}

fn xar_to_pyerr(xar_err: &dpp::xara::XarError, top: &dpp::DppError) -> PyErr {
    use dpp::xara::XarError as X;
    match xar_err {
        X::Io(_) => IoError::new_err(top.to_string()),
        X::FileNotFound(_) => FileNotFoundError::new_err(top.to_string()),
        X::InvalidMagic(_) | X::InvalidToc(_) | X::XmlParse(_) => {
            InvalidFormatError::new_err(top.to_string())
        }
        X::DecompressionFailed(_) => DecompressionError::new_err(top.to_string()),
        X::UnsupportedEncoding(_) => UnsupportedError::new_err(top.to_string()),
    }
}

fn pbzx_to_pyerr(pbzx_err: &dpp::pbzx::PbzxError, top: &dpp::DppError) -> PyErr {
    use dpp::pbzx::PbzxError as P;
    match pbzx_err {
        P::Io(_) => IoError::new_err(top.to_string()),
        P::FileNotFound(_) => FileNotFoundError::new_err(top.to_string()),
        P::InvalidMagic(_) | P::InvalidChunk { .. } | P::InvalidCpio(_) | P::InvalidPath(_) => {
            InvalidFormatError::new_err(top.to_string())
        }
        P::Decompression(_) | P::Compression(_) => DecompressionError::new_err(top.to_string()),
        P::UnexpectedEof(_) => IoError::new_err(top.to_string()),
        P::Unsupported(_) => UnsupportedError::new_err(top.to_string()),
    }
}
