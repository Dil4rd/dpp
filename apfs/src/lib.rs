pub mod error;
pub mod fletcher;
pub mod object;
pub mod superblock;
pub mod btree;
pub mod omap;
pub mod catalog;
pub mod extents;

pub use error::{ApfsError, Result};

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
    pub name: String,
    pub oid: u64,
    pub kind: EntryKind,
    pub size: u64,
    pub create_time: i64,
    pub modify_time: i64,
}

/// Detailed file/directory metadata
#[derive(Debug, Clone)]
pub struct FileStat {
    pub oid: u64,
    pub kind: EntryKind,
    pub size: u64,
    pub create_time: i64,
    pub modify_time: i64,
    pub uid: u32,
    pub gid: u32,
    pub mode: u16,
    pub nlink: u32,
}

/// Entry from walk() — includes full path
#[derive(Debug, Clone)]
pub struct WalkEntry {
    pub path: String,
    pub entry: DirEntry,
}

/// Volume information
#[derive(Debug, Clone)]
pub struct VolumeInfo {
    pub name: String,
    pub block_size: u32,
    pub num_files: u64,
    pub num_directories: u64,
    pub num_symlinks: u64,
}

/// High-level read-only APFS volume reader
pub struct ApfsVolume<R: Read + Seek> {
    reader: R,
    block_size: u32,
    vol_omap_root_block: u64,
    catalog_root_block: u64,
    info: VolumeInfo,
}

impl<R: Read + Seek> ApfsVolume<R> {
    /// Open an APFS container and mount the first volume.
    ///
    /// 1. Read block 0 → parse NX superblock, validate NXSB magic + Fletcher-64
    /// 2. Scan checkpoint descriptor area for latest valid NX superblock
    /// 3. Read container OMAP at omap_oid physical block
    /// 4. Find first non-zero OID in fs_oids array
    /// 5. Resolve volume OID → physical block via container OMAP
    /// 6. Parse volume superblock (APSB magic)
    /// 7. Read volume OMAP at vol.omap_oid physical block
    /// 8. Resolve vol.root_tree_oid → physical block via volume OMAP → catalog B-tree root
    /// 9. Store all state
    pub fn open(mut reader: R) -> Result<Self> {
        // Step 1-2: Read and validate container superblock
        let nxsb = superblock::read_nxsb(&mut reader)?;
        let nxsb = superblock::find_latest_nxsb(&mut reader, &nxsb)?;
        let block_size = nxsb.block_size;

        // Step 3: Read container OMAP
        let container_omap_root = omap::read_omap_tree_root(&mut reader, nxsb.omap_oid, block_size)?;

        // Step 4: Find first non-zero volume OID
        let vol_oid = nxsb.fs_oids.iter()
            .find(|&&o| o != 0)
            .copied()
            .ok_or(ApfsError::NoVolume)?;

        // Step 5: Resolve volume OID via container OMAP
        let vol_block = omap::omap_lookup(&mut reader, container_omap_root, block_size, vol_oid)?;

        // Step 6: Parse volume superblock
        let vol_data = object::read_block(&mut reader, vol_block, block_size)?;
        let vol_sb = superblock::ApfsSuperblock::parse(&vol_data)?;

        // Step 7: Read volume OMAP
        let vol_omap_root_block = omap::read_omap_tree_root(&mut reader, vol_sb.omap_oid, block_size)?;

        // Step 8: Resolve catalog root tree OID via volume OMAP
        let catalog_root_block = omap::omap_lookup(
            &mut reader, vol_omap_root_block, block_size, vol_sb.root_tree_oid,
        )?;

        // Step 9: Store state
        let info = VolumeInfo {
            name: vol_sb.volume_name.clone(),
            block_size,
            num_files: vol_sb.num_files,
            num_directories: vol_sb.num_directories,
            num_symlinks: vol_sb.num_symlinks,
        };

        Ok(ApfsVolume {
            reader,
            block_size,
            vol_omap_root_block,
            catalog_root_block,
            info,
        })
    }

    /// Get volume metadata
    pub fn volume_info(&self) -> &VolumeInfo {
        &self.info
    }

    /// List entries in a directory by path
    pub fn list_directory(&mut self, path: &str) -> Result<Vec<DirEntry>> {
        let (oid, _inode) = if path == "/" || path.is_empty() {
            // Root directory has a well-known OID
            (catalog::ROOT_DIR_PARENT, catalog::ROOT_DIR_RECORD)
        } else {
            let (oid, inode) = catalog::resolve_path(
                &mut self.reader,
                self.catalog_root_block,
                self.vol_omap_root_block,
                self.block_size,
                path,
            )?;
            if inode.kind() != catalog::INODE_DIR_TYPE {
                return Err(ApfsError::NotADirectory(path.to_string()));
            }
            (oid, oid)
        };

        let parent = if path == "/" || path.is_empty() {
            catalog::ROOT_DIR_RECORD
        } else {
            oid
        };

        catalog::list_directory(
            &mut self.reader,
            self.catalog_root_block,
            self.vol_omap_root_block,
            self.block_size,
            parent,
        )
    }

