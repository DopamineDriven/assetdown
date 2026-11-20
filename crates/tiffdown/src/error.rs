//! What opening a file can fail with.

/// A file that could not be read as a TIFF. The category leads the message, as it does for the
/// other parsers: `unsupported format: …`, `limit exceeded: …`, `malformed file: …`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TiffError {
  /// A readable file that is not a TIFF this crate reads: the payload says what it is instead.
  #[error("unsupported format: {0}")]
  UnsupportedFormat(String),
  /// A cap on what is read was exceeded.
  #[error("limit exceeded: {0}")]
  LimitExceeded(String),
  /// The file is a TIFF and is not laid out as one should be.
  #[error("malformed file: {0}")]
  Malformed(String),
}
