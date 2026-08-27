use flate2::read::ZlibDecoder;
use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use std::io::{self, Read};

use crate::error::{Result, XarError};
use crate::header::XarHeader;

/// File type in the archive
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XarFileType {
    File,
    Directory,
    Symlink,
}

/// Data descriptor for a file entry
#[derive(Debug, Clone)]
pub struct XarFileData {
    /// Offset into the heap (relative to heap start)
    pub offset: u64,
    /// Compressed length in heap
    pub length: u64,
    /// Uncompressed size
    pub size: u64,
    /// Encoding style (e.g. "application/x-gzip", "application/octet-stream")
    pub encoding: String,
    /// Extracted checksum (if any)
    pub extracted_checksum: Option<String>,
    /// Archived checksum (if any)
    pub archived_checksum: Option<String>,
}

/// A file entry from the TOC
#[derive(Debug, Clone)]
pub struct XarFile {
    /// File ID from the TOC
    pub id: u64,
    /// File name (just the last component)
    pub name: String,
    /// Full path from root
    pub path: String,
    /// Type of entry
    pub file_type: XarFileType,
    /// Symlink target text from the TOC `<link>` element. Only present for
    /// `XarFileType::Symlink` entries.
    pub link: Option<String>,
    /// Data descriptor (None for directories)
    pub data: Option<XarFileData>,
    /// Child file indices (for directories)
    pub children: Vec<usize>,
    /// Parent index (None for root-level entries)
    pub parent: Option<usize>,
}

