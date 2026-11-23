//! Compound files: the container the Office formats before 2007 are kept in, and the one an
//! object embedded in a document is kept in. A compound file is a small file system of its
//! own, with sectors chained through an allocation table and a directory of storages (its
//! folders) and streams (its files).
//!
//! [`CompoundFile::open`] reads one, and gives a stream by its path or every stream by its
//! name. [`compound`] writes one, which is how a storage is handed back as a file of its own.
//! [`packaged`] and [`HOLDING`] are how an embedded object keeps a file.

mod error;
mod file;
mod object;
mod write;

pub use error::CfbError;
pub use file::{Child, CompoundFile, Stream};
pub use object::{HOLDING, packaged};
pub use write::compound;
