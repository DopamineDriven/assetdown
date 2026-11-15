/// Why a PDF could not be opened. Once a document is open, every reader is best-effort and
/// reports trouble as warnings or empty results, not as errors.
#[derive(Debug, thiserror::Error)]
pub enum PdfError {
  #[error("Failed to load PDF: {0}")]
  Load(#[from] lopdf::Error),
}