/// Parse the TOC from a XAR archive.
/// Returns (files, heap_offset).
pub fn parse_toc<R: Read>(reader: &mut R, header: &XarHeader) -> Result<(Vec<XarFile>, u64)> {
    let expected_len = usize::try_from(header.toc_uncompressed_len)
        .map_err(|_| XarError::InvalidToc("declared uncompressed TOC length does not fit in memory".to_string()))?;
    let decode_limit = header
        .toc_uncompressed_len
        .checked_add(1)
        .ok_or_else(|| XarError::InvalidToc("declared uncompressed TOC length is too large".to_string()))?;

    // Bound the decoder to the declared TOC extent so buffering cannot consume
    // heap bytes. Drain any unused bytes to leave the reader at the heap.
    let mut decoder = ZlibDecoder::new(reader.take(header.toc_compressed_len));
    let mut xml_data = Vec::new();
    decoder
        .by_ref()
        .take(decode_limit)
        .read_to_end(&mut xml_data)
        .map_err(|e| XarError::DecompressionFailed(format!("TOC zlib: {}", e)))?;
    let mut compressed_extent = decoder.into_inner();
    io::copy(&mut compressed_extent, &mut io::sink())?;
    if compressed_extent.limit() != 0 {
        return Err(XarError::InvalidToc(
            "archive ended before the declared compressed TOC extent".to_string(),
        ));
    }
    if xml_data.len() != expected_len {
        return Err(XarError::InvalidToc(format!(
            "declared uncompressed TOC length {} does not match decoded length {}",
            header.toc_uncompressed_len,
            xml_data.len()
        )));
    }

    let files = parse_toc_xml(&xml_data)?;
    let heap_offset = u64::from(header.header_size)
        .checked_add(header.toc_compressed_len)
        .ok_or_else(|| XarError::InvalidToc("TOC extent overflows the archive address space".to_string()))?;

    Ok((files, heap_offset))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ElementContext {
    Xar,
    Toc,
    File,
    FileData,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CaptureField {
    Name,
    FileType,
    Link,
    DataOffset,
    DataLength,
    DataSize,
    ExtractedChecksum,
    ArchivedChecksum,
    Encoding,
}

struct TextCapture {
    field: CaptureField,
    depth: usize,
    value: String,
}

#[derive(Default)]
struct FileDataBuilder {
    offset: Option<u64>,
    length: Option<u64>,
    size: Option<u64>,
    encoding: Option<String>,
    extracted_checksum: Option<String>,
    archived_checksum: Option<String>,
}

impl FileDataBuilder {
    fn build(self) -> Result<XarFileData> {
        let offset = self
            .offset
            .ok_or_else(|| XarError::XmlParse("file <data> is missing <offset>".to_string()))?;
        let length = self
            .length
            .ok_or_else(|| XarError::XmlParse("file <data> is missing <length>".to_string()))?;
        let size = self
            .size
            .ok_or_else(|| XarError::XmlParse("file <data> is missing <size>".to_string()))?;
        Ok(XarFileData {
            offset,
            length,
            size,
            encoding: self
                .encoding
                .unwrap_or_else(|| "application/octet-stream".to_string()),
            extracted_checksum: self.extracted_checksum,
            archived_checksum: self.archived_checksum,
        })
    }
}

/// Internal state for a file being parsed.
struct FileBuilder {
    id: u64,
    name: String,
    file_type: Option<String>,
    link: Option<String>,
    children: Vec<usize>,
    data: Option<FileDataBuilder>,
    capture: Option<TextCapture>,
    assigned_fields: Vec<CaptureField>,
}

impl FileBuilder {
    fn begin_capture(&mut self, field: CaptureField, depth: usize) -> Result<()> {
        if self.capture.is_some() {
            return Err(XarError::XmlParse(
                "nested scalar metadata elements are not supported".to_string(),
            ));
        }
        self.capture = Some(TextCapture {
            field,
            depth,
            value: String::new(),
        });
        Ok(())
    }

    fn append_text(&mut self, text: &str) {
        if let Some(capture) = &mut self.capture {
            capture.value.push_str(text);
        }
    }

    fn finish_capture(&mut self, depth: usize) -> Result<()> {
        let Some(capture) = self.capture.take_if(|capture| capture.depth == depth) else {
            return Ok(());
        };
        self.assign_text(capture.field, capture.value)
    }

    fn assign_text(&mut self, field: CaptureField, value: String) -> Result<()> {
        if self.assigned_fields.contains(&field) {
            return Err(XarError::XmlParse(format!(
                "duplicate direct <{}> field in <file>",
                field.element_name()
            )));
        }
        self.assigned_fields.push(field);
        match field {
            CaptureField::Name => self.name = value,
            CaptureField::FileType => self.file_type = Some(value.trim().to_string()),
            CaptureField::Link => self.link = Some(value),
            CaptureField::DataOffset => {
                self.data_mut()?.offset = Some(parse_u64_field("offset", &value)?);
            }
            CaptureField::DataLength => {
                self.data_mut()?.length = Some(parse_u64_field("length", &value)?);
            }
            CaptureField::DataSize => {
                self.data_mut()?.size = Some(parse_u64_field("size", &value)?);
            }
            CaptureField::ExtractedChecksum => {
                self.data_mut()?.extracted_checksum = Some(value.trim().to_string());
            }
            CaptureField::ArchivedChecksum => {
                self.data_mut()?.archived_checksum = Some(value.trim().to_string());
            }
            CaptureField::Encoding => unreachable!("encoding is assigned from its style attribute"),
        }
        Ok(())
    }

    fn assign_encoding(&mut self, value: String) -> Result<()> {
        if self.assigned_fields.contains(&CaptureField::Encoding) {
            return Err(XarError::XmlParse(
                "duplicate direct <encoding> field in file <data>".to_string(),
            ));
        }
        self.assigned_fields.push(CaptureField::Encoding);
        self.data_mut()?.encoding = Some(value);
        Ok(())
    }

    fn data_mut(&mut self) -> Result<&mut FileDataBuilder> {
        self.data.as_mut().ok_or_else(|| {
            XarError::XmlParse(
                "file data field appeared outside the direct <file>/<data> block".to_string(),
            )
        })
    }
}

impl CaptureField {
    fn element_name(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::FileType => "type",
            Self::Link => "link",
            Self::DataOffset => "offset",
            Self::DataLength => "length",
            Self::DataSize => "size",
            Self::ExtractedChecksum => "extracted-checksum",
            Self::ArchivedChecksum => "archived-checksum",
            Self::Encoding => "encoding",
        }
    }
}

fn parse_u64_field(name: &str, value: &str) -> Result<u64> {
    value
        .trim()
        .parse()
        .map_err(|error| XarError::XmlParse(format!("invalid <{name}> value {value:?}: {error}")))
}

fn current_file(stack: &mut [FileBuilder]) -> Result<&mut FileBuilder> {
    stack.last_mut().ok_or_else(|| {
        XarError::XmlParse("file metadata appeared without a current <file>".to_string())
    })
}

fn ensure_no_active_capture(stack: &[FileBuilder]) -> Result<()> {
    if stack.last().is_some_and(|file| file.capture.is_some()) {
        return Err(XarError::XmlParse(
            "nested element inside scalar file metadata".to_string(),
        ));
    }
    Ok(())
}

fn attribute_value(
    reader: &Reader<&[u8]>,
    element: &BytesStart<'_>,
    name: &[u8],
) -> Result<Option<String>> {
    let mut value = None;
    for attribute in element.attributes() {
        let attribute = attribute
            .map_err(|error| XarError::XmlParse(format!("invalid XML attribute: {error}")))?;
        if attribute.key.as_ref() == name {
            if value.is_some() {
                return Err(XarError::XmlParse(format!("duplicate XML attribute {:?}", String::from_utf8_lossy(name))));
            }
            let decoded = attribute
                .decode_and_unescape_value(reader.decoder())
                .map_err(|error| {
                    XarError::XmlParse(format!("invalid XML attribute value: {error}"))
                })?;
            value = Some(decoded.into_owned());
        }
    }
    Ok(value)
}

fn parse_toc_xml(xml: &[u8]) -> Result<Vec<XarFile>> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(false);

    let mut files: Vec<XarFile> = Vec::new();
    let mut buf = Vec::new();
    let mut seen_xar = false;
    let mut seen_toc = false;

    // Stack of files being parsed. Children are nested inside parents in XAR TOC.
    // When </file> is encountered, the file is popped and added to `files`.
    // Children are finalized before their parents, so child indices are known.
    let mut stack: Vec<FileBuilder> = Vec::new();

    // The typed element stack makes field eligibility structural: only a
    // direct file child can provide name/type/link, and only the direct data
    // child of that file can provide its payload descriptor.
    let mut element_stack: Vec<ElementContext> = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                ensure_no_active_capture(&stack)?;
                let parent = element_stack.last().copied();
                let depth = element_stack.len();
                let context = match e.name().as_ref() {
                    b"xar" if parent.is_none() && !seen_xar => {
                        seen_xar = true;
                        ElementContext::Xar
                    }
                    b"toc" if parent == Some(ElementContext::Xar) && !seen_toc => {
                        seen_toc = true;
                        ElementContext::Toc
                    }
                    b"xar" | b"toc" => {
                        return Err(XarError::XmlParse("duplicate or misplaced XAR document element".to_string()));
                    }
                    b"file" if matches!(parent, Some(ElementContext::Toc | ElementContext::File)) => {
                        let id = match attribute_value(&reader, e, b"id")? {
                            Some(value) => parse_u64_field("file id", &value)?,
                            None => 0,
                        };

                        stack.push(FileBuilder {
                            id,
                            name: String::new(),
                            file_type: None,
                            link: None,
                            children: Vec::new(),
                            data: None,
                            capture: None,
                            assigned_fields: Vec::new(),
                        });
                        ElementContext::File
                    }
                    b"data" if parent == Some(ElementContext::File) => {
                        let file = stack.last_mut().ok_or_else(|| {
                            XarError::XmlParse(
                                "<data> appeared without a current <file>".to_string(),
                            )
                        })?;
                        if file.data.is_some() {
                            return Err(XarError::XmlParse(
                                "duplicate direct <data> block in <file>".to_string(),
                            ));
                        }
                        file.data = Some(FileDataBuilder::default());
                        ElementContext::FileData
                    }
                    b"name" if parent == Some(ElementContext::File) => {
                        current_file(&mut stack)?.begin_capture(CaptureField::Name, depth)?;
                        ElementContext::Other
                    }
                    b"type" if parent == Some(ElementContext::File) => {
                        current_file(&mut stack)?.begin_capture(CaptureField::FileType, depth)?;
                        ElementContext::Other
                    }
                    b"link" if parent == Some(ElementContext::File) => {
                        current_file(&mut stack)?.begin_capture(CaptureField::Link, depth)?;
                        ElementContext::Other
                    }
                    b"offset" if parent == Some(ElementContext::FileData) => {
                        current_file(&mut stack)?.begin_capture(CaptureField::DataOffset, depth)?;
                        ElementContext::Other
                    }
                    b"length" if parent == Some(ElementContext::FileData) => {
                        current_file(&mut stack)?.begin_capture(CaptureField::DataLength, depth)?;
                        ElementContext::Other
                    }
                    b"size" if parent == Some(ElementContext::FileData) => {
                        current_file(&mut stack)?.begin_capture(CaptureField::DataSize, depth)?;
                        ElementContext::Other
                    }
                    b"extracted-checksum" if parent == Some(ElementContext::FileData) => {
                        current_file(&mut stack)?
                            .begin_capture(CaptureField::ExtractedChecksum, depth)?;
                        ElementContext::Other
                    }
                    b"archived-checksum" if parent == Some(ElementContext::FileData) => {
                        current_file(&mut stack)?
                            .begin_capture(CaptureField::ArchivedChecksum, depth)?;
                        ElementContext::Other
                    }
                    b"encoding" if parent == Some(ElementContext::FileData) => {
                        let style = attribute_value(&reader, e, b"style")?.ok_or_else(|| {
                            XarError::XmlParse("file data <encoding> is missing its style attribute".to_string())
                        })?;
                        current_file(&mut stack)?.assign_encoding(style)?;
                        ElementContext::Other
                    }
                    _ if parent.is_none() => {
                        return Err(XarError::XmlParse("the TOC document root must be <xar>".to_string()));
                    }
                    _ => ElementContext::Other,
                };
                element_stack.push(context);
            }
            Ok(Event::Empty(ref e)) => {
                ensure_no_active_capture(&stack)?;
                let parent = element_stack.last().copied();
                let field = match e.name().as_ref() {
                    b"xar" if parent.is_none() && !seen_xar => {
                        seen_xar = true;
                        None
                    }
                    b"toc" if parent == Some(ElementContext::Xar) && !seen_toc => {
                        seen_toc = true;
                        None
                    }
                    b"xar" | b"toc" => {
                        return Err(XarError::XmlParse("duplicate or misplaced XAR document element".to_string()));
                    }
                    b"file" if matches!(parent, Some(ElementContext::Toc | ElementContext::File)) => {
                        return Err(XarError::XmlParse("empty <file> is missing required metadata".to_string()));
                    }
                    b"data" if parent == Some(ElementContext::File) => {
                        return Err(XarError::XmlParse(
                            "file <data> is missing <offset>, <length>, and <size>".to_string(),
                        ));
                    }
                    b"name" if parent == Some(ElementContext::File) => Some(CaptureField::Name),
                    b"type" if parent == Some(ElementContext::File) => Some(CaptureField::FileType),
                    b"link" if parent == Some(ElementContext::File) => Some(CaptureField::Link),
                    b"offset" if parent == Some(ElementContext::FileData) => {
                        Some(CaptureField::DataOffset)
                    }
                    b"length" if parent == Some(ElementContext::FileData) => {
                        Some(CaptureField::DataLength)
                    }
                    b"size" if parent == Some(ElementContext::FileData) => {
                        Some(CaptureField::DataSize)
                    }
                    b"extracted-checksum" if parent == Some(ElementContext::FileData) => {
                        Some(CaptureField::ExtractedChecksum)
                    }
                    b"archived-checksum" if parent == Some(ElementContext::FileData) => {
                        Some(CaptureField::ArchivedChecksum)
                    }
                    b"encoding" if parent == Some(ElementContext::FileData) => {
                        let style = attribute_value(&reader, e, b"style")?.ok_or_else(|| {
                            XarError::XmlParse("file data <encoding> is missing its style attribute".to_string())
                        })?;
                        current_file(&mut stack)?.assign_encoding(style)?;
                        None
                    }
                    _ if parent.is_none() => {
                        return Err(XarError::XmlParse("the TOC document root must be <xar>".to_string()));
                    }
                    _ => None,
                };
                if let Some(field) = field {
                    current_file(&mut stack)?.assign_text(field, String::new())?;
                }
            }
            Ok(Event::Text(ref e)) => {
                let text = e
                    .unescape()
                    .map_err(|error| XarError::XmlParse(format!("invalid XML text: {error}")))?;
                if let Some(file) = stack.last_mut() {
                    file.append_text(&text);
                }
            }
            Ok(Event::CData(ref e)) => {
                let text = e
                    .decode()
                    .map_err(|error| XarError::XmlParse(format!("invalid CDATA text: {error}")))?;
                if let Some(file) = stack.last_mut() {
                    file.append_text(&text);
                }
            }
            Ok(Event::End(_)) => {
                let depth = element_stack.len().checked_sub(1).ok_or_else(|| {
                    XarError::XmlParse("closing element without an open element".to_string())
                })?;
                if let Some(file) = stack.last_mut() {
                    file.finish_capture(depth)?;
                }
                let context = element_stack.pop().ok_or_else(|| {
                    XarError::XmlParse("closing element without an open element".to_string())
                })?;
                match context {
                    ElementContext::Xar | ElementContext::Toc => {}
                    ElementContext::File => {
                        let builder = stack.pop().ok_or_else(|| {
                            XarError::XmlParse("closing <file> without a current file".to_string())
                        })?;
                        if builder.name.is_empty() {
                            return Err(XarError::XmlParse(
                                "file is missing a non-empty direct <name>".to_string(),
                            ));
                        }

                        let file_type = match builder.file_type.as_deref() {
                            Some("directory") => XarFileType::Directory,
                            Some("symlink") => XarFileType::Symlink,
                            Some("file") => XarFileType::File,
                            None => {
                                return Err(XarError::XmlParse("file is missing a direct <type>".to_string()));
                            }
                            Some(other) => {
                                return Err(XarError::XmlParse(format!(
                                    "unsupported file <type> value {other:?}"
                                )));
                            }
                        };
                        let link = match file_type {
                            XarFileType::Symlink => match builder.link {
                                Some(link) if !link.is_empty() => Some(link),
                                _ => {
                                    return Err(XarError::XmlParse("symlink is missing a non-empty direct <link>".to_string()));
                                }
                            },
                            XarFileType::File | XarFileType::Directory => None,
                        };
                        let data = builder.data.map(FileDataBuilder::build).transpose()?;

                        let file_idx = files.len();
                        if let Some(parent) = stack.last_mut() {
                            // Parent is not yet finalized, but we can record that
                            // this file is a child of whatever is on top of the stack
                            parent.children.push(file_idx);
                        }

                        files.push(XarFile {
                            id: builder.id,
                            name: builder.name,
                            path: String::new(),
                            file_type,
                            link,
                            data,
                            children: builder.children,
                            // Parent index is fixed up from the child lists below.
                            parent: None,
                        });
                    }
                    ElementContext::FileData | ElementContext::Other => {}
                }
            }
            Ok(Event::Eof) => {
                if !element_stack.is_empty() || !stack.is_empty() {
                    return Err(XarError::XmlParse("unexpected end of TOC XML".to_string()));
                }
                if !seen_xar || !seen_toc {
                    return Err(XarError::XmlParse("TOC XML must contain one <xar>/<toc> document".to_string()));
                }
                break;
            }
            Err(e) => return Err(XarError::XmlParse(format!("XML error: {}", e))),
            _ => {}
        }
        buf.clear();
    }

    // Fix up parent indices: iterate through files and set parent based on children lists
    for i in 0..files.len() {
        let children = files[i].children.clone();
        for &child_idx in &children {
            if child_idx < files.len() {
                files[child_idx].parent = Some(i);
            }
        }
    }

    let paths = (0..files.len())
        .map(|index| resolved_file_path(&files, index))
        .collect::<Result<Vec<_>>>()?;
    for (file, path) in files.iter_mut().zip(paths) {
        file.path = path;
    }

    Ok(files)
}

