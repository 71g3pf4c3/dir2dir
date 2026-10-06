//! Application layer: use cases and ports.
//!
//! Use cases orchestrate; ports are the contracts they orchestrate through.
//! This layer knows about trees and plans, but nothing about the filesystem
//! or about JSON — those are adapter concerns living in
//! `dir2dir-infrastructure`.

pub mod error;
pub mod ports;
pub mod use_cases;

pub use error::{CodecError, SinkError, SourceError, UseCaseError};
pub use ports::{ApplyOptions, TreeCodec, TreeSink, TreeSource};
pub use use_cases::{CopyDir, CopyOptions, DiffDirs, ExportTree, ImportTree};
