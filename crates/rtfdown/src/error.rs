//! What can be wrong with a file, in the three kinds every parser of the workspace has.

/// Why an RTF file could not be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RtfError {
  /// The bytes are not RTF.
  #[error("unsupported format: {0}")]
  UnsupportedFormat(String),
  /// The file asks for more than is allowed for one file.
  #[error("limit exceeded: {0}")]
  LimitExceeded(String),
  /// The file is RTF and is damaged.
  #[error("malformed document: {0}")]
  Malformed(String),
}
