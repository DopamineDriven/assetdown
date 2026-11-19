//! PDF. The first line of the file gives its version, and a file laid out for reading over a
//! network states its page count in its first object. Everything else takes the whole file
//! and a PDF reader, which is `pdfdown` when the `pdf` feature is on.

use super::{DocumentDetail, DocumentFacts};
use crate::image::Reading;
use crate::source::Source;

/// What only a PDF says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfDetail {
  /// The version of the format the file is written in: `1.4`, `1.7`, `2.0`.
  pub version: Option<String>,
  /// The file is laid out so that its first page can be shown before the rest arrives.
  pub is_linearized: Option<bool>,
  pub is_encrypted: Option<bool>,
  /// At least one page has text of its own, which a scan has not.
  pub is_searchable: Option<bool>,
  /// The application the document was written in, and the one that made the PDF of it.
  pub creator: Option<String>,
  pub producer: Option<String>,
  /// The document has a form to fill in.
  pub has_form: Option<bool>,
}

/// How much of the start of a file is looked at for what its first object says.
const HEAD_BYTES: usize = 1024;

/// How many characters of text a preview holds.
#[cfg(feature = "pdf")]
const PREVIEW_CHARACTERS: usize = 500;

/// The number written after `key` in `text`: `/N 7`.
fn number_after(text: &str, key: &str) -> Option<u32> {
  let rest = text.split_once(key)?.1.trim_start();
  let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
  digits.parse().ok()
}

pub(crate) fn read(source: &Source<'_>, reading: &mut Reading) -> Option<DocumentFacts> {
  let head = String::from_utf8_lossy(source.head(HEAD_BYTES)).into_owned();
  let version: String = head
    .strip_prefix("%PDF-")?
    .chars()
    .take_while(|c| c.is_ascii_digit() || *c == '.')
    .collect();
  // A file laid out for the network opens with a dictionary that says so and counts its pages.
  let linearized = head.contains("/Linearized");
  let mut facts = DocumentFacts::unread(DocumentDetail::Pdf(PdfDetail {
    version: (!version.is_empty()).then_some(version),
    is_linearized: linearized.then_some(true),
    is_encrypted: None,
    is_searchable: None,
    creator: None,
    producer: None,
    has_form: None,
  }));
  if linearized {
    facts.page_count = number_after(&head, "/N");
  }
  full(source, &mut facts, reading);
  Some(facts)
}

/// Without a PDF reader, the first line and the first object are all that is read.
#[cfg(not(feature = "pdf"))]
fn full(_source: &Source<'_>, _facts: &mut DocumentFacts, _reading: &mut Reading) {}

/// The rest of the facts, from the whole file.
#[cfg(feature = "pdf")]
fn full(source: &Source<'_>, facts: &mut DocumentFacts, reading: &mut Reading) {
  let Some(bytes) = source.whole_file() else {
    reading
      .needs
      .get_or_insert(source.rest_from(source.head_len()));
    return;
  };
  let pdf = match pdfdown::Pdf::open(bytes.to_vec()) {
    Ok(pdf) => pdf,
    Err(error) => {
      reading.warn(format!("the file could not be read as a PDF: {error}"));
      return;
    }
  };
  let meta = pdf.metadata();
  // A file that asks for a password opens with everything in it still encrypted: it would
  // read as having no pages, no words and no pictures.
  if pdf.document().is_encrypted() {
    reading.warn("the file is encrypted and asks for a password, so what it holds is not read");
    if let DocumentDetail::Pdf(detail) = &mut facts.detail {
      detail.is_encrypted = Some(true);
    }
    return;
  }
  facts.page_count = Some(meta.page_count);
  facts.title = meta.title;
  facts.author = meta.author;
  facts.subject = meta.subject;
  // One string in the file, which is a list.
  facts.keywords = meta
    .keywords
    .iter()
    .flat_map(|written| written.split([',', ';']))
    .map(str::trim)
    .filter(|keyword| !keyword.is_empty())
    .map(str::to_string)
    .collect();
  facts.language = meta.language;
  facts.created_date = meta.creation_date;
  facts.modified_date = meta.modification_date;
  facts.image_count = Some(pdf.image_count());
  // The size most pages share is the one listed without the pages it applies to.
  let usual = meta
    .page_boxes
    .iter()
    .find(|page_box| page_box.pages.is_none())
    .or(meta.page_boxes.first());
  if let Some(page_box) = usual {
    facts.page_width = Some(page_box.width);
    facts.page_height = Some(page_box.height);
    facts.paper_size = Some(format!("{:?}", page_box.paper_size).to_ascii_uppercase());
  }
  facts.page_size_uniform = Some(meta.page_boxes.len() <= 1);
  facts.annot_count = u32::try_from(pdf.annotations().len()).ok();

  let pages = pdf.text();
  let words: usize = pages
    .iter()
    .map(|page| page.text.split_whitespace().count())
    .sum();
  facts.word_count = u64::try_from(words).ok();
  let text: String = pages
    .iter()
    .flat_map(|page| page.text.split_whitespace())
    .flat_map(|word| word.chars().chain(std::iter::once(' ')))
    .take(PREVIEW_CHARACTERS)
    .collect();
  let text = text.trim_end();
  facts.text_preview = (!text.is_empty()).then(|| text.to_string());

  let DocumentDetail::Pdf(detail) = &mut facts.detail else {
    return;
  };
  detail.version = Some(meta.version);
  detail.is_linearized = Some(meta.is_linearized);
  detail.is_encrypted = Some(meta.is_encrypted);
  detail.is_searchable = Some(words > 0);
  detail.creator = meta.creator;
  detail.producer = meta.producer;
  detail.has_form = Some(meta.form.is_some());
}

#[cfg(test)]
mod tests;
