//! What can be wrong with what was handed over, before any of it is read.

/// Bytes that say nothing are not an error: they are an answer, `Sniffed::Unknown`. This is for
/// input that contradicts itself.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MimeError {
  /// Pieces of a file that overlap and disagree, or that lie past the size given for the file.
  #[error("invalid ranges: {0}")]
  InvalidRanges(String),
}
