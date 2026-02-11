pub mod error;
pub mod pipeline;

pub use error::{DppError, Result};
pub use pipeline::{
    DmgPipeline, ExtractMode, HfsHandle, ApfsHandle,
    FilesystemHandle, FsDirEntry, FsWalkEntry, FsEntryKind,
    FsType, FsFileStat, FsVolumeInfo,
};

// Re-export underlying crates
pub use hfsplus;
pub use apfs;
pub use xara;
pub use udif;
pub use pbzx;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_mode_default() {
        assert!(matches!(ExtractMode::default(), ExtractMode::TempFile));
    }

    #[test]
    fn test_fs_type_equality() {
        assert_eq!(FsType::HfsPlus, FsType::HfsPlus);
        assert_eq!(FsType::Apfs, FsType::Apfs);
        assert_ne!(FsType::HfsPlus, FsType::Apfs);
    }

    #[test]
    fn test_fs_file_stat_from_hfs() {
        let hfs_stat = hfsplus::FileStat {
            cnid: 42,
            kind: hfsplus::EntryKind::File,
            size: 1024,
            create_date: 100,
            modify_date: 200,
            permissions: hfsplus::HfsPermissions {
                owner_id: 501,
                group_id: 20,
                mode: 0o100644,
            },
            data_fork_extents: 3,
            resource_fork_size: 512,
        };

        let unified = FsFileStat::from(&hfs_stat);
        assert_eq!(unified.fs_type, FsType::HfsPlus);
        assert_eq!(unified.id, 42);
        assert_eq!(unified.kind, FsEntryKind::File);
        assert_eq!(unified.size, 1024);
        assert_eq!(unified.uid, 501);
        assert_eq!(unified.gid, 20);
        assert_eq!(unified.mode, 0o100644);
        assert_eq!(unified.create_time, 100);
        assert_eq!(unified.modify_time, 200);
        assert_eq!(unified.nlink, None);
        assert_eq!(unified.data_fork_extents, Some(3));
        assert_eq!(unified.resource_fork_size, Some(512));
    }

    #[test]
    fn test_fs_file_stat_from_hfs_no_resource_fork() {
        let hfs_stat = hfsplus::FileStat {
            cnid: 10,
            kind: hfsplus::EntryKind::Directory,
            size: 0,
            create_date: 50,
            modify_date: 60,
            permissions: hfsplus::HfsPermissions {
                owner_id: 0,
                group_id: 0,
                mode: 0o040755,
            },
            data_fork_extents: 0,
            resource_fork_size: 0,
        };

        let unified = FsFileStat::from(&hfs_stat);
        assert_eq!(unified.resource_fork_size, None);
    }

    #[test]
    fn test_fs_file_stat_from_apfs() {
        let apfs_stat = apfs::FileStat {
            oid: 999,
            kind: apfs::EntryKind::Symlink,
            size: 48,
            create_time: 1_000_000_000,
            modify_time: 2_000_000_000,
            uid: 0,
            gid: 80,
            mode: 0o120755,
            nlink: 2,
        };

        let unified = FsFileStat::from(&apfs_stat);
        assert_eq!(unified.fs_type, FsType::Apfs);
        assert_eq!(unified.id, 999);
        assert_eq!(unified.kind, FsEntryKind::Symlink);
        assert_eq!(unified.size, 48);
        assert_eq!(unified.uid, 0);
        assert_eq!(unified.gid, 80);
        assert_eq!(unified.mode, 0o120755);
        assert_eq!(unified.create_time, 1_000_000_000);
        assert_eq!(unified.modify_time, 2_000_000_000);
        assert_eq!(unified.nlink, Some(2));
        assert_eq!(unified.data_fork_extents, None);
        assert_eq!(unified.resource_fork_size, None);
    }

    #[test]
    fn test_error_display() {
        let err = DppError::NoHfsPartition;
        assert_eq!(err.to_string(), "no HFS+ partition found in DMG");

        let err = DppError::NoApfsPartition;
        assert_eq!(err.to_string(), "no APFS partition found in DMG");

        let err = DppError::NoFilesystemPartition;
        assert_eq!(err.to_string(), "no filesystem partition found in DMG");

        let err = DppError::FileNotFound("test.pkg".to_string());
        assert_eq!(err.to_string(), "file not found: test.pkg");
    }
}
