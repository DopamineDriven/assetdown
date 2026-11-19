//! EBML: a file of nested elements, which opens with a header that names the document type.
//! Matroska and WebM are both this.

use crate::source::{Missing, Source};

/// What an EBML file's header says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Header {
  /// `webm` or `matroska`. None when the header does not say.
  pub doc_type: Option<String>,
}

/// The four bytes every EBML file opens with: the identifier of the header element.
const MAGIC: [u8; 4] = [0x1A, 0x45, 0xDF, 0xA3];

/// The identifier of the element that holds the document type.
const DOC_TYPE: u64 = 0x4282;

/// A header is a few dozen bytes. One that states a length beyond this is read this far.
const LONGEST_HEADER: u64 = 4096;

/// The file's EBML header, when it opens with one.
pub(crate) fn header(source: &Source<'_>) -> Result<Option<Header>, Missing> {
  if source.array::<4>(0)? != MAGIC {
    return Ok(None);
  }
  let unnamed = Header { doc_type: None };
  // The header's length is a number of one to eight bytes; its first byte says how many.
  let length = match stated_length(source) {
    Ok(length) => length,
    Err(Missing::PastEnd) => return Ok(Some(unnamed)),
    Err(missing) => return Err(missing),
  };
  let Some((length, width)) = length else {
    return Ok(Some(unnamed));
  };
  let wanted = usize::try_from(length.min(LONGEST_HEADER)).unwrap_or(0);
  let body = match source.read(4 + width, wanted) {
    Ok(body) => body,
    Err(Missing::PastEnd) => return Ok(Some(unnamed)),
    Err(missing) => return Err(missing),
  };
  Ok(Some(Header {
    doc_type: doc_type(body),
  }))
}

/// The header's length and how many bytes it is written in.
fn stated_length(source: &Source<'_>) -> Result<Option<(u64, u64)>, Missing> {
  let [first] = source.array::<1>(4)?;
  let Some(width) = width_of(first) else {
    return Ok(None);
  };
  let bytes = source.read(4, width)?;
  let width = u64::try_from(width).unwrap_or(8);
  Ok(number(bytes, Marker::Dropped).map(|(length, _)| (length, width)))
}

/// How many bytes a number takes: one more than the zeros its first byte begins with.
fn width_of(first: u8) -> Option<usize> {
  let width = usize::try_from(first.leading_zeros()).ok()? + 1;
  (width <= 8).then_some(width)
}

/// Whether the bit that marks a number's width is part of its value. It is for an element's
/// identifier and is not for its length.
#[derive(Clone, Copy, PartialEq)]
enum Marker {
  Kept,
  Dropped,
}

/// The number `bytes` begins with, and how many bytes it took.
fn number(bytes: &[u8], marker: Marker) -> Option<(u64, usize)> {
  let (first, rest) = bytes.split_first()?;
  let width = width_of(*first)?;
  let rest = rest.get(..width - 1)?;
  let lead = match marker {
    Marker::Kept => *first,
    Marker::Dropped => *first & !(0x80 >> (width - 1)),
  };
  let value = rest.iter().fold(u64::from(lead), |value, byte| {
    (value << 8) | u64::from(*byte)
  });
  Some((value, width))
}

/// The document type among the header's elements: each an identifier, a length, and that many
/// bytes.
fn doc_type(mut body: &[u8]) -> Option<String> {
  loop {
    let (id, id_width) = number(body, Marker::Kept)?;
    let (length, length_width) = number(body.get(id_width..)?, Marker::Dropped)?;
    let start = id_width + length_width;
    let end = start.checked_add(usize::try_from(length).ok()?)?;
    let content = body.get(start..end)?;
    if id == DOC_TYPE {
      let text = String::from_utf8_lossy(content);
      return Some(text.trim_end_matches('\0').to_string());
    }
    body = body.get(end..)?;
  }
}

#[cfg(test)]
mod tests;
