//! Tree nodes: directories, files with modes, symlinks.

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::name::NodeName;

/// Unix permission bits (masked to `0o777`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FileMode(u32);

impl FileMode {
    pub const DEFAULT: Self = Self(0o644);
    pub const EXECUTABLE: Self = Self(0o755);

    /// Mask to permission bits; higher bits are discarded.
    pub fn new(bits: u32) -> Self {
        Self(bits & 0o777)
    }

    pub fn bits(self) -> u32 {
        self.0
    }

    pub fn is_executable(self) -> bool {
        self.0 & 0o111 != 0
    }
}

/// A node of the filesystem tree model.
///
/// Only what a directory tree actually needs: nested directories, byte
/// content with a mode, and symlinks. Everything else (FIFOs, sockets,
/// devices, hardlinks) is deliberately not representable — adapters must
/// fail loudly when they meet it instead of silently dropping data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    Dir { children: BTreeMap<NodeName, Node> },
    File { content: Vec<u8>, mode: FileMode },
    Symlink { target: PathBuf },
}

impl Node {
    pub fn dir() -> Self {
        Node::Dir {
            children: BTreeMap::new(),
        }
    }

    pub fn file(content: impl Into<Vec<u8>>, mode: FileMode) -> Self {
        Node::File {
            content: content.into(),
            mode,
        }
    }

    pub fn symlink(target: impl Into<PathBuf>) -> Self {
        Node::Symlink {
            target: target.into(),
        }
    }

    pub fn is_dir(&self) -> bool {
        matches!(self, Node::Dir { .. })
    }

    /// Children of a directory node; `None` for files and symlinks.
    pub fn children(&self) -> Option<&BTreeMap<NodeName, Node>> {
        match self {
            Node::Dir { children } => Some(children),
            _ => None,
        }
    }
}
