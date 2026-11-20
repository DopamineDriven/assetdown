//! What can be wrong with a file, in the three kinds every parser of the workspace has.

/// Why a JPEG could not be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum JpegError {
  /// The bytes are not a JPEG, or are one kept in a way that is not read.
  #[error("unsupported format: {0}")]
  UnsupportedFormat(String),
  /// The file asks for more than is allowed for one file.
  #[error("limit exceeded: {0}")]
  LimitExceeded(String),
  /// The file is a JPEG and is damaged.
  #[error("malformed file: {0}")]
  Malformed(String),
}
