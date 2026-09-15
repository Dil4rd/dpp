pub mod attributes;
pub mod btree;
pub mod catalog;
pub mod error;
pub mod extents;
pub mod unicode;
pub mod volume;

#[cfg(any(test, feature = "testutil"))]
pub mod testutil;

pub use error::{HfsPlusError, Result};
pub use volume::VolumeHeader;

/// Failure while decoding a transparently compressed file.
pub use cmpfs::CmpfsError as CompressionError;
/// `com.apple.decmpfs` header of a transparently compressed file.
pub use cmpfs::Header as CompressionHeader;
/// Where a compressed file's payload lives.
pub use cmpfs::Storage as CompressionStorage;
/// Whether an extended attribute is user data or compression machinery.
pub use cmpfs::XattrKind;

use std::io::{Read, Seek, Write};

/// Entry kind in the filesystem
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    File,
    Directory,
    Symlink,
}

/// A directory entry returned by list_directory
#[derive(Debug, Clone)]
pub struct DirEntry {
    /// File or folder name
    pub name: String,
    /// Catalog Node ID
    pub cnid: u32,
    /// Entry type
    pub kind: EntryKind,
    /// Data fork logical size (0 for directories)
    pub size: u64,
    /// HFS+ creation date (seconds since 1904-01-01)
    pub create_date: u32,
    /// HFS+ modification date
    pub modify_date: u32,
}

/// HFS+ permissions (BSD-style)
#[derive(Debug, Clone)]
pub struct HfsPermissions {
    pub owner_id: u32,
    pub group_id: u32,
    pub mode: u16,
}

/// Detailed file/directory metadata
#[derive(Debug, Clone)]
pub struct FileStat {
    pub cnid: u32,
    pub kind: EntryKind,
    pub size: u64,
    pub create_date: u32,
    pub modify_date: u32,
    pub permissions: HfsPermissions,
    pub data_fork_extents: u32,
    pub resource_fork_size: u64,
    /// Present when the file is transparently compressed. `size` above is then
    /// the decompressed size; the data fork is empty.
    ///
    /// Also the signal that the file's compression is already resolved: the
    /// attributes listed as [`XattrKind::Compression`] must not be replicated
    /// onto an extracted copy.
    pub compression: Option<CompressionHeader>,
}

/// An extended attribute name, classified so a caller replicating metadata can
/// skip compression machinery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XattrEntry {
    /// Attribute name.
    pub name: String,
    /// Whether this is user data or transparent-compression machinery.
    pub kind: XattrKind,
}

/// Entry from walk() — includes full path
#[derive(Debug, Clone)]
pub struct WalkEntry {
    pub path: String,
    pub entry: DirEntry,
}

/// Pair each attribute name with its kind.
///
/// The file counts as compressed when the header attribute is present, which is
/// the same condition `read_file` decompresses on. Deriving it from the listing
/// keeps the two in step: if `read_file` treated the resource fork as a
/// payload, this reports it as machinery.
fn classify(names: Vec<String>) -> Vec<XattrEntry> {
    let compressed = names.iter().any(|name| name == cmpfs::XATTR_NAME);
    names
        .into_iter()
        .map(|name| XattrEntry {
            kind: cmpfs::classify_xattr(&name, compressed),
            name,
        })
        .collect()
}

/// High-level HFS+/HFSX volume reader
pub struct HfsVolume<R: Read + Seek> {
    reader: R,
    pub(crate) header: VolumeHeader,
    pub(crate) catalog_btree_header: btree::BTreeHeaderRecord,
    pub(crate) extents_btree_header: btree::BTreeHeaderRecord,
    /// Attributes B-tree header, read on first use. `Some(None)` records a
    /// volume with no attributes file. Loading it lazily keeps a damaged
    /// attributes tree from failing `open` for callers that never ask for an
    /// extended attribute.
    attributes_btree_header: Option<Option<btree::BTreeHeaderRecord>>,
}

impl<R: Read + Seek> HfsVolume<R> {
    /// Open and validate an HFS+/HFSX volume
    pub fn open(mut reader: R) -> Result<Self> {
        let header = volume::VolumeHeader::parse(&mut reader)?;

        // Read catalog B-tree header
        let catalog_btree_header =
            btree::read_btree_header(&mut reader, &header.catalog_file, header.block_size)?;

        // Read extents overflow B-tree header
        let extents_btree_header =
            btree::read_btree_header(&mut reader, &header.extents_file, header.block_size)?;

        Ok(HfsVolume {
            reader,
            header,
            catalog_btree_header,
            extents_btree_header,
            attributes_btree_header: None,
        })
    }

