//! Pure filesystem tree model for dir2dir.
//!
//! This crate is the domain layer: zero dependencies, zero I/O. It defines
//! the tree model ([`FsTree`]), its invariants ([`NodeName`], [`NodePath`])
//! and the pure operations over it ([`FsTree::diff`]). Everything that
//! touches the outside world lives in `dir2dir-infrastructure`; everything
//! that orchestrates lives in `dir2dir-application`.
//!
//! The dependency rule: every other crate may depend on this one. This crate
//! depends on nothing.

pub mod change;
pub mod error;
pub mod name;
pub mod node;
pub mod path;
pub mod tree;

pub use change::ChangeSet;
pub use error::{DomainError, InvalidNodeName};
pub use name::NodeName;
pub use node::{FileMode, Node};
pub use path::NodePath;
pub use tree::FsTree;
