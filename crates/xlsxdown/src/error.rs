//! What reading a workbook can fail with.

use ooxmldown::PackageError;

/// A buffer that could not be read as a workbook. The payload is the detail; `Display` puts the
/// category in front of it.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SpreadsheetError {
  /// A readable package that is not a plain workbook: the payload says what it is instead.
  #[error("unsupported format: {0}")]
  UnsupportedFormat(String),
  /// A package or workbook cap was exceeded.
  #[error("limit exceeded: {0}")]
  LimitExceeded(String),
  /// The package or one of its parts is not what a workbook should hold.
  #[error("malformed workbook: {0}")]
  Malformed(String),
}

/// A package error keeps its category and its detail; only the wording of the category changes.
impl From<PackageError> for SpreadsheetError {
  fn from(error: PackageError) -> Self {
    match error {
      PackageError::LimitExceeded(detail) => SpreadsheetError::LimitExceeded(detail),
      PackageError::Malformed(detail) => SpreadsheetError::Malformed(detail),
    }
  }
}
