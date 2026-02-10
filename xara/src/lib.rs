pub mod error;
pub mod header;
pub mod toc;
pub mod heap;
pub mod pkg;

pub use error::{XarError, Result};
pub use header::XarHeader;
pub use toc::{XarFile, XarFileType, XarFileData};
pub use pkg::PkgReader;

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
}
