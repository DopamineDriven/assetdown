//! Invoice extraction for a PDF. The extractor is `contentdown`'s; this module supplies the one
//! thing only a PDF has, the files embedded in it, where an e-invoice's XML travels.

use contentdown::PageLayout;
use contentdown::invoice::{EmbeddedFile, InvoiceOptions, InvoiceResult};
use lopdf::Document;

use crate::attachments::extract_attachments;

/// Embedded XML first when preferred, else the layout track over `pages`.
pub fn extract_invoice(
  doc: &Document,
  options: Option<&InvoiceOptions>,
  pages: &[PageLayout],
) -> InvoiceResult {
  // The attachments are only decoded when the XML track will look at them.
  let prefer_xml = options.and_then(|o| o.prefer_embedded_xml).unwrap_or(true);
  let files: Vec<EmbeddedFile> = if prefer_xml {
    extract_attachments(doc)
      .into_iter()
      .map(|attachment| EmbeddedFile {
        name: attachment.name,
        mime_type: attachment.mime_type,
        relationship: attachment.relationship,
        data: attachment.data,
      })
      .collect()
  } else {
    Vec::new()
  };
  contentdown::invoice::extract_invoice(&files, options, pages)
}
