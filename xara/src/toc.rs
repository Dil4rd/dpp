use flate2::read::ZlibDecoder;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::io::Read;

use crate::error::{XarError, Result};
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
    /// Data descriptor (None for directories)
    pub data: Option<XarFileData>,
    /// Child file indices (for directories)
    pub children: Vec<usize>,
    /// Parent index (None for root-level entries)
    pub parent: Option<usize>,
}

/// Parse the TOC from a XAR archive.
/// Returns (files, heap_offset).
pub fn parse_toc<R: Read>(
    reader: &mut R,
    header: &XarHeader,
) -> Result<(Vec<XarFile>, u64)> {
    let mut compressed = vec![0u8; header.toc_compressed_len as usize];
    reader.read_exact(&mut compressed)?;

    let mut decoder = ZlibDecoder::new(&compressed[..]);
    let mut xml_data = Vec::with_capacity(header.toc_uncompressed_len as usize);
    decoder.read_to_end(&mut xml_data)
        .map_err(|e| XarError::DecompressionFailed(format!("TOC zlib: {}", e)))?;

    let files = parse_toc_xml(&xml_data)?;
    let heap_offset = header.header_size as u64 + header.toc_compressed_len;

    Ok((files, heap_offset))
}

/// Internal state for a file being parsed
struct FileBuilder {
    id: u64,
    name: String,
    file_type: Option<String>,
    children: Vec<usize>,
    // data fields
    in_data: bool,
    data_offset: Option<u64>,
    data_length: Option<u64>,
    data_size: Option<u64>,
    data_encoding: Option<String>,
    extracted_checksum: Option<String>,
    archived_checksum: Option<String>,
    in_extracted_checksum: bool,
    in_archived_checksum: bool,
    // current tag being parsed
    current_tag: String,
    // path components from parent files on the stack
    parent_path: String,
}

fn parse_toc_xml(xml: &[u8]) -> Result<Vec<XarFile>> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(true);

    let mut files: Vec<XarFile> = Vec::new();
    let mut buf = Vec::new();
    let mut in_toc = false;

    // Stack of files being parsed. Children are nested inside parents in XAR TOC.
    // When </file> is encountered, the file is popped and added to `files`.
    // Children are finalized before their parents, so child indices are known.
    let mut stack: Vec<FileBuilder> = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match tag.as_str() {
                    "toc" => in_toc = true,
                    "file" if in_toc => {
                        let mut id = 0u64;
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"id" {
                                id = String::from_utf8_lossy(&attr.value)
                                    .parse()
                                    .unwrap_or(0);
                            }
                        }

                        let parent_path = if let Some(parent) = stack.last() {
                            if parent.parent_path.is_empty() {
                                parent.name.clone()
                            } else {
                                format!("{}/{}", parent.parent_path, parent.name)
                            }
                        } else {
                            String::new()
                        };

                        stack.push(FileBuilder {
                            id,
                            name: String::new(),
                            file_type: None,
                            children: Vec::new(),
                            in_data: false,
                            data_offset: None,
                            data_length: None,
                            data_size: None,
                            data_encoding: None,
                            extracted_checksum: None,
                            archived_checksum: None,
                            in_extracted_checksum: false,
                            in_archived_checksum: false,
                            current_tag: String::new(),
                            parent_path,
                        });
                    }
                    "data" if !stack.is_empty() => {
                        stack.last_mut().unwrap().in_data = true;
                    }
                    "extracted-checksum" if !stack.is_empty() => {
                        let f = stack.last_mut().unwrap();
                        if f.in_data {
                            f.in_extracted_checksum = true;
                        }
                    }
                    "archived-checksum" if !stack.is_empty() => {
                        let f = stack.last_mut().unwrap();
                        if f.in_data {
                            f.in_archived_checksum = true;
                        }
                    }
                    "encoding" if !stack.is_empty() => {
                        let f = stack.last_mut().unwrap();
                        if f.in_data {
                            for attr in e.attributes().flatten() {
                                if attr.key.as_ref() == b"style" {
                                    f.data_encoding = Some(
                                        String::from_utf8_lossy(&attr.value).to_string(),
                                    );
                                }
                            }
                        }
                    }
                    _ => {
                        if let Some(f) = stack.last_mut() {
                            f.current_tag = tag;
                        }
                    }
                }
            }
            Ok(Event::Empty(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if tag == "encoding"
                    && let Some(f) = stack.last_mut()
                        && f.in_data {
                            for attr in e.attributes().flatten() {
                                if attr.key.as_ref() == b"style" {
                                    f.data_encoding = Some(
                                        String::from_utf8_lossy(&attr.value).to_string(),
                                    );
                                }
                            }
                        }
            }
            Ok(Event::Text(ref e)) => {
                let text = e.unescape().unwrap_or_default().to_string();
                if let Some(f) = stack.last_mut() {
                    if f.in_extracted_checksum {
                        f.extracted_checksum = Some(text);
                    } else if f.in_archived_checksum {
                        f.archived_checksum = Some(text);
                    } else if f.in_data {
                        match f.current_tag.as_str() {
                            "offset" => f.data_offset = text.parse().ok(),
                            "length" => f.data_length = text.parse().ok(),
                            "size" => f.data_size = text.parse().ok(),
                            _ => {}
                        }
                    } else {
                        match f.current_tag.as_str() {
                            "name" => f.name = text,
                            "type" => f.file_type = Some(text),
                            _ => {}
                        }
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match tag.as_str() {
                    "toc" => in_toc = false,
                    "file" if !stack.is_empty() => {
                        let builder = stack.pop().unwrap();

                        let file_type = match builder.file_type.as_deref() {
                            Some("directory") => XarFileType::Directory,
                            Some("symlink") => XarFileType::Symlink,
                            _ => XarFileType::File,
                        };

                        let data = if let (Some(offset), Some(length), Some(size)) =
                            (builder.data_offset, builder.data_length, builder.data_size)
                        {
                            Some(XarFileData {
                                offset,
                                length,
                                size,
                                encoding: builder.data_encoding.unwrap_or_else(|| {
                                    "application/octet-stream".to_string()
                                }),
                                extracted_checksum: builder.extracted_checksum,
                                archived_checksum: builder.archived_checksum,
                            })
                        } else {
                            None
                        };

                        let path = if builder.parent_path.is_empty() {
                            builder.name.clone()
                        } else {
                            format!("{}/{}", builder.parent_path, builder.name)
                        };

                        let file_idx = files.len();
                        let parent = if stack.is_empty() {
                            None
                        } else {
                            // Parent is not yet finalized, but we can record that
                            // this file is a child of whatever is on top of the stack
                            stack.last_mut().unwrap().children.push(file_idx);
                            // Parent index will be set when the parent is finalized
                            // For now, store None; we'll fix it up at the end
                            None
                        };

                        files.push(XarFile {
                            id: builder.id,
                            name: builder.name,
                            path,
                            file_type,
                            data,
                            children: builder.children,
                            parent,
                        });
                    }
                    "data" if !stack.is_empty() => {
                        stack.last_mut().unwrap().in_data = false;
                    }
                    "extracted-checksum" if !stack.is_empty() => {
                        stack.last_mut().unwrap().in_extracted_checksum = false;
                    }
                    "archived-checksum" if !stack.is_empty() => {
                        stack.last_mut().unwrap().in_archived_checksum = false;
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
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

    Ok(files)
}

/// Find a file by path in the flat file list
pub fn find_by_path<'a>(files: &'a [XarFile], path: &str) -> Option<&'a XarFile> {
    let path = path.trim_matches('/');
    files.iter().find(|f| f.path == path)
}
