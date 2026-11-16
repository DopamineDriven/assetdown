//! What opening a presentation can fail with.

use ooxmldown::PackageError;

/// A file that could not be read as a presentation. The category leads the message, as it does
/// for a workbook and a document: `unsupported format: …`, `limit exceeded: …`,
/// `malformed presentation: …`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PresentationError {
  /// A readable file that is not a presentation: the payload says what it is instead.
  #[error("unsupported format: {0}")]
  UnsupportedFormat(String),
  /// A package or presentation cap was exceeded.
  #[error("limit exceeded: {0}")]
  LimitExceeded(String),
  /// The package or one of its parts is not what a presentation should hold.
  #[error("malformed presentation: {0}")]
  Malformed(String),
}

/// A package error keeps its category and its detail; only the wording of the category changes.
impl From<PackageError> for PresentationError {
  fn from(error: PackageError) -> Self {
    match error {
      PackageError::LimitExceeded(detail) => PresentationError::LimitExceeded(detail),
      PackageError::Malformed(detail) => PresentationError::Malformed(detail),
    }
  }
}
