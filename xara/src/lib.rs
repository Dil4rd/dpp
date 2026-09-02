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
    fn test_xar_symlink_target_roundtrip() {
        let toc_xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<xar><toc><file id="1">
  <link type="file"><![CDATA[ ../A&B ]]></link>
  <type>symlink</type><name>link</name>
</file></toc></xar>"#;
        let xar_buf = build_test_xar(toc_xml, &[]);
        let mut cursor = Cursor::new(&xar_buf);
        let archive = XarArchive::open(&mut cursor).unwrap();

        let link = archive.find("link").unwrap();
        assert_eq!(link.file_type, XarFileType::Symlink);
        assert_eq!(link.link.as_deref(), Some(" ../A&B "));
    }

    #[test]
    fn test_xar_rejects_declared_toc_length_mismatch() {
        let toc_xml = br#"<xar><toc></toc></xar>"#;
        for declared_len in [toc_xml.len() - 1, toc_xml.len() + 1] {
            let mut xar_buf = build_test_xar(toc_xml, &[]);
            xar_buf[16..24].copy_from_slice(&u64::try_from(declared_len).unwrap().to_be_bytes());
            assert!(matches!(
                XarArchive::open(Cursor::new(&xar_buf)),
                Err(XarError::InvalidToc(_))
            ));
        }
    }

    #[test]
    fn test_xar_rejects_truncated_declared_toc_extent() {
        let toc_xml = br#"<xar><toc></toc></xar>"#;
        let mut xar_buf = build_test_xar(toc_xml, &[]);
        let declared_len = u64::from_be_bytes(xar_buf[8..16].try_into().unwrap()) + 1;
        xar_buf[8..16].copy_from_slice(&declared_len.to_be_bytes());
        assert!(matches!(
            XarArchive::open(Cursor::new(&xar_buf)),
            Err(XarError::InvalidToc(_))
        ));
    }

    #[test]
    fn test_xar_rejects_heap_entry_offset_overflow() {
        let toc_xml = br#"<xar><toc><file id="1">
  <name>overflow</name><type>file</type>
  <data><offset>18446744073709551615</offset><length>0</length><size>0</size></data>
</file></toc></xar>"#;
        let xar_buf = build_test_xar(toc_xml, &[]);
        let mut archive = XarArchive::open(Cursor::new(&xar_buf)).unwrap();
        let file = archive.find("overflow").unwrap().clone();
        assert!(matches!(
            archive.read_file(&file),
            Err(XarError::InvalidToc(_))
        ));
    }

    #[test]
    fn test_xar_rejects_raw_entry_size_mismatch() {
        let toc_xml = br#"<xar><toc><file id="1">
  <name>mismatch</name><type>file</type>
  <data><offset>0</offset><length>5</length><size>4</size></data>
</file></toc></xar>"#;
        let xar_buf = build_test_xar(toc_xml, b"hello");
        let mut archive = XarArchive::open(Cursor::new(&xar_buf)).unwrap();
        let file = archive.find("mismatch").unwrap().clone();
        assert!(matches!(
            archive.read_file(&file),
            Err(XarError::InvalidToc(_))
        ));
    }

    #[test]
    fn test_xar_streams_compressed_entries_and_checks_decoded_size() {
        use flate2::Compression;
        use flate2::write::{GzEncoder, ZlibEncoder};
        use std::io::{self, Write};

        struct RejectWrites;

        impl Write for RejectWrites {
            fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
                Err(io::Error::other("writer rejected data"))
            }

            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        let mut zlib_encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        zlib_encoder.write_all(b"hello").unwrap();
        let zlib_heap = zlib_encoder.finish().unwrap();

        let mut gzip_encoder = GzEncoder::new(Vec::new(), Compression::default());
        gzip_encoder.write_all(b"hello").unwrap();
        let gzip_heap = gzip_encoder.finish().unwrap();

        for (encoding, heap) in [
            ("application/zlib", zlib_heap.as_slice()),
            ("application/x-gzip", gzip_heap.as_slice()),
        ] {
            for (declared_size, should_succeed) in [(5, true), (4, false)] {
                let toc_xml = format!(
                    "<xar><toc><file id=\"1\"><name>payload</name><type>file</type>\
                     <data><offset>0</offset><length>{}</length><size>{declared_size}</size>\
                     <encoding style=\"{encoding}\"/></data></file></toc></xar>",
                    heap.len()
                );
                let xar_buf = build_test_xar(toc_xml.as_bytes(), heap);
                let mut archive = XarArchive::open(Cursor::new(&xar_buf)).unwrap();
                let file = archive.find("payload").unwrap().clone();
                let mut output = Vec::new();
                let result = archive.read_file_to(&file, &mut output);
                if should_succeed {
                    assert_eq!(result.unwrap(), 5);
                    assert_eq!(output, b"hello");
                } else {
                    assert!(matches!(result, Err(XarError::DecompressionFailed(_))));
                    assert_eq!(output, b"hell");
                }
            }
        }

        let toc_xml = format!(
            "<xar><toc><file id=\"1\"><name>payload</name><type>file</type>\
             <data><offset>0</offset><length>{}</length><size>5</size>\
             <encoding style=\"application/zlib\"/></data></file></toc></xar>",
            zlib_heap.len()
        );
        let xar_buf = build_test_xar(toc_xml.as_bytes(), &zlib_heap);
        let mut archive = XarArchive::open(Cursor::new(&xar_buf)).unwrap();
        let file = archive.find("payload").unwrap().clone();
        assert!(matches!(
            archive.read_file_to(&file, RejectWrites),
            Err(XarError::Io(_))
        ));
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
        assert_eq!(stats.dirs, 1); // pkg dir itself (base)
        // Base prefix is stripped: pkg/Payload → Payload
        assert!(tmp.path().join("Payload").exists());
        assert!(!tmp.path().join("pkg").exists());
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
      <link type="file">real.txt</link>
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

    #[test]
    fn test_open_and_extract_base64_encoded_name() {
        use base64::Engine;

        let name = "こんにちは.txt";
        let encoded_name = base64::engine::general_purpose::STANDARD.encode(name.as_bytes());
        let toc_xml = format!(
            "<xar><toc><file id=\"1\"><name enctype=\"base64\">{encoded_name}</name><type>file</type><data><offset>0</offset><length>7</length><size>7</size><encoding style=\"application/octet-stream\"/></data></file></toc></xar>"
        );

        let xar_buf = build_test_xar(toc_xml.as_bytes(), b"payload");
        let mut archive = XarArchive::open(Cursor::new(&xar_buf)).unwrap();

        let file = archive
            .find(name)
            .expect("decoded name should be searchable")
            .clone();
        assert_eq!(file.name, name);
        assert_eq!(archive.read_file(&file).unwrap(), b"payload");

        let tmp = tempfile::tempdir().unwrap();
        let stats = archive.extract_all(tmp.path()).unwrap();
        assert_eq!(stats.files, 1);
        assert_eq!(std::fs::read(tmp.path().join(name)).unwrap(), b"payload");
    }

    #[test]
    fn test_extract_rejects_base64_name_decoding_to_traversal() {
        // "Li4v...cGFzc3dk" is the base64 encoding of "../../etc/passwd". A
        // maliciously crafted XAR could set enctype="base64" on a <name> to
        // smuggle a traversal payload through decoding; sanitize_path must
        // still catch it post-decode, same as it does for a literal name.
        let toc_xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<xar>
  <toc>
    <file id="1">
      <name enctype="base64">Li4vLi4vZXRjL3Bhc3N3ZA==</name>
      <type>file</type>
      <data>
        <offset>0</offset>
        <length>4</length>
        <size>4</size>
        <encoding style="application/octet-stream"/>
      </data>
    </file>
  </toc>
</xar>"#;

        let xar_buf = build_test_xar(toc_xml, b"data");
        let mut cursor = Cursor::new(&xar_buf);
        let mut archive = XarArchive::open(&mut cursor).unwrap();

        let tmp = tempfile::tempdir().unwrap();
        let result = archive.extract_all(tmp.path());

        assert!(
            matches!(result, Err(XarError::InvalidPath(_))),
            "expected traversal rejection, got {result:?}"
        );
    }
}