    /// Access the parsed volume header
    pub fn volume_header(&self) -> &VolumeHeader {
        &self.header
    }

    /// List entries in a directory by path
    pub fn list_directory(&mut self, path: &str) -> Result<Vec<DirEntry>> {
        let cnid = self.resolve_path_to_cnid(path)?;
        let mut entries = catalog::list_directory(
            &mut self.reader,
            &self.header,
            &self.catalog_btree_header,
            cnid,
        )?;
        self.resolve_entry_sizes(&mut entries)?;
        Ok(entries)
    }

    /// Make listed sizes agree with `stat`: a compressed file's data fork is
    /// empty, so its listed size is the decmpfs header's uncompressed size,
    /// exactly as `stat` reports it.
    fn resolve_entry_sizes(&mut self, entries: &mut [DirEntry]) -> Result<()> {
        for entry in entries.iter_mut() {
            if entry.kind == EntryKind::File
                && let Some(header) = self.compression(entry.cnid)?
            {
                entry.size = header.uncompressed_size;
            }
        }
        Ok(())
    }

    /// The Attributes B-tree header, or `None` on a volume with no attributes
    /// file. Read once and cached.
    fn attributes_btree(&mut self) -> Result<Option<btree::BTreeHeaderRecord>> {
        if self.attributes_btree_header.is_none() {
            let header = if self.header.attributes_file.logical_size == 0 {
                None
            } else {
                Some(btree::read_btree_header(
                    &mut self.reader,
                    &self.header.attributes_file,
                    self.header.block_size,
                )?)
            };
            self.attributes_btree_header = Some(header);
        }
        Ok(self.attributes_btree_header.clone().flatten())
    }

    /// Read an extended attribute, or `None` when the file has no attribute of
    /// that name.
    pub fn get_xattr(&mut self, path: &str, name: &str) -> Result<Option<Vec<u8>>> {
        let file_id = self.resolve_path_to_cnid(path)?;
        self.read_xattr(file_id, name)
    }

    /// Every extended attribute on a file or directory, classified.
    ///
    /// Nothing is filtered out: a caller inspecting the volume sees what the
    /// volume holds. [`XattrKind::Compression`] marks the attributes macOS
    /// hides, which [`Self::read_file`] has already resolved and which must
    /// not be copied onto an extracted file.
    ///
    /// Scans the Attributes B-tree, so it costs more than a single
    /// [`Self::get_xattr`]; prefer that when the name is known.
    ///
    /// HFS+ keeps the resource fork in the catalog record rather than as an
    /// attribute, so unlike APFS this never reports
    /// `com.apple.ResourceFork`; see [`FileStat::resource_fork_size`].
    pub fn list_xattrs(&mut self, path: &str) -> Result<Vec<XattrEntry>> {
        let file_id = self.resolve_path_to_cnid(path)?;
        let Some(attributes) = self.attributes_btree()? else {
            return Ok(Vec::new());
        };
        let names = attributes::list_names(&mut self.reader, &attributes, file_id)?;
        Ok(classify(names))
    }

    fn read_xattr(&mut self, file_id: u32, name: &str) -> Result<Option<Vec<u8>>> {
        let Some(attributes) = self.attributes_btree()? else {
            return Ok(None);
        };
        match attributes::lookup(&mut self.reader, &attributes, file_id, name)? {
            None => Ok(None),
            Some(attributes::AttrValue::Inline(data)) => Ok(Some(data)),
            Some(attributes::AttrValue::Fork(fork)) => {
                let mut data = Vec::new();
                extents::read_fork_data(
                    &mut self.reader,
                    &self.header,
                    &self.extents_btree_header,
                    &fork,
                    file_id,
                    &mut data,
                )?;
                Ok(Some(data))
            }
        }
    }

    /// The `com.apple.decmpfs` header of a transparently compressed file, or
    /// `None` when the file stores its bytes in the data fork as usual.
    fn compression(&mut self, file_id: u32) -> Result<Option<CompressionHeader>> {
        let Some(attr) = self.read_xattr(file_id, cmpfs::XATTR_NAME)? else {
            return Ok(None);
        };
        Ok(Some(CompressionHeader::parse(&attr)?))
    }

