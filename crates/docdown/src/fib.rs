//! The block a document opens with: which Word wrote it, which of the two table streams it
//! uses, how long each run of its text is, and where each structure of the table stream is.

use crate::error::DocError;

/// The mark a document of Word 97 or later opens with, and the one of Word 6 and 95.
const WORD_97: u16 = 0xA5EC;
const WORD_6: u16 = 0xA5DC;
/// The lowest version number Word 97 states.
const FIRST_VERSION: u16 = 0x00C1;
/// Where the places of the table stream's structures are listed, after a count of them.
const PLACES_AT: usize = 0x9A;

/// Which structure of the table stream, by its place in the list.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Part {
  StyleSheet = 1,
  Sections = 6,
  Running = 11,
  CharacterProperties = 12,
  ParagraphProperties = 13,
  Fonts = 15,
  Pieces = 33,
  /// Where the shapes of the main text are anchored.
  Shapes = 40,
  Drawings = 50,
  /// Where each text box's text starts.
  Boxes = 56,
}

pub(crate) struct Fib {
  /// The name of the table stream this document uses.
  pub table: &'static str,
  pub is_template: bool,
  /// How many characters the document's own text is, and its footnotes, and its headers and
  /// footers, which follow one another in that order.
  pub text: usize,
  pub footnotes: usize,
  pub running: usize,
  /// How many characters come between the headers and footers and the text boxes: the text
  /// of macros, of comments, and of endnotes.
  pub before_boxes: usize,
  pub boxes: usize,
  /// Where each structure of the table stream starts, and how long it is.
  places: Vec<(usize, usize)>,
}

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
  Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
  Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

impl Fib {
  pub fn read(main: &[u8]) -> Result<Fib, DocError> {
    let unreadable =
      || DocError::Malformed("the document's opening block cannot be read".to_string());
    if main.len() < PLACES_AT {
      return Err(unreadable());
    }
    let mark = u16_at(main, 0).ok_or_else(unreadable)?;
    let version = u16_at(main, 2).ok_or_else(unreadable)?;
    let flags = u16_at(main, 0x0A).ok_or_else(unreadable)?;
    match mark {
      WORD_97 if version >= FIRST_VERSION => {}
      // The older layout keeps its text and its properties another way.
      WORD_97 | WORD_6 => {
        return Err(DocError::UnsupportedFormat(
          "a Word 6 or 95 document".to_string(),
        ));
      }
      _ => {
        return Err(DocError::Malformed(
          "the document does not open as a Word document does".to_string(),
        ));
      }
    }
    if flags & 0x0100 != 0 {
      return Err(DocError::UnsupportedFormat(
        "an encrypted document".to_string(),
      ));
    }
    let count = |at: usize| {
      u32_at(main, at)
        .and_then(|count| usize::try_from(count).ok())
        .ok_or_else(unreadable)
    };
    let listed = usize::from(u16_at(main, PLACES_AT - 2).ok_or_else(unreadable)?);
    let places = (0..listed)
      .map_while(|index| {
        let at = PLACES_AT + index * 8;
        Some((
          usize::try_from(u32_at(main, at)?).ok()?,
          usize::try_from(u32_at(main, at + 4)?).ok()?,
        ))
      })
      .collect();
    Ok(Fib {
      table: if flags & 0x0200 != 0 {
        "1Table"
      } else {
        "0Table"
      },
      is_template: flags & 0x0001 != 0,
      text: count(0x4C)?,
      footnotes: count(0x50)?,
      running: count(0x54)?,
      before_boxes: count(0x58)?
        .saturating_add(count(0x5C)?)
        .saturating_add(count(0x60)?),
      boxes: count(0x64)?,
      places,
    })
  }

  /// A structure of the table stream. None when the document has none, or it lies outside
  /// the stream.
  pub fn part<'a>(&self, part: Part, table: &'a [u8]) -> Option<&'a [u8]> {
    let (at, length) = *self.places.get(part as usize)?;
    if length == 0 {
      return None;
    }
    table.get(at..at.checked_add(length)?)
  }
}
