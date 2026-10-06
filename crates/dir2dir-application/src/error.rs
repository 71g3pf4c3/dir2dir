//! Port error contracts.
//!
//! The ports own their error types: adapters map their backend failures
//! into these, so use cases never see `std::io::Error` or
//! `serde_json::Error` and the application layer stays backend-agnostic.

use std::path::PathBuf;

use dir2dir_domain::InvalidNodeName;

/// Failures of [`crate::ports::TreeSource::read`].
#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    #[error("I/O error at {path}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("unsupported filesystem object at {path}: {kind}")]
    Unsupported { path: PathBuf, kind: &'static str },
    #[error("non-UTF-8 node name: {name:?}")]
    NonUtf8Name { name: String },
    #[error("invalid node name {name:?}")]
    InvalidName {
        name: String,
        #[source]
        reason: InvalidNodeName,
    },
}

/// Failures of [`crate::ports::TreeSink`].
#[derive(Debug, thiserror::Error)]
pub enum SinkError {
    #[error("I/O error at {path}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("destination exists and is not a directory: {path}")]
    DestinationNotDirectory { path: PathBuf },
}

/// Failures of [`crate::ports::TreeCodec`].
#[derive(Debug, thiserror::Error)]
pub enum CodecError {
    #[error("invalid document: {message}")]
    InvalidDocument { message: String },
    #[error("document violates the tree model: {message}")]
    InvalidModel { message: String },
    #[error("codec backend error: {message}")]
    Backend { message: String },
}

/// Anything a use case can fail with.
#[derive(Debug, thiserror::Error)]
pub enum UseCaseError {
    #[error(transparent)]
    Source(#[from] SourceError),
    #[error(transparent)]
    Sink(#[from] SinkError),
    #[error(transparent)]
    Codec(#[from] CodecError),
}