    /// Decompress a transparently compressed file.
    ///
    /// Whole-file, unlike the extent path: compression blocks are addressed
    /// relative to the decompressed output, so nothing can be emitted before
    /// the block covering it has been decoded.
    ///
    /// The resource fork here is the file's real fork from the catalog record,
    /// not an extended attribute — HFS+ has one, so it does not need the
    /// `com.apple.ResourceFork` attribute APFS uses.
    fn read_compressed(
        &mut self,
        file_record: &catalog::CatalogFile,
        header: &CompressionHeader,
    ) -> Result<Vec<u8>> {
        let attr = self
            .read_xattr(file_record.file_id, cmpfs::XATTR_NAME)?
            .ok_or_else(|| {
                HfsPlusError::CorruptedData(format!(
                    "file {} lost its decmpfs attribute",
                    file_record.file_id
                ))
            })?;

        let resource_fork = if header.storage() == CompressionStorage::ResourceFork {
            if file_record.resource_fork.logical_size == 0 {
                return Err(HfsPlusError::CorruptedData(format!(
                    "file {} is compressed into its resource fork, but the fork is empty",
                    file_record.file_id
                )));
            }
            let mut data = Vec::new();
            extents::read_fork_data(
                &mut self.reader,
                &self.header,
                &self.extents_btree_header,
                &file_record.resource_fork,
                file_record.file_id,
                &mut data,
            )?;
            Some(data)
        } else {
            None
        };

        Ok(cmpfs::decompress(&attr, resource_fork.as_deref())?)
    }

    /// Read an entire file into memory.
    ///
    /// Errors with [`HfsPlusError::CorruptedData`] if the fork's extents run
    /// out before its declared `logical_size` is reached. `Vec<u8>` cannot
    /// represent "complete except for a hole", so a short read has to be a
    /// failure here rather than a quietly truncated buffer; use
    /// [`HfsVolume::read_file_to`], which returns the byte count, to
    /// recover as much as the volume can give.
    ///
    /// Transparently compressed files are decompressed; see [`Self::stat`] to
    /// detect one first.
    pub fn read_file(&mut self, path: &str) -> Result<Vec<u8>> {
        let file_record = self.resolve_path_to_file(path)?;

        if let Some(header) = self.compression(file_record.file_id)? {
            return self.read_compressed(&file_record, &header);
        }

        let declared_size = file_record.data_fork.logical_size;

        let mut buf = Vec::new();
        let bytes_read = extents::read_fork_data(
            &mut self.reader,
            &self.header,
            &self.extents_btree_header,
            &file_record.data_fork,
            file_record.file_id,
            &mut buf,
        )?;

        if bytes_read != declared_size {
            return Err(HfsPlusError::CorruptedData(format!(
                "file {path:?} declares {declared_size} bytes but only {bytes_read} \
                 could be read; its extent chain ends early"
            )));
        }

        Ok(buf)
    }

