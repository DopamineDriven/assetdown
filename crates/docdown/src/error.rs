//! What can be wrong with a file, in the three kinds every parser of the workspace has.

/// Why a Word document could not be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DocError {
  /// The bytes are not a document of Word 97 to 2003: the payload says what they are instead.
  #[error("unsupported format: {0}")]
  UnsupportedFormat(String),
  /// The file asks for more than is allowed for one file.
  #[error("limit exceeded: {0}")]
  LimitExceeded(String),
  /// The file is a Word document and is damaged.
  #[error("malformed document: {0}")]
  Malformed(String),
}
