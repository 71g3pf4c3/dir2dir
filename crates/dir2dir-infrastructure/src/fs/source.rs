//! [`TreeSource`] backed by the local filesystem.

use std::path::Path;

use dir2dir_application::ports::TreeSource;
use dir2dir_application::SourceError;
use dir2dir_domain::FsTree;

use super::scan::scan_tree;

/// Reads directory trees from the local filesystem.
///
/// The root must be a real directory — a symlinked root is rejected rather
/// than silently followed. Resolve it yourself if you want that.
#[derive(Debug, Clone, Copy, Default)]
pub struct FsSource;

impl TreeSource for FsSource {
    fn read(&self, root: &Path) -> Result<FsTree, SourceError> {
        let metadata = std::fs::symlink_metadata(root).map_err(|e| SourceError::Io {
            path: root.to_owned(),
            source: e,
        })?;
        if !metadata.is_dir() {
            return Err(SourceError::Unsupported {
                path: root.to_owned(),
                kind: "source root is not a directory",
            });
        }
        scan_tree(root)
    }
}
