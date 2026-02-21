//! Filesystem extraction for XAR archives.

use std::path::{Component, Path, PathBuf};

use crate::XarArchive;
use crate::error::{Result, XarError};
use crate::toc::XarFileType;
use std::io::{Read, Seek};

/// Statistics returned after extraction.
#[derive(Debug, Clone, Default)]
pub struct ExtractStats {
    /// Number of regular files extracted
    pub files: u64,
    /// Number of directories created
    pub dirs: u64,
    /// Number of symlinks skipped (symlinks are never created during extraction)
    pub symlinks_skipped: u64,
    /// Total bytes written to disk
    pub bytes: u64,
}

/// Strip a normalized base prefix from a path, returning only the remainder.
///
/// Returns `""` when `path` is the base itself, and the relative tail when
/// `path` starts with `prefix/`.
fn strip_base_prefix<'a>(path: &'a str, prefix: &str) -> &'a str {
    if prefix.is_empty() {
        path
    } else {
        path.strip_prefix(prefix)
            .and_then(|s| s.strip_prefix('/'))
            .unwrap_or("")
    }
}

/// Sanitize a path: reject `..` components and absolute prefixes.
fn sanitize_path(path: &str) -> Result<PathBuf> {
    let p = Path::new(path);
    let mut clean = PathBuf::new();
    for component in p.components() {
        match component {
            Component::Normal(c) => clean.push(c),
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(XarError::InvalidPath(format!(
                    "path traversal detected: {}",
                    path
                )));
            }
            Component::RootDir | Component::Prefix(_) => {}
        }
    }
    if clean.as_os_str().is_empty() {
        return Err(XarError::InvalidPath(format!(
            "empty path after sanitization: {}",
            path
        )));
    }
    Ok(clean)
}

/// Build a sort key that produces depth-first tree order (dirs before files at each level).
fn tree_sort_key(path: &str, is_dir: bool) -> String {
    let parts: Vec<&str> = path.split('/').collect();
    let mut key_parts = Vec::with_capacity(parts.len());
    for (i, part) in parts.iter().enumerate() {
        let is_last = i == parts.len() - 1;
        let prefix = if !is_last || is_dir { "\x00" } else { "\x01" };
        key_parts.push(format!("{prefix}{part}"));
    }
    key_parts.join("/")
}

pub(crate) fn extract_inner<R: Read + Seek>(
    archive: &mut XarArchive<R>,
    base_path: &str,
    dest: &Path,
) -> Result<ExtractStats> {
    let prefix = base_path.trim_start_matches('/').trim_end_matches('/');
    let extract_all = prefix.is_empty();

    // Clone the file list so we can call &mut self methods on the archive
    let files = archive.files.clone();

    // Filter by path prefix
    let mut filtered: Vec<_> = files
        .iter()
        .filter(|f| {
            if extract_all {
                true
            } else {
                f.path == prefix || f.path.starts_with(&format!("{prefix}/"))
            }
        })
        .collect();

    // Sort: directories first so parent dirs are created before children
    filtered.sort_by(|a, b| {
        let a_is_dir = a.file_type == XarFileType::Directory;
        let b_is_dir = b.file_type == XarFileType::Directory;
        let a_key = tree_sort_key(&a.path, a_is_dir);
        let b_key = tree_sort_key(&b.path, b_is_dir);
        a_key.cmp(&b_key)
    });

    std::fs::create_dir_all(dest)?;

    let mut stats = ExtractStats::default();

    for file in &filtered {
        // Strip base prefix so only the relative remainder is extracted
        let rel = if extract_all {
            file.path.as_str()
        } else {
            strip_base_prefix(&file.path, prefix)
        };

        // The base dir itself maps to dest (already created above)
        if rel.is_empty() {
            if file.file_type == XarFileType::Directory {
                stats.dirs += 1;
            }
            continue;
        }

        let clean = sanitize_path(rel)?;
        let dest_path = dest.join(&clean);

        match file.file_type {
            XarFileType::Directory => {
                std::fs::create_dir_all(&dest_path)?;
                stats.dirs += 1;
            }
            XarFileType::File => {
                if let Some(parent) = dest_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let mut out_file = std::fs::File::create(&dest_path)?;
                let n = archive.read_file_to(file, &mut out_file)?;
                stats.bytes += n;
                stats.files += 1;
            }
            XarFileType::Symlink => {
                stats.symlinks_skipped += 1;
            }
        }
    }

    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_path() {
        assert!(sanitize_path("normal/path/file.txt").is_ok());
        assert!(sanitize_path("/absolute/path").is_ok());
        assert!(sanitize_path("../traversal").is_err());
        assert!(sanitize_path("path/../traversal").is_err());
    }

    #[test]
    fn test_sanitize_path_strips_root() {
        let p = sanitize_path("/usr/lib/foo.dylib").unwrap();
        assert_eq!(p, PathBuf::from("usr/lib/foo.dylib"));
    }
}
