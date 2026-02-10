pub mod error;
pub mod volume;
pub mod btree;
pub mod catalog;
pub mod extents;
pub mod unicode;

pub use error::{HfsPlusError, Result};
pub use volume::VolumeHeader;

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
}

/// Entry from walk() — includes full path
#[derive(Debug, Clone)]
pub struct WalkEntry {
    pub path: String,
    pub entry: DirEntry,
}

/// High-level HFS+/HFSX volume reader
pub struct HfsVolume<R: Read + Seek> {
    reader: R,
    pub(crate) header: VolumeHeader,
    pub(crate) catalog_btree_header: btree::BTreeHeaderRecord,
    pub(crate) extents_btree_header: btree::BTreeHeaderRecord,
}

impl<R: Read + Seek> HfsVolume<R> {
    /// Open and validate an HFS+/HFSX volume
    pub fn open(mut reader: R) -> Result<Self> {
        let header = volume::VolumeHeader::parse(&mut reader)?;

        // Read catalog B-tree header
        let catalog_btree_header = btree::read_btree_header(
            &mut reader,
            &header.catalog_file,
            header.block_size,
        )?;

        // Read extents overflow B-tree header
        let extents_btree_header = btree::read_btree_header(
            &mut reader,
            &header.extents_file,
            header.block_size,
        )?;

        Ok(HfsVolume {
            reader,
            header,
            catalog_btree_header,
            extents_btree_header,
        })
    }

    /// Access the parsed volume header
    pub fn volume_header(&self) -> &VolumeHeader {
        &self.header
    }

    /// List entries in a directory by path
    pub fn list_directory(&mut self, path: &str) -> Result<Vec<DirEntry>> {
        let cnid = self.resolve_path_to_cnid(path)?;
        catalog::list_directory(
            &mut self.reader,
            &self.header,
            &self.catalog_btree_header,
            cnid,
        )
    }

    /// Read an entire file into memory
    pub fn read_file(&mut self, path: &str) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        self.read_file_to(path, &mut buf)?;
        Ok(buf)
    }

    /// Open a file for streaming Read+Seek access without loading it into memory.
    /// Returns a ForkReader that translates logical file offsets to physical disk offsets.
    pub fn open_file(&mut self, path: &str) -> Result<extents::ForkReader<'_, R>> {
        let file_record = self.resolve_path_to_file(path)?;
        Ok(extents::ForkReader::new(
            &mut self.reader,
            &file_record.data_fork,
            self.header.block_size,
        ))
    }

    /// Stream a file to a writer
    pub fn read_file_to<W: Write>(&mut self, path: &str, mut writer: W) -> Result<u64> {
        let file_record = self.resolve_path_to_file(path)?;
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
            catalog::CatalogRecord::File(f) => Ok(FileStat {
                cnid: f.file_id,
                kind: EntryKind::File,
                size: f.data_fork.logical_size,
                create_date: f.create_date,
                modify_date: f.content_mod_date,
                permissions: HfsPermissions {
                    owner_id: f.permissions.owner_id,
                    group_id: f.permissions.group_id,
                    mode: f.permissions.file_mode,
                },
                data_fork_extents: f.data_fork.extents.iter().filter(|e| e.block_count > 0).count() as u32,
                resource_fork_size: f.resource_fork.logical_size,
            }),
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
            }),
            _ => Err(HfsPlusError::CorruptedData("unexpected thread record".into())),
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
            _ => Err(HfsPlusError::CorruptedData("unexpected thread record".into())),
        }
    }

    fn resolve_path_to_file(&mut self, path: &str) -> Result<catalog::CatalogFile> {
        let (record, _name) = self.resolve_path_to_record(path)?;
        match record {
            catalog::CatalogRecord::File(f) => Ok(f),
            catalog::CatalogRecord::Folder(_) => Err(HfsPlusError::NotADirectory(path.to_string())),
            _ => Err(HfsPlusError::CorruptedData("unexpected thread record".into())),
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
        let dir_entries = catalog::list_directory(
            &mut self.reader,
            &self.header,
            &self.catalog_btree_header,
            parent_cnid,
        )?;

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
