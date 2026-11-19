//! What a document says of itself: how many pages it has, how many words, how large a page
//! is, who wrote it and when. The fields every kind of document shares are in
//! [`DocumentFacts`], under the names of the columns they fill; what only one kind has is in
//! [`DocumentDetail`].

pub(crate) mod office;
pub(crate) mod pdf;
pub(crate) mod rtf;
pub(crate) mod text;

pub use office::OfficeDetail;
pub use pdf::PdfDetail;
pub use rtf::RtfDetail;
pub use text::TextDetail;

/// What only one kind of document says.
#[derive(Debug, Clone, PartialEq)]
pub enum DocumentDetail {
  Pdf(PdfDetail),
  Office(OfficeDetail),
  Text(TextDetail),
  Rtf(RtfDetail),
}

/// What a document says of itself. A fact that was not read is `None`.
#[derive(Debug, Clone, PartialEq)]
pub struct DocumentFacts {
  pub page_count: Option<u32>,
  pub word_count: Option<u64>,
  pub language: Option<String>,
  pub title: Option<String>,
  pub author: Option<String>,
  pub subject: Option<String>,
  pub keywords: Vec<String>,
  /// The start of the document's text.
  pub text_preview: Option<String>,
  /// The size of the page most pages share, in points.
  pub page_width: Option<f64>,
  pub page_height: Option<f64>,
  /// The standard paper size that page is nearest to: `A4`, `LETTER`, `UNKNOWN`.
  pub paper_size: Option<String>,
  /// Whether every page is the same size.
  pub page_size_uniform: Option<bool>,
  pub image_count: Option<u32>,
  pub annot_count: Option<u32>,
  /// When the document was made and last changed, as the file writes them.
  pub created_date: Option<String>,
  pub modified_date: Option<String>,
  pub detail: DocumentDetail,
}

impl DocumentFacts {
  /// The facts of a document of which nothing has been read but what `detail` holds.
  pub(crate) fn unread(detail: DocumentDetail) -> Self {
    DocumentFacts {
      page_count: None,
      word_count: None,
      language: None,
      title: None,
      author: None,
      subject: None,
      keywords: Vec::new(),
      text_preview: None,
      page_width: None,
      page_height: None,
      paper_size: None,
      page_size_uniform: None,
      image_count: None,
      annot_count: None,
      created_date: None,
      modified_date: None,
      detail,
    }
  }
}