fn resolved_file_path(files: &[XarFile], index: usize) -> Result<String> {
    let mut components = Vec::new();
    let mut current = Some(index);
    let mut remaining = files.len().saturating_add(1);
    while let Some(file_index) = current {
        if remaining == 0 {
            return Err(XarError::XmlParse("cycle in XAR file parent graph".to_string()));
        }
        remaining -= 1;
        let file = files
            .get(file_index)
            .ok_or_else(|| XarError::XmlParse("invalid XAR file parent index".to_string()))?;
        components.push(file.name.as_str());
        current = file.parent;
    }
    components.reverse();
    Ok(components.join("/"))
}

/// Find a file by path in the flat file list
pub fn find_by_path<'a>(files: &'a [XarFile], path: &str) -> Option<&'a XarFile> {
    let path = path.trim_matches('/');
    files.iter().find(|f| f.path == path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_link_target_for_symlink_entry() {
        // Matches the real element order produced by macOS `xar`: <link> comes
        // before <type>/<name>.
        let xml = br#"<?xml version="1.0"?>
<xar><toc>
  <file id="1">
    <link type="file">../README.txt</link>
    <type>symlink</type>
    <name>readme-link.txt</name>
  </file>
</toc></xar>"#;

        let files = parse_toc_xml(xml).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].file_type, XarFileType::Symlink);
        assert_eq!(files[0].name, "readme-link.txt");
        assert_eq!(files[0].link.as_deref(), Some("../README.txt"));
    }

    #[test]
    fn non_symlink_entries_have_no_link_target() {
        let xml = br#"<?xml version="1.0"?>
<xar><toc>
  <file id="1">
    <name>plain.txt</name>
    <type>file</type>
    <link type="file">must-not-leak</link>
  </file>
</toc></xar>"#;

        let files = parse_toc_xml(xml).unwrap();
        assert_eq!(files[0].link, None);
    }

    #[test]
    fn ea_block_name_before_real_name_does_not_clobber_it() {
        // Real XAR TOCs (per macOS `xar`) emit <ea> — which carries its own
        // <name>, e.g. "com.apple.provenance" — before the file's real <type>/<name>.
        let xml = br#"<?xml version="1.0"?>
<xar><toc>
  <file id="1">
    <ea id="0">
      <length>19</length>
      <offset>20</offset>
      <size>11</size>
      <name>com.apple.provenance</name>
    </ea>
    <type>file</type>
    <name>real-name.txt</name>
  </file>
</toc></xar>"#;

        let files = parse_toc_xml(xml).unwrap();
        assert_eq!(files[0].name, "real-name.txt");
        assert_eq!(files[0].path, "real-name.txt");
    }

    #[test]
    fn ea_block_name_after_real_name_does_not_clobber_it() {
        // The inverse ordering: if a producer emits <ea> AFTER <type>/<name>
        // instead of before, the pre-fix parser (which tracked `current_tag`
        // for any tag anywhere inside <file>, regardless of nesting depth)
        // would let <ea>'s <name> overwrite the file's real name here, since
        // it was the last "name" tag seen. This must not happen.
        let xml = br#"<?xml version="1.0"?>
<xar><toc>
  <file id="1">
    <type>file</type>
    <name>real-name.txt</name>
    <ea id="0">
      <length>19</length>
      <offset>20</offset>
      <size>11</size>
      <name>com.apple.provenance</name>
    </ea>
  </file>
</toc></xar>"#;

        let files = parse_toc_xml(xml).unwrap();
        assert_eq!(files[0].name, "real-name.txt");
        assert_eq!(files[0].path, "real-name.txt");
    }

    #[test]
    fn ea_offset_length_size_do_not_leak_into_data_descriptor() {
        // <ea> carries its own <offset>/<length>/<size>, sitting alongside a
        // real <data> block with different values. The two must not mix.
        let xml = br#"<?xml version="1.0"?>
<xar><toc>
  <file id="1">
    <ea id="0">
      <length>19</length>
      <offset>999</offset>
      <size>11</size>
    </ea>
    <type>file</type>
    <name>payload.bin</name>
    <data>
      <offset>0</offset>
      <length>42</length>
      <size>42</size>
      <encoding style="application/octet-stream"/>
    </data>
  </file>
</toc></xar>"#;

        let files = parse_toc_xml(xml).unwrap();
        let data = files[0].data.as_ref().expect("data descriptor present");
        assert_eq!(data.offset, 0);
        assert_eq!(data.length, 42);
        assert_eq!(data.size, 42);
    }

    #[test]
    fn nested_directory_and_symlink_link_targets_are_scoped_per_file() {
        // A directory containing a symlink; the directory's own <name> must
        // not bleed into the child's fields or vice versa, and the full path
        // must be built from both ancestor names.
        let xml = br#"<?xml version="1.0"?>
<xar><toc>
  <file id="1">
    <type>directory</type>
    <name>dir</name>
    <file id="2">
      <link type="file">../target.txt</link>
      <type>symlink</type>
      <name>link.txt</name>
    </file>
  </file>
</toc></xar>"#;

        let files = parse_toc_xml(xml).unwrap();
        assert_eq!(files.len(), 2);
        let link_entry = files.iter().find(|f| f.name == "link.txt").unwrap();
        assert_eq!(link_entry.path, "dir/link.txt");
        assert_eq!(link_entry.link.as_deref(), Some("../target.txt"));
        let dir_entry = files.iter().find(|f| f.name == "dir").unwrap();
        assert_eq!(dir_entry.link, None);
    }

    #[test]
    fn preserves_exact_link_text_across_xml_event_types() {
        let cases: &[(&[u8], &str)] = &[
            (
                br#"<xar><toc><file id="1"><link type="file"> target </link><type>symlink</type><name>link</name></file></toc></xar>"#,
                " target ",
            ),
            (
                br#"<xar><toc><file id="1"><link type="file"><![CDATA[../A&B]]></link><type>symlink</type><name>link</name></file></toc></xar>"#,
                "../A&B",
            ),
            (
                br#"<xar><toc><file id="1"><link type="file">foo<!-- split -->bar</link><type>symlink</type><name>link</name></file></toc></xar>"#,
                "foobar",
            ),
            (
                br#"<xar><toc><file id="1"><link type="file">A&amp;B</link><type>symlink</type><name>link</name></file></toc></xar>"#,
                "A&B",
            ),
        ];

        for &(xml, expected) in cases {
            let files = parse_toc_xml(xml).unwrap();
            assert_eq!(files[0].link.as_deref(), Some(expected));
        }
    }

    #[test]
    fn nested_ea_data_does_not_overwrite_file_data() {
        let xml = br#"<xar><toc><file id="1">
  <type>file</type><name>payload</name>
  <data>
    <offset>0</offset><length>4</length><size>4</size>
    <encoding style="application/octet-stream"/>
    <extracted-checksum>real-extracted</extracted-checksum>
    <archived-checksum>real-archived</archived-checksum>
  </data>
  <ea id="0"><name>extension</name><data>
    <offset>999</offset><length>3</length><size>3</size>
    <encoding style="application/x-gzip"/>
    <extracted-checksum>fake-extracted</extracted-checksum>
    <archived-checksum>fake-archived</archived-checksum>
  </data></ea>
</file></toc></xar>"#;

        let files = parse_toc_xml(xml).unwrap();
        let data = files[0].data.as_ref().unwrap();
        assert_eq!((data.offset, data.length, data.size), (0, 4, 4));
        assert_eq!(data.encoding, "application/octet-stream");
        assert_eq!(data.extracted_checksum.as_deref(), Some("real-extracted"));
        assert_eq!(data.archived_checksum.as_deref(), Some("real-archived"));
    }

    #[test]
    fn nested_ea_data_does_not_create_file_data() {
        let xml = br#"<xar><toc><file id="1">
  <type>file</type><name>metadata-only</name>
  <ea id="0"><data><offset>9</offset><length>3</length><size>3</size></data></ea>
</file></toc></xar>"#;

        let files = parse_toc_xml(xml).unwrap();
        assert!(files[0].data.is_none());
    }

    #[test]
    fn nested_data_does_not_interrupt_the_direct_file_data_block() {
        let xml = br#"<xar><toc><file id="1">
  <type>file</type><name>payload</name>
  <data>
    <offset>0</offset>
    <extension><data><offset>999</offset><length>3</length><size>3</size></data></extension>
    <length>4</length><size>4</size>
    <encoding style="application&#x2f;octet-stream"/>
  </data>
</file></toc></xar>"#;

        let files = parse_toc_xml(xml).unwrap();
        let data = files[0].data.as_ref().unwrap();
        assert_eq!((data.offset, data.length, data.size), (0, 4, 4));
        assert_eq!(data.encoding, "application/octet-stream");
    }

    #[test]
    fn invalid_xml_text_is_reported_instead_of_silently_erased() {
        let xml = br#"<xar><toc><file id="1"><link>&unknown;</link><type>symlink</type><name>link</name></file></toc></xar>"#;
        assert!(matches!(parse_toc_xml(xml), Err(XarError::XmlParse(_))));
    }

    #[test]
    fn incomplete_direct_data_block_is_rejected() {
        let xml = br#"<xar><toc><file id="1">
  <type>file</type><name>payload</name>
  <data><offset>0</offset><length>4</length></data>
</file></toc></xar>"#;
        assert!(matches!(parse_toc_xml(xml), Err(XarError::XmlParse(_))));
    }

    #[test]
    fn empty_direct_data_block_is_rejected() {
        let xml = br#"<xar><toc><file id="1"><type>file</type><name>payload</name><data/></file></toc></xar>"#;
        assert!(matches!(parse_toc_xml(xml), Err(XarError::XmlParse(_))));
    }

    #[test]
    fn nested_elements_inside_scalar_metadata_are_rejected() {
        let xml = br#"<xar><toc><file id="1"><type>symlink</type><name>link</name><link>before<extension/>after</link></file></toc></xar>"#;
        assert!(matches!(parse_toc_xml(xml), Err(XarError::XmlParse(_))));
    }

    #[test]
    fn duplicate_direct_data_blocks_are_rejected() {
        let xml = br#"<xar><toc><file id="1">
  <type>file</type><name>payload</name><data></data><data></data>
</file></toc></xar>"#;
        assert!(matches!(parse_toc_xml(xml), Err(XarError::XmlParse(_))));
    }

    #[test]
    fn paths_do_not_depend_on_parent_name_element_order() {
        let xml = br#"<xar><toc><file id="1"><type>directory</type>
  <file id="2"><type>directory</type>
    <file id="3"><type>file</type><name>leaf</name></file><name>child</name>
  </file><name>parent</name>
</file></toc></xar>"#;

        let files = parse_toc_xml(xml).unwrap();
        assert_eq!(files.iter().map(|file| file.path.as_str()).collect::<Vec<_>>(), ["parent/child/leaf", "parent/child", "parent"]);
    }

    #[test]
    fn duplicate_scalar_fields_and_unknown_types_are_rejected() {
        let cases: &[&[u8]] = &[
            br#"<xar><toc><file><type>file</type><name>first</name><name>second</name></file></toc></xar>"#,
            br#"<xar><toc><file><type>file</type><type>symlink</type><name>entry</name></file></toc></xar>"#,
            br#"<xar><toc><file><type>file</type><name>entry</name><data><offset>0</offset><offset>1</offset><length>0</length><size>0</size></data></file></toc></xar>"#,
            br#"<xar><toc><file><type>file</type><name>entry</name><data><offset>0</offset><length>0</length><size>0</size><encoding style="a"/><encoding style="b"/></data></file></toc></xar>"#,
            br#"<xar><toc><file><type>file</type><name>entry</name><data><offset>0</offset><length>0</length><size>0</size><encoding/></data></file></toc></xar>"#,
            br#"<xar><toc><file><type>file</type></file></toc></xar>"#,
            br#"<xar><toc><file><type>file</type><name/></file></toc></xar>"#,
            br#"<xar><toc><file><type>mystery</type><name>entry</name></file></toc></xar>"#,
        ];

        for xml in cases {
            assert!(matches!(parse_toc_xml(xml), Err(XarError::XmlParse(_))));
        }
    }

    #[test]
    fn document_structure_and_required_symlink_state_are_rejected_when_malformed() {
        let cases: &[&[u8]] = &[
            br#"<toc><file><type>file</type><name>entry</name></file></toc>"#,
            br#"<xar></xar>"#,
            br#"<xar><toc/><toc/></xar>"#,
            br#"<xar><extension><toc/></extension></xar>"#,
            br#"<xar><toc><file/></toc></xar>"#,
            br#"<xar><toc><file><name>entry</name></file></toc></xar>"#,
            br#"<xar><toc><file><type>symlink</type><name>link</name></file></toc></xar>"#,
            br#"<xar><toc><file><type>symlink</type><name>link</name><link/></file></toc></xar>"#,
            br#"<xar><toc/></xar><second/>"#,
        ];

        for xml in cases {
            assert!(matches!(parse_toc_xml(xml), Err(XarError::XmlParse(_))), "accepted malformed XML: {}", String::from_utf8_lossy(xml));
        }
    }

    #[test]
    fn duplicate_relevant_attributes_are_rejected() {
        let cases: &[&[u8]] = &[
            br#"<xar><toc><file id="1" id="2"><type>file</type><name>entry</name></file></toc></xar>"#,
            br#"<xar><toc><file><type>file</type><name>entry</name><data><offset>0</offset><length>0</length><size>0</size><encoding style="a" style="b"/></data></file></toc></xar>"#,
        ];

        for xml in cases {
            assert!(matches!(parse_toc_xml(xml), Err(XarError::XmlParse(_))));
        }
    }
}
