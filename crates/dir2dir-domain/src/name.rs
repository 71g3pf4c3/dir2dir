//! Validated single path segments.

use std::fmt;

use crate::error::InvalidNodeName;

/// A validated single path segment.
///
/// Construction is the validation boundary: every [`crate::Node`] in a
/// [`crate::FsTree`] has a `NodeName`, so no rooted path, no `..` traversal,
/// no embedded separators and no NUL bytes can enter the model — regardless
/// of which codec or adapter built the tree.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeName(String);

impl NodeName {
    pub fn new(raw: impl Into<String>) -> Result<Self, InvalidNodeName> {
        let raw = raw.into();
        match raw.as_str() {
            "" => Err(InvalidNodeName::Empty),
            "." => Err(InvalidNodeName::CurrentDir),
            ".." => Err(InvalidNodeName::ParentDir),
            _ if raw.contains('/') => Err(InvalidNodeName::PathSeparator),
            _ if raw.contains('\0') => Err(InvalidNodeName::NulByte),
            _ => Ok(Self(raw)),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for NodeName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for NodeName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_ordinary_names() {
        assert!(NodeName::new("a").is_ok());
        assert!(NodeName::new(".hidden").is_ok());
        assert!(NodeName::new("with spaces and ünicode").is_ok());
    }

    #[test]
    fn rejects_special_segments() {
        assert_eq!(NodeName::new(""), Err(InvalidNodeName::Empty));
        assert_eq!(NodeName::new("."), Err(InvalidNodeName::CurrentDir));
        assert_eq!(NodeName::new(".."), Err(InvalidNodeName::ParentDir));
        assert_eq!(NodeName::new("a/b"), Err(InvalidNodeName::PathSeparator));
        assert_eq!(NodeName::new("a\0b"), Err(InvalidNodeName::NulByte));
    }
}
