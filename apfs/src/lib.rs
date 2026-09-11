// Not public: its comparator contract fails silently when broken, so callers
// are served by the readers below instead. See the module docs.
pub(crate) mod btree;
pub mod catalog;
pub mod error;
pub mod extents;
pub mod fletcher;
pub mod object;
pub mod omap;
pub mod superblock;

pub use error::{ApfsError, Result};

/// Failure while decoding a transparently compressed file.
pub use cmpfs::CmpfsError as CompressionError;
/// `com.apple.decmpfs` header of a transparently compressed file.
pub use cmpfs::Header as CompressionHeader;
/// Where a compressed file's payload lives.
pub use cmpfs::Storage as CompressionStorage;

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
    /// Present when the file is transparently compressed. `size` above is then
    /// the decompressed size, which the inode does not record.
    pub compression: Option<CompressionHeader>,
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
        let container_omap_root =
            omap::read_omap_tree_root(&mut reader, nxsb.omap_oid, block_size)?;

        // Step 4: Find first non-zero volume OID
        let vol_oid = nxsb
            .fs_oids
            .iter()
            .find(|&&o| o != 0)
            .copied()
            .ok_or(ApfsError::NoVolume)?;

        // Step 5: Resolve volume OID via container OMAP
        let vol_block = omap::omap_lookup(&mut reader, container_omap_root, block_size, vol_oid)?;

        // Step 6: Parse volume superblock
        let vol_data = object::read_block(&mut reader, vol_block, block_size)?;
        let vol_sb = superblock::ApfsSuperblock::parse(&vol_data)?;

        // Step 7: Read volume OMAP
        let vol_omap_root_block =
            omap::read_omap_tree_root(&mut reader, vol_sb.omap_oid, block_size)?;

        // Step 8: Resolve catalog root tree OID via volume OMAP
        let catalog_root_block = omap::omap_lookup(
            &mut reader,
            vol_omap_root_block,
            block_size,
            vol_sb.root_tree_oid,
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

    /// Symlink targets on APFS are stored in a "com.apple.fs.symlink"
    /// extended attribute (NUL-terminated), not as file data. Returns the
    /// target bytes for symlink inodes, or None when no xattr is present.
    fn symlink_target(&mut self, oid: u64) -> Result<Option<Vec<u8>>> {
        let Some(xattr) = catalog::lookup_xattr(
            &mut self.reader,
            self.catalog_root_block,
            self.vol_omap_root_block,
            self.block_size,
            oid,
            catalog::SYMLINK_XATTR_NAME,
        )?
        else {
            return Ok(None);
        };
        let end = xattr.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
        Ok(Some(xattr[..end].to_vec()))
    }

    /// Read an extended attribute, or `None` when the inode has no attribute
    /// of that name.
    ///
    /// Resolves both storage forms: values held in the catalog record and
    /// values held in a data stream, which is how resource forks are stored.
    pub fn get_xattr(&mut self, path: &str, name: &str) -> Result<Option<Vec<u8>>> {
        let (oid, _inode) = catalog::resolve_path(
            &mut self.reader,
            self.catalog_root_block,
            self.vol_omap_root_block,
            self.block_size,
            path,
        )?;
        self.read_xattr(oid, name)
    }

    /// Names of every extended attribute on a file or directory.
    pub fn list_xattrs(&mut self, path: &str) -> Result<Vec<String>> {
        let (oid, _inode) = catalog::resolve_path(
            &mut self.reader,
            self.catalog_root_block,
            self.vol_omap_root_block,
            self.block_size,
            path,
        )?;
        catalog::list_xattr_names(
            &mut self.reader,
            self.catalog_root_block,
            self.vol_omap_root_block,
            self.block_size,
            oid,
        )
    }

    fn read_xattr(&mut self, oid: u64, name: &str) -> Result<Option<Vec<u8>>> {
        let value = catalog::lookup_xattr_value(
            &mut self.reader,
            self.catalog_root_block,
            self.vol_omap_root_block,
            self.block_size,
            oid,
            name,
        )?;

        match value {
            None => Ok(None),
            Some(catalog::XattrValue::Embedded(data)) => Ok(Some(data)),
            Some(catalog::XattrValue::DataStream { obj_id, size }) => {
                let extents = catalog::lookup_extents(
                    &mut self.reader,
                    self.catalog_root_block,
                    self.vol_omap_root_block,
                    self.block_size,
                    obj_id,
                )?;
                let mut data = Vec::with_capacity(usize::try_from(size).unwrap_or(0));
                extents::read_file_data(
                    &mut self.reader,
                    self.block_size,
                    &extents,
                    size,
                    &mut data,
                )?;
                Ok(Some(data))
            }
        }
    }

    /// The `com.apple.decmpfs` header of a transparently compressed inode, or
    /// `None` when the file stores its bytes in the data fork as usual.
    fn compression(&mut self, oid: u64) -> Result<Option<CompressionHeader>> {
        let Some(attr) = self.read_xattr(oid, cmpfs::XATTR_NAME)? else {
            return Ok(None);
        };
        Ok(Some(CompressionHeader::parse(&attr)?))
    }

    /// Decompress a transparently compressed file.
    ///
    /// Whole-file, unlike the extent path: compression blocks are addressed
    /// relative to the decompressed output, so nothing can be emitted before
    /// the block covering it has been decoded.
    fn read_compressed(&mut self, oid: u64, header: &CompressionHeader) -> Result<Vec<u8>> {
        let attr = self.read_xattr(oid, cmpfs::XATTR_NAME)?.ok_or_else(|| {
            ApfsError::CorruptedData(format!("inode {oid} lost its decmpfs attribute"))
        })?;

        let resource_fork = match header.storage() {
            CompressionStorage::ResourceFork => Some(
                self.read_xattr(oid, cmpfs::RESOURCE_FORK_XATTR_NAME)?
                    .ok_or_else(|| {
                        ApfsError::CorruptedData(format!(
                            "inode {oid} is compressed into its resource fork, but has none"
                        ))
                    })?,
            ),
            _ => None,
        };

        Ok(cmpfs::decompress(&attr, resource_fork.as_deref())?)
    }

    /// Stream a file to a writer
    ///
    /// Transparently compressed files are decompressed; see [`Self::stat`] to
    /// detect one first.
    pub fn read_file_to<W: Write>(&mut self, path: &str, writer: &mut W) -> Result<u64> {
        let (oid, inode) = catalog::resolve_path(
            &mut self.reader,
            self.catalog_root_block,
            self.vol_omap_root_block,
            self.block_size,
            path,
        )?;

        if inode.kind() != catalog::INODE_SYMLINK_TYPE
            && let Some(header) = self.compression(oid)?
        {
            let data = self.read_compressed(oid, &header)?;
            writer.write_all(&data)?;
            return Ok(data.len() as u64);
        }

        // Symlink inodes carry no extents; read the target from the xattr.
        // Falls through to the extent read for images that store the target
        // as file data.
        if inode.kind() == catalog::INODE_SYMLINK_TYPE
            && let Some(target) = self.symlink_target(oid)?
        {
            writer.write_all(&target)?;
            return Ok(target.len() as u64);
        }

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
    ///
    /// Fails on a transparently compressed file: its data fork is empty, so a
    /// reader over the extents would report a successful read of nothing. Use
    /// [`Self::read_file`] for those.
    pub fn open_file(&mut self, path: &str) -> Result<extents::ApfsForkReader<'_, R>> {
        let (oid, _) = catalog::resolve_path(
            &mut self.reader,
            self.catalog_root_block,
            self.vol_omap_root_block,
            self.block_size,
            path,
        )?;
        if let Some(header) = self.compression(oid)? {
            return Err(ApfsError::Unsupported(format!(
                "{path} is decmpfs-compressed (type {}); read_file decompresses it, \
                 streaming does not",
                header.compression_type
            )));
        }

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

        // Symlink inodes report size 0; use the xattr target length instead.
        let mut compression = None;
        let size = if inode.kind() == catalog::INODE_SYMLINK_TYPE {
            self.symlink_target(oid)?
                .map_or(inode.size(), |t| t.len() as u64)
        } else {
            // A compressed inode records no size of its own — the data fork is
            // empty and the real length is in the decmpfs header.
            compression = self.compression(oid)?;
            compression.map_or_else(|| inode.size(), |h| h.uncompressed_size)
        };

        Ok(FileStat {
            oid,
            kind: match inode.kind() {
                catalog::INODE_DIR_TYPE => EntryKind::Directory,
                catalog::INODE_SYMLINK_TYPE => EntryKind::Symlink,
                _ => EntryKind::File,
            },
            size,
            create_time: inode.create_time,
            modify_time: inode.modify_time,
            uid: inode.uid,
            gid: inode.gid,
            mode: inode.mode,
            nlink: inode.nlink(),
            compression,
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
        let small_file = walk.iter().find(|e| {
            e.entry.kind == EntryKind::File && e.entry.size > 0 && e.entry.size < 1_000_000
        });

        let entry = small_file.expect("Should find a small file in the test image");
        let data = vol.read_file(&entry.path).unwrap();
        assert_eq!(
            data.len() as u64,
            entry.entry.size,
            "Read size should match stat size"
        );

        let stat = vol.stat(&entry.path).unwrap();
        assert_eq!(stat.size, entry.entry.size);
    }

    /// Requires ../tests/appfs.raw fixture. Run with `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn test_read_symlink_targets() {
        let file = std::fs::File::open("../tests/appfs.raw").unwrap();
        let reader = BufReader::new(file);

        let mut vol = ApfsVolume::open(reader).unwrap();

        let walk = vol.walk().unwrap();
        let symlinks: Vec<_> = walk
            .iter()
            .filter(|e| e.entry.kind == EntryKind::Symlink)
            .map(|e| e.path.clone())
            .collect();
        assert!(
            !symlinks.is_empty(),
            "Test image should contain symlinks to read"
        );

        for path in &symlinks {
            let target = vol.read_file(path).unwrap();
            assert!(
                !target.is_empty(),
                "Symlink {path} should resolve to a non-empty target"
            );
            assert!(
                !target.contains(&0),
                "Symlink target for {path} should have its trailing NUL stripped"
            );

            // stat() reports the target length, not the inode's zero size.
            let stat = vol.stat(path).unwrap();
            assert_eq!(stat.kind, EntryKind::Symlink);
            assert_eq!(
                stat.size,
                target.len() as u64,
                "stat size should match the target length for {path}"
            );
        }
    }

    /// Requires ../tests/appfs.raw. Run with `cargo test -- --ignored`.
    ///
    /// Walks every file on the fixture, lists its extended attributes and
    /// fetches each one back. Measured Sep 2026: 129 files, 126 carrying
    /// attributes, 610 values across seven names — `com.apple.provenance`
    /// (298), the four `com.apple.cs.*` code-signing attributes (74 each),
    /// `com.apple.fs.symlink` (15) and `com.apple.FinderInfo` (1). All 610
    /// resolve, so the key comparator and both storage forms are exercised
    /// against a real volume rather than only synthetically.
    ///
    /// The fixture has no decmpfs-compressed file, so it cannot cover
    /// decompression; `cmpfs` and the synthetic hfsplus tests do that.
    #[test]
    #[ignore]
    fn lists_and_reads_every_xattr_on_the_fixture() {
        let file = std::fs::File::open("../tests/appfs.raw").unwrap();
        let mut vol = ApfsVolume::open(std::io::BufReader::new(file)).unwrap();

        let entries = vol.walk().unwrap();
        let mut listed = 0usize;
        let mut fetched = 0usize;

        for entry in &entries {
            for name in vol.list_xattrs(&entry.path).unwrap() {
                listed += 1;
                let value = vol.get_xattr(&entry.path, &name).unwrap();
                assert!(
                    value.is_some(),
                    "{} listed {name} but it could not be read back",
                    entry.path
                );
                fetched += 1;
            }
        }

        assert_eq!(listed, fetched);
        assert!(
            listed > 500,
            "expected hundreds of attributes, found {listed}"
        );
    }
}
