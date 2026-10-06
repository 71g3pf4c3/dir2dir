//! Tree capture: filesystem → model.
//!
//! Shared by `FsSource` (reading sources) and `FsSink` (reading the
//! destination baseline for planning).
//!
//! Symlinks are captured as symlinks — never followed. Unrepresentable
//! objects (FIFOs, sockets, devices) are hard errors, not silent skips.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use dir2dir_application::SourceError;
use dir2dir_domain::{FileMode, FsTree, Node, NodeName};

/// Capture a directory tree into the model.
pub fn scan_tree(root: &Path) -> Result<FsTree, SourceError> {
    Ok(FsTree::from_children(scan_dir(root)?))
}

fn scan_dir(dir: &Path) -> Result<BTreeMap<NodeName, Node>, SourceError> {
    let mut children = BTreeMap::new();
    for entry in fs::read_dir(dir).map_err(|e| io_err(dir, e))? {
        let entry = entry.map_err(|e| io_err(dir, e))?;
        let raw_name = entry.file_name();
        let name = NodeName::new(raw_name.to_str().ok_or_else(|| SourceError::NonUtf8Name {
            name: raw_name.to_string_lossy().into_owned(),
        })?)
        .map_err(|reason| SourceError::InvalidName {
            name: raw_name.to_string_lossy().into_owned(),
            reason,
        })?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|e| io_err(&path, e))?;
        children.insert(name, scan_entry(&path, file_type)?);
    }
    Ok(children)
}

fn scan_entry(path: &Path, file_type: fs::FileType) -> Result<Node, SourceError> {
    if file_type.is_dir() {
        Ok(Node::Dir {
            children: scan_dir(path)?,
        })
    } else if file_type.is_file() {
        let content = fs::read(path).map_err(|e| io_err(path, e))?;
        Ok(Node::file(content, file_mode(path)?))
    } else if file_type.is_symlink() {
        let target = fs::read_link(path).map_err(|e| io_err(path, e))?;
        Ok(Node::symlink(target))
    } else {
        Err(SourceError::Unsupported {
            path: path.to_owned(),
            kind: "fifo, socket or device",
        })
    }
}

/// Read permission bits without following symlinks.
fn file_mode(path: &Path) -> Result<FileMode, SourceError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = fs::symlink_metadata(path).map_err(|e| io_err(path, e))?;
        Ok(FileMode::new(metadata.permissions().mode()))
    }
    #[cfg(not(unix))]
    {
        // No permission bits worth modeling here; files land as non-exec.
        let _ = path;
        Ok(FileMode::DEFAULT)
    }
}

fn io_err(path: &Path, source: std::io::Error) -> SourceError {
    SourceError::Io {
        path: path.to_owned(),
        source,
    }
}
