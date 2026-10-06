//! Use cases: the verbs of the tool.

use std::path::Path;

use dir2dir_domain::ChangeSet;

use crate::error::UseCaseError;
use crate::ports::{ApplyOptions, TreeCodec, TreeSink, TreeSource};

/// Options for [`CopyDir::execute`].
#[derive(Debug, Clone, Copy, Default)]
pub struct CopyOptions {
    /// Compute and report the plan; touch nothing.
    pub dry_run: bool,
    /// Delete destination paths that are absent from the source tree.
    pub prune: bool,
}

/// dir → dir: capture the source into the model, plan the destination
/// mutation, optionally apply it.
///
/// The returned [`ChangeSet`] is the plan that was (or would have been)
/// executed.
pub struct CopyDir<'a> {
    source: &'a dyn TreeSource,
    sink: &'a dyn TreeSink,
}

impl<'a> CopyDir<'a> {
    pub fn new(source: &'a dyn TreeSource, sink: &'a dyn TreeSink) -> Self {
        Self { source, sink }
    }

    pub fn execute(
        &self,
        source_root: &Path,
        destination_root: &Path,
        options: &CopyOptions,
    ) -> Result<ChangeSet, UseCaseError> {
        let tree = self.source.read(source_root)?;
        let plan = self.sink.plan(destination_root, &tree)?;
        if !options.dry_run {
            self.sink.apply(
                destination_root,
                &tree,
                &plan,
                &ApplyOptions {
                    prune: options.prune,
                },
            )?;
        }
        Ok(plan)
    }
}

/// dir → JSON: capture the source and serialize it.
pub struct ExportTree<'a> {
    source: &'a dyn TreeSource,
    codec: &'a dyn TreeCodec,
}

impl<'a> ExportTree<'a> {
    pub fn new(source: &'a dyn TreeSource, codec: &'a dyn TreeCodec) -> Self {
        Self { source, codec }
    }

    pub fn execute(&self, root: &Path) -> Result<String, UseCaseError> {
        let tree = self.source.read(root)?;
        Ok(self.codec.serialize(&tree)?)
    }
}

/// JSON → dir: deserialize a document and materialize it.
pub struct ImportTree<'a> {
    codec: &'a dyn TreeCodec,
    sink: &'a dyn TreeSink,
}

impl<'a> ImportTree<'a> {
    pub fn new(codec: &'a dyn TreeCodec, sink: &'a dyn TreeSink) -> Self {
        Self { codec, sink }
    }

    pub fn execute(
        &self,
        root: &Path,
        document: &str,
        options: &ApplyOptions,
    ) -> Result<ChangeSet, UseCaseError> {
        let tree = self.codec.deserialize(document)?;
        let plan = self.sink.plan(root, &tree)?;
        self.sink.apply(root, &tree, &plan, options)?;
        Ok(plan)
    }
}

/// dir vs dir: capture both trees, diff in the model.
///
/// The result reads as a mutation of `left` into `right`.
pub struct DiffDirs<'a> {
    source: &'a dyn TreeSource,
}

impl<'a> DiffDirs<'a> {
    pub fn new(source: &'a dyn TreeSource) -> Self {
        Self { source }
    }

    pub fn execute(&self, left: &Path, right: &Path) -> Result<ChangeSet, UseCaseError> {
        let left_tree = self.source.read(left)?;
        let right_tree = self.source.read(right)?;
        Ok(left_tree.diff(&right_tree))
    }
}
