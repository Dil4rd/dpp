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
    /// `base_path/` are extracted. Pass `"/"` to extract everything.
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_parse_header_valid() {
        // Build a 28-byte XAR header (big-endian)
        let mut buf = Vec::new();
        buf.extend_from_slice(&0x78617221u32.to_be_bytes()); // magic "xar!"
        buf.extend_from_slice(&28u16.to_be_bytes()); // header_size
        buf.extend_from_slice(&1u16.to_be_bytes()); // version
        buf.extend_from_slice(&100u64.to_be_bytes()); // toc_compressed_len
        buf.extend_from_slice(&200u64.to_be_bytes()); // toc_uncompressed_len
        buf.extend_from_slice(&1u32.to_be_bytes()); // checksum_algo = SHA1

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
        use flate2::Compression;
        use flate2::write::ZlibEncoder;
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

    /// Helper to build an in-memory XAR archive for extraction tests.
    fn build_test_xar(toc_xml: &[u8], heap: &[u8]) -> Vec<u8> {
        use flate2::Compression;
        use flate2::write::ZlibEncoder;
        use std::io::Write;

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
        xar_buf.extend_from_slice(heap);
        xar_buf
    }

    #[test]
    fn test_extract_all() {
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

        let xar_buf = build_test_xar(toc_xml, b"hello");
        let mut cursor = Cursor::new(&xar_buf);
        let mut archive = XarArchive::open(&mut cursor).unwrap();

        let tmp = tempfile::tempdir().unwrap();
        let stats = archive.extract_all(tmp.path()).unwrap();

        assert_eq!(stats.files, 1);
        assert_eq!(stats.dirs, 1);
        assert_eq!(stats.bytes, 5);
        assert!(tmp.path().join("hello.txt").exists());
        assert!(tmp.path().join("subdir").is_dir());
    }

    #[test]
    fn test_extract_path_filter() {
        // Two files at different paths: one inside "pkg/" and one outside
        let toc_xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<xar>
  <toc>
    <file id="1">
      <name>pkg</name>
      <type>directory</type>
      <file id="2">
        <name>Payload</name>
        <type>file</type>
        <data>
          <offset>0</offset>
          <length>7</length>
          <size>7</size>
          <encoding style="application/octet-stream"/>
        </data>
      </file>
    </file>
    <file id="3">
      <name>Distribution</name>
      <type>file</type>
      <data>
        <offset>7</offset>
        <length>4</length>
        <size>4</size>
        <encoding style="application/octet-stream"/>
      </data>
    </file>
  </toc>
</xar>"#;

        let xar_buf = build_test_xar(toc_xml, b"payloadtest");
        let mut cursor = Cursor::new(&xar_buf);
        let mut archive = XarArchive::open(&mut cursor).unwrap();

        let tmp = tempfile::tempdir().unwrap();
        let stats = archive.extract_path("pkg", tmp.path()).unwrap();

        assert_eq!(stats.files, 1);
        assert_eq!(stats.dirs, 1);
        assert!(tmp.path().join("pkg/Payload").exists());
        // Distribution should NOT be extracted
        assert!(!tmp.path().join("Distribution").exists());
    }

    #[test]
    fn test_extract_skips_symlinks() {
        let toc_xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<xar>
  <toc>
    <file id="1">
      <name>real.txt</name>
      <type>file</type>
      <data>
        <offset>0</offset>
        <length>4</length>
        <size>4</size>
        <encoding style="application/octet-stream"/>
      </data>
    </file>
    <file id="2">
      <name>link.txt</name>
      <type>symlink</type>
    </file>
  </toc>
</xar>"#;

        let xar_buf = build_test_xar(toc_xml, b"data");
        let mut cursor = Cursor::new(&xar_buf);
        let mut archive = XarArchive::open(&mut cursor).unwrap();

        let tmp = tempfile::tempdir().unwrap();
        let stats = archive.extract_all(tmp.path()).unwrap();

        assert_eq!(stats.files, 1);
        assert_eq!(stats.symlinks_skipped, 1);
        assert!(tmp.path().join("real.txt").exists());
        assert!(!tmp.path().join("link.txt").exists());
    }
}
