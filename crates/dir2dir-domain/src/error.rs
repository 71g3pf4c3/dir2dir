//! Domain errors. Hand-rolled, because the domain does not take dependencies.

use std::error::Error;
use std::fmt;

use crate::path::NodePath;

/// A node name failed model invariants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvalidNodeName {
    Empty,
    CurrentDir,
    ParentDir,
    PathSeparator,
    NulByte,
}

impl fmt::Display for InvalidNodeName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::Empty => "node name must not be empty",
            Self::CurrentDir => "node name must not be `.`",
            Self::ParentDir => "node name must not be `..`",
            Self::PathSeparator => "node name must not contain `/`",
            Self::NulByte => "node name must not contain NUL bytes",
        };
        f.write_str(message)
    }
}

impl Error for InvalidNodeName {}

/// A tree-structure violation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainError {
    /// A path component that must be a directory is a file or a symlink.
    NotADirectory { path: NodePath },
}

impl fmt::Display for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotADirectory { path } => write!(f, "not a directory: {path}"),
        }
    }
}

impl Error for DomainError {}
