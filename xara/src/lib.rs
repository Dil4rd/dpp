pub mod error;
pub mod header;
pub mod heap;
pub mod pkg;
pub mod toc;

pub use error::{Result, XarError};
pub use header::XarHeader;
pub use pkg::PkgReader;
pub use toc::{XarFile, XarFileData, XarFileType};

#[cfg(feature = "extract")]
mod extract;
#[cfg(feature = "extract")]
pub use extract::ExtractStats;

use std::io::{Read, Seek, Write};

/// XAR archive reader
pub struct XarArchive<R: Read + Seek> {
    reader: R,
    pub(crate) header: XarHeader,
    pub(crate) files: Vec<XarFile>,
    pub(crate) heap_offset: u64,
}

impl<R: Read + Seek> XarArchive<R> {
    /// Open and parse a XAR archive
    pub fn open(mut reader: R) -> Result<Self> {
        let header = header::parse_header(&mut reader)?;
        let (files, heap_offset) = toc::parse_toc(&mut reader, &header)?;
        Ok(XarArchive {
            reader,
            header,
            files,
            heap_offset,
        })
    }

    /// Access the parsed header
    pub fn header(&self) -> &XarHeader {
        &self.header
    }

    /// Get all files in the archive
    pub fn files(&self) -> &[XarFile] {
        &self.files
    }

    /// Find a file by path
    pub fn find(&self, path: &str) -> Option<&XarFile> {
        toc::find_by_path(&self.files, path)
    }

    /// Read a file entry into memory
    pub fn read_file(&mut self, file: &XarFile) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        self.read_file_to(file, &mut buf)?;
        Ok(buf)
    }

    /// Stream a file entry to a writer
    pub fn read_file_to<W: Write>(&mut self, file: &XarFile, writer: W) -> Result<u64> {
        heap::read_entry(&mut self.reader, self.heap_offset, file, writer)
    }

    /// Extract all files to a directory.
    ///
    /// Returns statistics about what was extracted. Symlinks are skipped
    /// (counted in [`ExtractStats::symlinks_skipped`]).
    #[cfg(feature = "extract")]
    pub fn extract_all<P: AsRef<std::path::Path>>(&mut self, dest: P) -> Result<ExtractStats> {
        self.extract_path("/", dest)
    }

    /// Extract files under `base_path` to a directory.
    ///
    /// Only entries whose path equals `base_path` or starts with
    /// `base_path/` are extracted. The `base_path` prefix is stripped from
    /// output paths so only the relative remainder appears under `dest`.
    /// Pass `"/"` to extract everything (no stripping).
    /// Symlinks are skipped (counted in [`ExtractStats::symlinks_skipped`]).
    #[cfg(feature = "extract")]
    pub fn extract_path<P: AsRef<std::path::Path>>(
        &mut self,
        base_path: &str,
        dest: P,
    ) -> Result<ExtractStats> {
        extract::extract_inner(self, base_path, dest.as_ref())
    }
}
