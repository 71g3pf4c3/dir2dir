//! Validated strictly-relative tree paths.

use std::fmt;

use crate::name::NodeName;

/// A validated, strictly relative path inside a tree.
///
/// A `NodePath` is a sequence of [`NodeName`] segments. The root is the
/// empty path; there is no way to express an absolute path, a parent
/// traversal or a rooted path — by construction.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct NodePath(Vec<NodeName>);

impl NodePath {
    pub fn root() -> Self {
        Self(Vec::new())
    }

    pub fn is_root(&self) -> bool {
        self.0.is_empty()
    }

    pub fn push(&mut self, name: NodeName) {
        self.0.push(name);
    }

    pub fn pop(&mut self) -> Option<NodeName> {
        self.0.pop()
    }

    pub fn components(&self) -> &[NodeName] {
        &self.0
    }

    pub fn depth(&self) -> usize {
        self.0.len()
    }

    pub(crate) fn from_components(components: &[NodeName]) -> Self {
        Self(components.to_vec())
    }
}

impl fmt::Display for NodePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_root() {
            return f.write_str(".");
        }
        let mut first = true;
        for name in &self.0 {
            if !first {
                f.write_str("/")?;
            }
            write!(f, "{name}")?;
            first = false;
        }
        Ok(())
    }
}

impl From<Vec<NodeName>> for NodePath {
    fn from(components: Vec<NodeName>) -> Self {
        Self(components)
    }
}

impl<'a> FromIterator<&'a NodeName> for NodePath {
    fn from_iter<I: IntoIterator<Item = &'a NodeName>>(iter: I) -> Self {
        Self(iter.into_iter().cloned().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(s: &str) -> NodeName {
        NodeName::new(s).expect("valid name")
    }

    #[test]
    fn displays_like_a_relative_path() {
        let mut path = NodePath::root();
        path.push(name("a"));
        path.push(name("b c"));
        assert_eq!(path.to_string(), "a/b c");
        assert_eq!(NodePath::root().to_string(), ".");
    }

    #[test]
    fn push_pop_roundtrip() {
        let mut path = NodePath::root();
        path.push(name("x"));
        assert_eq!(path.pop(), Some(name("x")));
        assert!(path.is_root());
        assert_eq!(path.pop(), None);
    }
}
