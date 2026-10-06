//! Infrastructure adapters: the local filesystem and the JSON codec.

pub mod codec;
pub mod fs;

pub use codec::JsonCodec;
pub use fs::{FsSink, FsSource};