    /// Open a file for streaming Read+Seek access without loading it into memory.
    /// Returns a ForkReader that translates logical file offsets to physical disk offsets.
    ///
    /// Fails on a transparently compressed file: its data fork is empty, so a
    /// reader over the extents would report a successful read of nothing. Use
    /// [`Self::read_file`] for those.
    pub fn open_file(&mut self, path: &str) -> Result<extents::ForkReader<'_, R>> {
        let file_record = self.resolve_path_to_file(path)?;
        if let Some(header) = self.compression(file_record.file_id)? {
            return Err(HfsPlusError::CorruptedData(format!(
                "{path} is decmpfs-compressed (type {}); read_file decompresses it, \
                 streaming does not",
                header.compression_type
            )));
        }
        Ok(extents::ForkReader::new(
            &mut self.reader,
            &file_record.data_fork,
            self.header.block_size,
        ))
    }

    /// Stream a file to a writer
    ///
    /// Transparently compressed files are decompressed and written whole.
    pub fn read_file_to<W: Write>(&mut self, path: &str, mut writer: W) -> Result<u64> {
        let file_record = self.resolve_path_to_file(path)?;

        if let Some(header) = self.compression(file_record.file_id)? {
            let data = self.read_compressed(&file_record, &header)?;
            writer.write_all(&data)?;
            return Ok(data.len() as u64);
        }

        extents::read_fork_data(
            &mut self.reader,
            &self.header,
            &self.extents_btree_header,
            &file_record.data_fork,
            file_record.file_id,
            &mut writer,
        )
    }

    /// Get metadata for a file or directory
    pub fn stat(&mut self, path: &str) -> Result<FileStat> {
        let (record, _name) = self.resolve_path_to_record(path)?;
        match record {
            catalog::CatalogRecord::File(f) => {
                // A compressed file's data fork is empty; the real length is
                // in the decmpfs header.
                let compression = self.compression(f.file_id)?;
                Ok(FileStat {
                    cnid: f.file_id,
                    // Same mode test as list_directory: a symlink is a file
                    // record whose mode says S_IFLNK.
                    kind: if f.permissions.file_mode & 0o170000 == 0o120000 {
                        EntryKind::Symlink
                    } else {
                        EntryKind::File
                    },
                    size: compression.map_or(f.data_fork.logical_size, |h| h.uncompressed_size),
                    create_date: f.create_date,
                    modify_date: f.content_mod_date,
                    permissions: HfsPermissions {
                        owner_id: f.permissions.owner_id,
                        group_id: f.permissions.group_id,
                        mode: f.permissions.file_mode,
                    },
                    data_fork_extents: f
                        .data_fork
                        .extents
                        .iter()
                        .filter(|e| e.block_count > 0)
                        .count() as u32,
                    resource_fork_size: f.resource_fork.logical_size,
                    compression,
                })
            }
            catalog::CatalogRecord::Folder(f) => Ok(FileStat {
                cnid: f.folder_id,
                kind: EntryKind::Directory,
                size: 0,
                create_date: f.create_date,
                modify_date: f.content_mod_date,
                permissions: HfsPermissions {
                    owner_id: f.permissions.owner_id,
                    group_id: f.permissions.group_id,
                    mode: f.permissions.file_mode,
                },
                data_fork_extents: 0,
                resource_fork_size: 0,
                compression: None,
            }),
            _ => Err(HfsPlusError::CorruptedData(
                "unexpected thread record".into(),
            )),
        }
    }

    /// Recursive walk of all entries
    pub fn walk(&mut self) -> Result<Vec<WalkEntry>> {
        let mut entries = Vec::new();
        self.walk_recursive(catalog::CNID_ROOT_FOLDER, "", &mut entries)?;
        Ok(entries)
    }

    /// Check if a path exists
    pub fn exists(&mut self, path: &str) -> Result<bool> {
        match self.resolve_path_to_record(path) {
            Ok(_) => Ok(true),
            Err(HfsPlusError::FileNotFound(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    // --- Internal helpers ---

    fn resolve_path_to_cnid(&mut self, path: &str) -> Result<u32> {
        let (record, _name) = self.resolve_path_to_record(path)?;
        match record {
            catalog::CatalogRecord::Folder(f) => Ok(f.folder_id),
            catalog::CatalogRecord::File(f) => Ok(f.file_id),
            _ => Err(HfsPlusError::CorruptedData(
                "unexpected thread record".into(),
            )),
        }
    }

    fn resolve_path_to_file(&mut self, path: &str) -> Result<catalog::CatalogFile> {
        let (record, _name) = self.resolve_path_to_record(path)?;
        match record {
            catalog::CatalogRecord::File(f) => Ok(f),
            catalog::CatalogRecord::Folder(_) => Err(HfsPlusError::NotADirectory(path.to_string())),
            _ => Err(HfsPlusError::CorruptedData(
                "unexpected thread record".into(),
            )),
        }
    }

    fn resolve_path_to_record(&mut self, path: &str) -> Result<(catalog::CatalogRecord, String)> {
        catalog::resolve_path(
            &mut self.reader,
            &self.header,
            &self.catalog_btree_header,
            path,
        )
    }

    fn walk_recursive(
        &mut self,
        parent_cnid: u32,
        parent_path: &str,
        entries: &mut Vec<WalkEntry>,
    ) -> Result<()> {
        let mut dir_entries = catalog::list_directory(
            &mut self.reader,
            &self.header,
            &self.catalog_btree_header,
            parent_cnid,
        )?;
        self.resolve_entry_sizes(&mut dir_entries)?;

        for entry in dir_entries {
            let full_path = if parent_path.is_empty() {
                format!("/{}", entry.name)
            } else {
                format!("{}/{}", parent_path, entry.name)
            };

            let is_dir = entry.kind == EntryKind::Directory;
            let cnid = entry.cnid;

            entries.push(WalkEntry {
                path: full_path.clone(),
                entry,
            });

            if is_dir {
                self.walk_recursive(cnid, &full_path, entries)?;
            }
        }

        Ok(())
    }
}
