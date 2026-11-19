//! Text as a document: how it is encoded, how many lines and words it has, how it begins.
//! The counts are over the bytes given.

use super::{DocumentDetail, DocumentFacts};
use crate::image::Reading;
use crate::source::Source;
use crate::text;

/// What only a text says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextDetail {
  /// How its characters are written as bytes: `utf-8`, `utf-16le`, `utf-16be`, or
  /// `unknown-8bit` for one byte to a character in an encoding the bytes do not name.
  pub encoding: String,
  pub line_count: u64,
}

/// `leniently` is how the text was named: read with the allowance a second look gives.
pub(crate) fn read(
  source: &Source<'_>,
  leniently: bool,
  reading: &mut Reading,
) -> Option<DocumentFacts> {
  let whole = source.whole_file();
  let bytes = whole.unwrap_or_else(|| source.head(usize::MAX));
  let decode = if leniently {
    text::decode_leniently
  } else {
    text::decode
  };
  let (encoding, text) = decode(bytes, whole.is_none())?;
  let counts = text::counts(&text);
  if whole.is_none() {
    reading
      .needs
      .get_or_insert(source.rest_from(source.head_len()));
  }
  let detail = DocumentDetail::Text(TextDetail {
    encoding: encoding.as_str().to_string(),
    line_count: counts.lines,
  });
  Some(DocumentFacts {
    word_count: Some(counts.words),
    text_preview: counts.preview,
    ..DocumentFacts::unread(detail)
  })
}
