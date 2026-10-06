//! The complete tree: capture target, diff subject, mutation source.

use std::collections::BTreeMap;

use crate::change::ChangeSet;
use crate::error::DomainError;
use crate::name::NodeName;
use crate::node::Node;
use crate::path::NodePath;

/// A complete, validated filesystem tree. The root is always a directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FsTree(Node);

impl Default for FsTree {
    fn default() -> Self {
        Self::empty()
    }
}

impl FsTree {
    pub fn empty() -> Self {
        Self(Node::dir())
    }

    pub fn from_children(children: BTreeMap<NodeName, Node>) -> Self {
        Self(Node::Dir { children })
    }

    /// The root node; always a directory.
    pub fn root(&self) -> &Node {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.root().children().is_some_and(BTreeMap::is_empty)
    }

    /// Insert `node` at `path`, creating intermediate directories.
    ///
    /// Fails if a non-terminal component exists and is not a directory.
    /// A terminal node of any kind is replaced.
    pub fn insert(&mut self, path: &NodePath, node: Node) -> Result<(), DomainError> {
        let mut current = &mut self.0;
        for (index, name) in path.components().iter().enumerate() {
            let Node::Dir { children } = current else {
                return Err(DomainError::NotADirectory {
                    path: NodePath::from_components(&path.components()[..index]),
                });
            };
            current = children.entry(name.clone()).or_insert_with(Node::dir);
        }
        if path.is_root() && !node.is_dir() {
            return Err(DomainError::NotADirectory { path: path.clone() });
        }
        *current = node;
        Ok(())
    }

    pub fn get(&self, path: &NodePath) -> Option<&Node> {
        let mut current = &self.0;
        for name in path.components() {
            current = current.children()?.get(name)?;
        }
        Some(current)
    }

    /// The [`ChangeSet`] describing how to mutate `self` (the baseline)
    /// into `other` (the desired tree).
    ///
    /// Pure: no I/O, no clock, no randomness. Two identical trees produce
    /// an empty change set.
    pub fn diff(&self, other: &Self) -> ChangeSet {
        let mut changes = ChangeSet::new();
        let mut path = NodePath::root();
        diff_nodes(self.root(), other.root(), &mut path, &mut changes);
        changes
    }
}

fn diff_nodes(baseline: &Node, desired: &Node, path: &mut NodePath, out: &mut ChangeSet) {
    match (baseline, desired) {
        (Node::Dir { children: old }, Node::Dir { children: new }) => {
            for (name, desired_child) in new {
                path.push(name.clone());
                match old.get(name) {
                    Some(baseline_child) => diff_nodes(baseline_child, desired_child, path, out),
                    None => out.record_created(path.clone()),
                }
                path.pop();
            }
            for name in old.keys() {
                if !new.contains_key(name) {
                    path.push(name.clone());
                    out.record_extraneous(path.clone());
                    path.pop();
                }
            }
        }
        _ if baseline == desired => {}
        _ => out.record_overwritten(path.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::FileMode;

    fn name(s: &str) -> NodeName {
        NodeName::new(s).expect("valid name")
    }

    fn path(s: &str) -> NodePath {
        s.split('/').map(name).collect::<Vec<_>>().into()
    }

    fn tree(entries: &[(&str, Node)]) -> FsTree {
        let mut tree = FsTree::empty();
        for (p, node) in entries {
            tree.insert(&path(p), node.clone()).expect("insert");
        }
        tree
    }

    #[test]
    fn empty_trees_have_no_changes() {
        assert!(FsTree::empty().diff(&FsTree::empty()).is_empty());
    }

    #[test]
    fn insert_creates_intermediate_directories() {
        let mut tree = FsTree::empty();
        tree.insert(&path("a/b/c"), Node::file("x", FileMode::DEFAULT))
            .expect("insert");

        assert!(tree.get(&path("a/b/c")).is_some());
        assert!(tree.get(&path("a")).expect("a").is_dir());
        assert!(tree.get(&path("a/b")).expect("a/b").is_dir());
        assert!(tree.get(&path("a/b/c")).is_some());
    }

    #[test]
    fn insert_rejects_file_ancestors() {
        let mut tree = FsTree::empty();
        tree.insert(&path("f"), Node::file("x", FileMode::DEFAULT))
            .expect("insert");
        let error = tree
            .insert(&path("f/child"), Node::dir())
            .expect_err("file cannot be an ancestor");
        assert_eq!(error, DomainError::NotADirectory { path: path("f") });
    }

    #[test]
    fn diff_reports_created_overwritten_extraneous() {
        let before = tree(&[
            ("keep", Node::file("same", FileMode::DEFAULT)),
            ("change", Node::file("old", FileMode::DEFAULT)),
            ("gone", Node::dir()),
        ]);
        let after = tree(&[
            ("keep", Node::file("same", FileMode::DEFAULT)),
            ("change", Node::file("new", FileMode::DEFAULT)),
            ("new", Node::file("n", FileMode::DEFAULT)),
        ]);

        let changes = before.diff(&after);
        let created: Vec<String> = changes.created().map(|p| p.to_string()).collect();
        let overwritten: Vec<String> = changes.overwritten().map(|p| p.to_string()).collect();
        let extraneous: Vec<String> = changes.extraneous().map(|p| p.to_string()).collect();

        assert_eq!(created, ["new"]);
        assert_eq!(overwritten, ["change"]);
        assert_eq!(extraneous, ["gone"]);

        // identical trees diff to nothing
        assert!(after.diff(&after).is_empty());
    }

    #[test]
    fn diff_deep_changes_stay_scoped() {
        let before = tree(&[
            ("d/keep", Node::file("same", FileMode::DEFAULT)),
            ("d/change", Node::file("old", FileMode::DEFAULT)),
        ]);
        let after = tree(&[
            ("d/keep", Node::file("same", FileMode::DEFAULT)),
            ("d/change", Node::file("new", FileMode::DEFAULT)),
        ]);

        let changes = before.diff(&after);
        let overwritten: Vec<String> = changes.overwritten().map(|p| p.to_string()).collect();
        assert_eq!(overwritten, ["d/change"]);
    }

    #[test]
    fn kind_change_overwrites_whole_path() {
        let before = tree(&[("a", Node::dir())]);
        let after = tree(&[("a", Node::file("now a file", FileMode::DEFAULT))]);

        let changes = before.diff(&after);
        let overwritten: Vec<String> = changes.overwritten().map(|p| p.to_string()).collect();
        assert_eq!(overwritten, ["a"]);
        // the old directory's children are covered by the overwrite at "a"
        assert!(changes.extraneous().next().is_none());
    }

    #[test]
    fn mode_change_counts_as_overwrite() {
        let before = tree(&[("s", Node::file("x", FileMode::DEFAULT))]);
        let after = tree(&[("s", Node::file("x", FileMode::EXECUTABLE))]);

        let changes = before.diff(&after);
        assert_eq!(changes.counts(), (0, 1, 0));
    }
}
