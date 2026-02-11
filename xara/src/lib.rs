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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_parse_header_valid() {
        // Build a 28-byte XAR header (big-endian)
        let mut buf = Vec::new();
        buf.extend_from_slice(&0x78617221u32.to_be_bytes()); // magic "xar!"
        buf.extend_from_slice(&28u16.to_be_bytes());         // header_size
        buf.extend_from_slice(&1u16.to_be_bytes());          // version
        buf.extend_from_slice(&100u64.to_be_bytes());        // toc_compressed_len
        buf.extend_from_slice(&200u64.to_be_bytes());        // toc_uncompressed_len
        buf.extend_from_slice(&1u32.to_be_bytes());          // checksum_algo = SHA1

        let mut cursor = Cursor::new(&buf);
        let hdr = header::parse_header(&mut cursor).unwrap();

        assert_eq!(hdr.magic, 0x78617221);
        assert_eq!(hdr.header_size, 28);
        assert_eq!(hdr.version, 1);
        assert_eq!(hdr.toc_compressed_len, 100);
        assert_eq!(hdr.toc_uncompressed_len, 200);
        assert_eq!(hdr.checksum_algo, header::ChecksumAlgo::Sha1);
    }

    #[test]
    fn test_parse_header_invalid_magic() {
        let mut buf = Vec::new();
        buf.extend_from_slice(&0xDEADBEEFu32.to_be_bytes());
        buf.extend_from_slice(&28u16.to_be_bytes());
        buf.extend_from_slice(&1u16.to_be_bytes());
        buf.extend_from_slice(&0u64.to_be_bytes());
        buf.extend_from_slice(&0u64.to_be_bytes());
        buf.extend_from_slice(&0u32.to_be_bytes());

        let mut cursor = Cursor::new(&buf);
        let result = header::parse_header(&mut cursor);
        assert!(matches!(result, Err(XarError::InvalidMagic(0xDEADBEEF))));
    }

    #[test]
    fn test_xar_roundtrip() {
        // Build a minimal in-memory XAR: header + zlib-compressed TOC + heap
        use flate2::write::ZlibEncoder;
        use flate2::Compression;
        use std::io::Write;

        let toc_xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<xar>
  <toc>
    <file id="1">
      <name>hello.txt</name>
      <type>file</type>
      <data>
        <offset>0</offset>
        <length>5</length>
        <size>5</size>
        <encoding style="application/octet-stream"/>
      </data>
    </file>
    <file id="2">
      <name>subdir</name>
      <type>directory</type>
    </file>
  </toc>
</xar>"#;

        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(toc_xml).unwrap();
        let compressed_toc = encoder.finish().unwrap();

        let mut xar_buf = Vec::new();
        xar_buf.extend_from_slice(&0x78617221u32.to_be_bytes());
        xar_buf.extend_from_slice(&28u16.to_be_bytes());
        xar_buf.extend_from_slice(&1u16.to_be_bytes());
        xar_buf.extend_from_slice(&(compressed_toc.len() as u64).to_be_bytes());
        xar_buf.extend_from_slice(&(toc_xml.len() as u64).to_be_bytes());
        xar_buf.extend_from_slice(&0u32.to_be_bytes());
        xar_buf.extend_from_slice(&compressed_toc);
        xar_buf.extend_from_slice(b"hello"); // heap data

        let mut cursor = Cursor::new(&xar_buf);
        let mut archive = XarArchive::open(&mut cursor).unwrap();

        assert_eq!(archive.files().len(), 2);

        let file = archive.find("hello.txt").unwrap();
        assert_eq!(file.name, "hello.txt");
        assert_eq!(file.file_type, XarFileType::File);
        assert!(file.data.is_some());

        let dir = archive.find("subdir").unwrap();
        assert_eq!(dir.name, "subdir");
        assert_eq!(dir.file_type, XarFileType::Directory);

        let file_clone = file.clone();
        let data = archive.read_file(&file_clone).unwrap();
        assert_eq!(&data, b"hello");
    }
}
