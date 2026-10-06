//! The change set: a pure, inspectable plan.

use std::collections::BTreeSet;

use crate::path::NodePath;

/// The planned difference between a baseline tree and a desired tree.
///
/// Semantics:
///
/// - a **created** path is absent from the baseline;
/// - an **overwritten** path exists in the baseline but differs — a kind
///   change at the path implies its whole subtree below is rewritten;
/// - an **extraneous** path exists in the baseline but not in the desired
///   tree; it is only deleted when pruning is requested.
///
/// A `ChangeSet` is produced before any mutation and is the contract for
/// what a sink may touch. Empty set, empty side effects.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChangeSet {
    created: BTreeSet<NodePath>,
    overwritten: BTreeSet<NodePath>,
    extraneous: BTreeSet<NodePath>,
}

impl ChangeSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_created(&mut self, path: NodePath) {
        self.created.insert(path);
    }

    pub fn record_overwritten(&mut self, path: NodePath) {
        self.overwritten.insert(path);
    }

    pub fn record_extraneous(&mut self, path: NodePath) {
        self.extraneous.insert(path);
    }

    pub fn created(&self) -> impl Iterator<Item = &NodePath> {
        self.created.iter()
    }

    pub fn overwritten(&self) -> impl Iterator<Item = &NodePath> {
        self.overwritten.iter()
    }

    pub fn extraneous(&self) -> impl Iterator<Item = &NodePath> {
        self.extraneous.iter()
    }

    /// Whether a mutation is planned for this exact path.
    pub fn contains(&self, path: &NodePath) -> bool {
        self.created.contains(path) || self.overwritten.contains(path)
    }

    pub fn is_empty(&self) -> bool {
        self.created.is_empty() && self.overwritten.is_empty() && self.extraneous.is_empty()
    }

    pub fn counts(&self) -> (usize, usize, usize) {
        (
            self.created.len(),
            self.overwritten.len(),
            self.extraneous.len(),
        )
    }
}
