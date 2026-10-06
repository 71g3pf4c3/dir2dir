//! Ports: the contracts between use cases and the outside world.

use std::path::Path;

use dir2dir_domain::{ChangeSet, FsTree};

use crate::error::{CodecError, SinkError, SourceError};

/// Reads a directory tree into the model.
pub trait TreeSource: Send + Sync {
    fn read(&self, root: &Path) -> Result<FsTree, SourceError>;
}

/// Materializes the model into a directory tree.
///
/// Sinks are two-phase on purpose:
///
/// - [`TreeSink::plan`] computes the [`ChangeSet`] required to bring `root`
///   in line with `tree` without touching anything;
/// - [`TreeSink::apply`] executes a previously computed plan.
///
/// Splitting the phases is what makes dry-run, operator review and
/// auditability possible: the mutation is never a surprise.
pub trait TreeSink: Send + Sync {
    fn plan(&self, root: &Path, tree: &FsTree) -> Result<ChangeSet, SinkError>;

    fn apply(
        &self,
        root: &Path,
        tree: &FsTree,
        plan: &ChangeSet,
        options: &ApplyOptions,
    ) -> Result<(), SinkError>;
}

/// Options for [`TreeSink::apply`].
#[derive(Debug, Clone, Copy, Default)]
pub struct ApplyOptions {
    /// Delete paths that exist in the destination but not in the tree.
    pub prune: bool,
}

/// Serializes the model to and from a textual document.
pub trait TreeCodec: Send + Sync {
    fn serialize(&self, tree: &FsTree) -> Result<String, CodecError>;
    fn deserialize(&self, document: &str) -> Result<FsTree, CodecError>;
}
