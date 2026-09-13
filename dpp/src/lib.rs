pub mod error;
pub mod pipeline;

pub use error::{DppError, Result};
#[cfg(feature = "extract")]
pub use pipeline::ExtractStats;
pub use pipeline::{
    ApfsHandle, DmgPipeline, ExtractMode, FilesystemHandle, FsDirEntry, FsEntryKind, FsFileStat,
    FsType, FsVolumeInfo, FsWalkEntry, FsXattr, HfsHandle,
};

/// Whether an extended attribute is user data or compression machinery.
///
/// Re-exported through `apfs`, which shares the type with `hfsplus`, so `dpp`
/// needs no direct dependency on `cmpfs`.
pub use apfs::XattrKind;

// Re-export underlying crates
pub use apfs;
pub use hfsplus;
pub use pbzx;
pub use udif;
pub use xara;

#[cfg(test)]
mod tests;
