//! What reading a package can fail with.

/// A package that could not be read. The payload is the detail on its own, without the category
/// in front, so a format crate can carry it into its own error unchanged.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PackageError {
  /// A cap in [`Limits`](crate::Limits), or the XML node cap, was exceeded.
  #[error("limit exceeded: {0}")]
  LimitExceeded(String),
  /// The container or one of its parts is not what the packaging conventions say it should be.
  #[error("malformed package: {0}")]
  Malformed(String),
}
