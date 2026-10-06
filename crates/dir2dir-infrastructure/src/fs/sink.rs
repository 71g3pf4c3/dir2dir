//! [`TreeSink`] backed by the local filesystem.
//!
//! Two-phase, plan-driven writes:
//!
//! - `plan` diffs the captured destination baseline against the desired
//!   tree — no mutation happens here;
//! - `apply` writes only the nodes the plan marks dirty (a marked node
//!   implies its whole subtree), and prunes extraneous paths when asked.
//!
//! Symlink policy: the sink never writes *through* an existing symlink at
//! a planned path — whatever occupies the path is removed first, then the
//! node is created. This closes the worst TOCTOU hole (pre-seeded symlink
//! farms), though it is not a full hardening; there is still a window
//! between remove and create.

use std::fs;
use std::path::{Path, PathBuf};

use dir2dir_application::ports::{ApplyOptions, TreeSink};
use dir2dir_application::SinkError;
use dir2dir_domain::{ChangeSet, FileMode, FsTree, Node, NodePath};

use super::scan;

/// Writes directory trees to the local filesystem.
#[derive(Debug, Clone, Copy, Default)]
pub struct FsSink;

impl TreeSink for FsSink {
    fn plan(&self, root: &Path, tree: &FsTree) -> Result<ChangeSet, SinkError> {
        let baseline = match fs::symlink_metadata(root) {
            Ok(metadata) if metadata.is_dir() => {
                scan::scan_tree(root).map_err(|source| match source {
                    // A sink read failure is an I/O problem of the destination.
                    dir2dir_application::SourceError::Io { path, source } => {
                        SinkError::Io { path, source }
                    }
                    other => SinkError::Io {
                        path: root.to_owned(),
                        source: std::io::Error::other(other.to_string()),
                    },
                })?
            }
            Ok(_) => {
                return Err(SinkError::DestinationNotDirectory {
                    path: root.to_owned(),
                })
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => FsTree::empty(),
            Err(e) => {
                return Err(SinkError::Io {
                    path: root.to_owned(),
                    source: e,
                })
            }
        };
        Ok(baseline.diff(tree))
    }

    fn apply(
        &self,
        root: &Path,
        tree: &FsTree,
        plan: &ChangeSet,
        options: &ApplyOptions,
    ) -> Result<(), SinkError> {
        // Even an empty tree materializes the destination directory itself.
        fs::create_dir_all(root).map_err(|e| io_err(root, e))?;

        if plan.is_empty() && !options.prune {
            return Ok(());
        }

        self.write_subtree(root, tree.root(), plan)?;

        if options.prune {
            self.prune(root, plan)?;
        }
        Ok(())
    }
}

impl FsSink {
    fn write_subtree(&self, root: &Path, node: &Node, plan: &ChangeSet) -> Result<(), SinkError> {
        let mut rel = NodePath::root();
        self.write_node(root, node, &mut rel, plan, false)
    }

    /// `inherited_dirty`: an ancestor is in the plan, so this whole
    /// subtree is rewritten regardless of per-node plan membership.
    fn write_node(
        &self,
        root: &Path,
        node: &Node,
        rel: &mut NodePath,
        plan: &ChangeSet,
        inherited_dirty: bool,
    ) -> Result<(), SinkError> {
        let dirty = inherited_dirty || plan.contains(rel);
        let abs = absolute(root, rel);

        match node {
            Node::Dir { children } => {
                ensure_directory(&abs)?;
                for (name, child) in children {
                    rel.push(name.clone());
                    self.write_node(root, child, rel, plan, dirty)?;
                    rel.pop();
                }
            }
            Node::File { content, mode } => {
                if dirty {
                    write_file(&abs, content, *mode)?;
                }
            }
            Node::Symlink { target } => {
                if dirty {
                    write_symlink(&abs, target)?;
                }
            }
        }
        Ok(())
    }

    /// Delete extraneous paths (top-most first: `remove_dir_all` covers
    /// whatever lives below them).
    fn prune(&self, root: &Path, plan: &ChangeSet) -> Result<(), SinkError> {
        for rel in plan.extraneous() {
            let abs = absolute(root, rel);
            match fs::symlink_metadata(&abs) {
                Ok(metadata) if metadata.is_dir() => {
                    fs::remove_dir_all(&abs).map_err(|e| io_err(&abs, e))?;
                }
                Ok(_) => {
                    fs::remove_file(&abs).map_err(|e| io_err(&abs, e))?;
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(io_err(&abs, e)),
            }
        }
        Ok(())
    }
}

/// `root` joined with a validated relative path.
fn absolute(root: &Path, rel: &NodePath) -> PathBuf {
    let mut abs = root.to_path_buf();
    for name in rel.components() {
        abs.push(name.as_str());
    }
    abs
}

/// Make sure `path` is a directory, replacing anything else that occupies
/// it. For already-existing directories this is a no-op — their children
/// are managed by the plan, not by blanket clearing.
fn ensure_directory(path: &Path) -> Result<(), SinkError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => Ok(()),
        Ok(_) => {
            remove_existing(path)?;
            fs::create_dir_all(path).map_err(|e| io_err(path, e))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir_all(path).map_err(|e| io_err(path, e))
        }
        Err(e) => Err(io_err(path, e)),
    }
}

/// Remove whatever occupies `path` — file, symlink or directory — without
/// following symlinks. Missing is fine.
fn remove_existing(path: &Path) -> Result<(), SinkError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(path).map_err(|e| io_err(path, e)),
        Ok(_) => fs::remove_file(path).map_err(|e| io_err(path, e)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(io_err(path, e)),
    }
}

/// Delete-then-write: the new node is always freshly created, so we never
/// write *through* a pre-existing symlink.
fn write_file(path: &Path, content: &[u8], mode: FileMode) -> Result<(), SinkError> {
    remove_existing(path)?;
    fs::write(path, content).map_err(|e| io_err(path, e))?;
    set_mode(path, mode)
}

fn write_symlink(path: &Path, target: &Path) -> Result<(), SinkError> {
    remove_existing(path)?;
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, path).map_err(|e| io_err(path, e))
    }
    #[cfg(not(unix))]
    {
        let _ = target;
        Err(io_err(
            path,
            std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "symlinks are not supported on this platform",
            ),
        ))
    }
}

fn set_mode(path: &Path, mode: FileMode) -> Result<(), SinkError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(mode.bits()))
            .map_err(|e| io_err(path, e))
    }
    #[cfg(not(unix))]
    {
        let _ = (path, mode);
        Ok(())
    }
}

fn io_err(path: &Path, source: std::io::Error) -> SinkError {
    SinkError::Io {
        path: path.to_owned(),
        source,
    }
}
