//! The text, in the order the piece table gives: each piece says where its characters are
//! kept in the main stream, and whether each is one byte in the Western code page or two.

use crate::error::DocError;

/// A document may not hold more characters than this. Word's own limit is a little under it.
pub(crate) const MAX_CHARACTERS: usize = 1 << 25;

/// One character of the text: its code, and where it is kept in the main stream, which is
/// what the properties stated for it are found by.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Unit {
  pub code: u16,
  pub at: u32,
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
  Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

/// The table of pieces inside the structure that holds it: properties that go with pieces
/// come first, each marked 1 with its length, and the table is marked 2.
fn table_of(clx: &[u8]) -> Option<&[u8]> {
  let mut at = 0;
  loop {
    match *clx.get(at)? {
      0x01 => {
        let length = usize::from(u16::from_le_bytes(
          clx.get(at + 1..at + 3)?.try_into().ok()?,
        ));
        at += 3 + length;
      }
      0x02 => {
        let length = usize::try_from(u32_at(clx, at + 1)?).ok()?;
        return clx.get(at + 5..(at + 5).checked_add(length)?);
      }
      _ => return None,
    }
  }
}

/// Every character of the document, the main text first and the other runs of text after
/// it. A piece that lies outside the stream is left out, with a warning.
pub(crate) fn read(
  main: &[u8],
  clx: &[u8],
  warnings: &mut Vec<String>,
) -> Result<Vec<Unit>, DocError> {
  let unreadable =
    || DocError::Malformed("the table of the text's pieces cannot be read".to_string());
  // Where each piece starts among the characters and the last one ends, and then for each
  // piece eight bytes, the middle four of which say where it is kept.
  let table = table_of(clx)
    .filter(|table| table.len() >= 16)
    .ok_or_else(unreadable)?;
  let pieces = (table.len() - 4) / 12;
  let mut units: Vec<Unit> = Vec::new();
  let mut outside = false;
  for piece in 0..pieces {
    let span = (|| {
      let start = usize::try_from(u32_at(table, piece * 4)?).ok()?;
      let end = usize::try_from(u32_at(table, piece * 4 + 4)?).ok()?;
      let kept = u32_at(table, (pieces + 1) * 4 + piece * 8 + 2)?;
      Some((end.checked_sub(start)?, kept))
    })();
    let (count, kept) = span.ok_or_else(unreadable)?;
    if units.len().saturating_add(count) > MAX_CHARACTERS {
      return Err(DocError::LimitExceeded(format!(
        "the document has more than {MAX_CHARACTERS} characters"
      )));
    }
    // One bit says the characters are a byte each, and then the place is twice what it is.
    let narrow = kept & 0x4000_0000 != 0;
    let at = kept & 0x3FFF_FFFF;
    let (at, width) = if narrow { (at / 2, 1) } else { (at, 2) };
    let found = usize::try_from(at).ok().and_then(|start| {
      let end = start.checked_add(count.checked_mul(width)?)?;
      Some((start, main.get(start..end)?))
    });
    let Some((start, bytes)) = found else {
      outside = true;
      continue;
    };
    let place = |nth: usize| u32::try_from(start + nth * width).unwrap_or(u32::MAX);
    if narrow {
      let (text, _) = encoding_rs::WINDOWS_1252.decode_without_bom_handling(bytes);
      // Every byte of this code page is one character of the first plane.
      units.extend(text.encode_utf16().enumerate().map(|(nth, code)| Unit {
        code,
        at: place(nth),
      }));
    } else {
      units.extend(
        bytes
          .as_chunks::<2>()
          .0
          .iter()
          .enumerate()
          .map(|(nth, pair)| Unit {
            code: u16::from_le_bytes(*pair),
            at: place(nth),
          }),
      );
    }
  }
  if outside {
    warnings.push("a piece of the text lies outside the file and is left out".to_string());
  }
  Ok(units)
}
