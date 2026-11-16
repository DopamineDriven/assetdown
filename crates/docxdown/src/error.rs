//! What opening a document can fail with.

use ooxmldown::PackageError;

/// A file that could not be read as a Word document. The category leads the message, as it does
/// for a workbook: `unsupported format: …`, `limit exceeded: …`, `malformed document: …`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DocumentError {
  /// A readable file that is not a Word document: the payload says what it is instead.
  #[error("unsupported format: {0}")]
  UnsupportedFormat(String),
  /// A package or document cap was exceeded.
  #[error("limit exceeded: {0}")]
  LimitExceeded(String),
  /// The package or one of its parts is not what a document should hold.
  #[error("malformed document: {0}")]
  Malformed(String),
}

/// A package error keeps its category and its detail; only the wording of the category changes.
impl From<PackageError> for DocumentError {
  fn from(error: PackageError) -> Self {
    match error {
      PackageError::LimitExceeded(detail) => DocumentError::LimitExceeded(detail),
      PackageError::Malformed(detail) => DocumentError::Malformed(detail),
    }
  }
}