    /// Read an entire file into memory
    pub fn read_file(&mut self, path: &str) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        self.read_file_to(path, &mut buf)?;
        Ok(buf)
    }

    /// Stream a file to a writer
    pub fn read_file_to<W: Write>(&mut self, path: &str, writer: &mut W) -> Result<u64> {
        let (_oid, inode) = catalog::resolve_path(
            &mut self.reader,
            self.catalog_root_block,
            self.vol_omap_root_block,
            self.block_size,
            path,
        )?;

        // File extents are keyed by private_id, not the inode OID
        let file_extents = catalog::lookup_extents(
            &mut self.reader,
            self.catalog_root_block,
            self.vol_omap_root_block,
            self.block_size,
            inode.private_id,
        )?;

        extents::read_file_data(
            &mut self.reader,
            self.block_size,
            &file_extents,
            inode.size(),
            writer,
        )
    }

    /// Open a file for streaming Read+Seek access
    pub fn open_file(&mut self, path: &str) -> Result<extents::ApfsForkReader<'_, R>> {
        let (_oid, inode) = catalog::resolve_path(
            &mut self.reader,
            self.catalog_root_block,
            self.vol_omap_root_block,
            self.block_size,
            path,
        )?;

        // File extents are keyed by private_id, not the inode OID
        let file_extents = catalog::lookup_extents(
            &mut self.reader,
            self.catalog_root_block,
            self.vol_omap_root_block,
            self.block_size,
            inode.private_id,
        )?;

        Ok(extents::ApfsForkReader::new(
            &mut self.reader,
            self.block_size,
            file_extents,
            inode.size(),
        ))
    }

    /// Get metadata for a file or directory
    pub fn stat(&mut self, path: &str) -> Result<FileStat> {
        let (oid, inode) = catalog::resolve_path(
            &mut self.reader,
            self.catalog_root_block,
            self.vol_omap_root_block,
            self.block_size,
            path,
        )?;

        Ok(FileStat {
            oid,
            kind: match inode.kind() {
                catalog::INODE_DIR_TYPE => EntryKind::Directory,
                catalog::INODE_SYMLINK_TYPE => EntryKind::Symlink,
                _ => EntryKind::File,
            },
            size: inode.size(),
            create_time: inode.create_time,
            modify_time: inode.modify_time,
            uid: inode.uid,
            gid: inode.gid,
            mode: inode.mode,
            nlink: inode.nlink(),
        })
    }

    /// Recursive walk of all entries
    pub fn walk(&mut self) -> Result<Vec<WalkEntry>> {
        let mut entries = Vec::new();
        self.walk_recursive(catalog::ROOT_DIR_RECORD, "", &mut entries)?;
        Ok(entries)
    }

    /// Check if a path exists
    pub fn exists(&mut self, path: &str) -> Result<bool> {
        match catalog::resolve_path(
            &mut self.reader,
            self.catalog_root_block,
            self.vol_omap_root_block,
            self.block_size,
            path,
        ) {
            Ok(_) => Ok(true),
            Err(ApfsError::FileNotFound(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    fn walk_recursive(
        &mut self,
        parent_oid: u64,
        parent_path: &str,
        entries: &mut Vec<WalkEntry>,
    ) -> Result<()> {
        let dir_entries = catalog::list_directory(
            &mut self.reader,
            self.catalog_root_block,
            self.vol_omap_root_block,
            self.block_size,
            parent_oid,
        )?;

        for entry in dir_entries {
            let full_path = if parent_path.is_empty() {
                format!("/{}", entry.name)
            } else {
                format!("{}/{}", parent_path, entry.name)
            };

            let is_dir = entry.kind == EntryKind::Directory;
            let oid = entry.oid;

            entries.push(WalkEntry {
                path: full_path.clone(),
                entry,
            });

            if is_dir {
                self.walk_recursive(oid, &full_path, entries)?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufReader;

    /// Requires ../tests/appfs.raw fixture. Run with `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn test_volume_open() {
        let file = std::fs::File::open("../tests/appfs.raw").unwrap();
        let reader = BufReader::new(file);

        let mut vol = ApfsVolume::open(reader).unwrap();
        let info = vol.volume_info();

        assert!(!info.name.is_empty(), "Volume name should not be empty");
        assert_eq!(info.block_size, 4096);

        let entries = vol.list_directory("/").unwrap();
        assert!(!entries.is_empty(), "Root directory should have entries");

        let walk_entries = vol.walk().unwrap();
        assert!(!walk_entries.is_empty());
    }

    /// Requires ../tests/appfs.raw fixture. Run with `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn test_read_file_data() {
        let file = std::fs::File::open("../tests/appfs.raw").unwrap();
        let reader = BufReader::new(file);

        let mut vol = ApfsVolume::open(reader).unwrap();

        let walk = vol.walk().unwrap();
        let small_file = walk.iter()
            .find(|e| e.entry.kind == EntryKind::File && e.entry.size > 0 && e.entry.size < 1_000_000);

        let entry = small_file.expect("Should find a small file in the test image");
        let data = vol.read_file(&entry.path).unwrap();
        assert_eq!(data.len() as u64, entry.entry.size,
            "Read size should match stat size");

        let stat = vol.stat(&entry.path).unwrap();
        assert_eq!(stat.size, entry.entry.size);
    }
}
