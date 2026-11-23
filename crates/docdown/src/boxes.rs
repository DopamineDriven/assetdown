//! Text boxes. Their text is kept apart from the document's own, one run for all of them,
//! and each box belongs to a shape that is anchored at a mark in the document's text. The
//! anchor says where the shape is, and the drawing data says which box is the shape's.

use std::collections::HashMap;

use contentdown::document::{DownFrom, Placing};

use crate::pieces::Unit;

/// How far a box's text stands in from its sides, and from its top, in points: what a box
/// has when it states nothing else.
const INSET_ACROSS: f64 = 7.2;
const INSET_DOWN: f64 = 3.6;
/// The property of a shape that names its text box, and the kinds of record a drawing is
/// made of: the three that hold others, a shape, a shape's own number, and its properties.
const TEXT_ID: u16 = 0x0080;
const HOLDERS: [u16; 3] = [0xF000, 0xF002, 0xF003];
const SHAPE: u16 = 0xF004;
const SHAPE_NUMBER: u16 = 0xF00A;
const SHAPE_PROPERTIES: u16 = 0xF00B;

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
  Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
  Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

fn i32_at(bytes: &[u8], at: usize) -> Option<i32> {
  Some(i32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

/// One record of a drawing: its version and instance, its kind, and its data.
struct Drawn<'a> {
  instance: u16,
  kind: u16,
  data: &'a [u8],
}

/// The record `bytes` opens with, and what follows it.
fn drawn(bytes: &[u8]) -> Option<(Drawn<'_>, &[u8])> {
  let length = usize::try_from(u32_at(bytes, 4)?).ok()?;
  let data = bytes.get(8..8usize.checked_add(length)?)?;
  let record = Drawn {
    instance: u16_at(bytes, 0)? >> 4,
    kind: u16_at(bytes, 2)?,
    data,
  };
  Some((record, &bytes[8 + length..]))
}

/// A shape's number and the number of its text box, from the records it holds: its number in
/// one, and in another its properties, each a number and a value.
fn shape(mut held: &[u8]) -> Option<(u32, usize)> {
  let (mut number, mut text_box) = (None, None);
  while let Some((record, rest)) = drawn(held) {
    match record.kind {
      SHAPE_NUMBER => number = u32_at(record.data, 0),
      SHAPE_PROPERTIES => {
        text_box = record
          .data
          .as_chunks::<6>()
          .0
          .iter()
          .take(usize::from(record.instance))
          .find(|property| u16::from_le_bytes([property[0], property[1]]) & 0x3FFF == TEXT_ID)
          // The box's number, from 1, is the upper half of the value.
          .and_then(|property| {
            usize::from(u16::from_le_bytes([property[4], property[5]])).checked_sub(1)
          });
      }
      _ => {}
    }
    held = rest;
  }
  number.zip(text_box)
}

/// Which text box each shape's text is in, by the shape's number. The drawing data is what
/// the drawings share, and then each drawing with one byte before it.
fn text_boxes(drawings: &[u8]) -> HashMap<u32, usize> {
  let mut found = HashMap::new();
  let Some((_, mut rest)) = drawn(drawings) else {
    return found;
  };
  // What is still to be looked through. A record that holds others is opened in its turn,
  // so that shapes inside groups inside groups are found without going deeper each time.
  let mut open: Vec<&[u8]> = Vec::new();
  while let Some((drawing, after)) = rest.get(1..).and_then(drawn) {
    open.push(drawing.data);
    rest = after;
  }
  while let Some(mut held) = open.pop() {
    while let Some((record, after)) = drawn(held) {
      if record.kind == SHAPE {
        found.extend(shape(record.data));
      } else if HOLDERS.contains(&record.kind) {
        open.push(record.data);
      }
      held = after;
    }
  }
  found
}

/// The text boxes of the main text.
pub(crate) struct Boxes<'a> {
  /// The text of all of them, one after another.
  story: &'a [Unit],
  /// Where each box's text starts in it, and the last one's ends.
  starts: Vec<usize>,
  /// For a mark in the main text that a shape with text is anchored at: which box, and where
  /// its text goes.
  anchored: HashMap<usize, (usize, Placing)>,
}

impl<'a> Boxes<'a> {
  /// From the text of the boxes and three structures of the table stream: where each box
  /// starts, where each shape is anchored, and the drawing data.
  pub fn read(
    story: &'a [Unit],
    starts: Option<&[u8]>,
    anchors: Option<&[u8]>,
    drawings: Option<&[u8]>,
  ) -> Self {
    // Places, one more than there are stretches, and then 22 bytes about each stretch.
    let starts = starts.unwrap_or(&[]);
    let count = starts.len().saturating_sub(4) / 26;
    let starts: Vec<usize> = (0..=count)
      .filter_map(|nth| usize::try_from(u32_at(starts, nth * 4)?).ok())
      .collect();
    let text_boxes = text_boxes(drawings.unwrap_or(&[]));
    // Places, one more than there are shapes, and then 26 bytes about each shape: its
    // number, its left, top, right, and bottom, and what they are measured from.
    let anchors = anchors.unwrap_or(&[]);
    let shapes = anchors.len().saturating_sub(4) / 30;
    let anchored = (0..shapes)
      .filter_map(|nth| {
        let at = usize::try_from(u32_at(anchors, nth * 4)?).ok()?;
        let stated = anchors.get((shapes + 1) * 4 + nth * 26..)?;
        let text_box = *text_boxes.get(&u32_at(stated, 0)?)?;
        let points = |at: usize| i32_at(stated, at).map(|twips| f64::from(twips) / 20.0);
        let (left, top, right) = (points(4)?, points(8)?, points(12)?);
        let flags = u16_at(stated, 20)?;
        let placing = Placing {
          x: left + INSET_ACROSS,
          y: top + INSET_DOWN,
          width: Some((right - left - 2.0 * INSET_ACROSS).max(1.0)),
          // Two bits say what the sides are measured from, and the next two the top: the
          // margin, the page, or the column and the paragraph the shape is anchored in.
          from_page_side: (flags >> 1) & 0x03 == 1,
          down_from: match (flags >> 3) & 0x03 {
            0 => DownFrom::TopMargin,
            1 => DownFrom::PageTop,
            _ => DownFrom::Text,
          },
        };
        Some((at, (text_box, placing)))
      })
      .collect();
    Boxes {
      story,
      starts,
      anchored,
    }
  }

  /// How many boxes there are. The last stretch the table lists is no box: it is the mark
  /// that ends the boxes' text.
  pub fn count(&self) -> usize {
    self.starts.len().saturating_sub(2)
  }

  /// The text of the box numbered `index` from 0.
  pub fn text(&self, index: usize) -> &'a [Unit] {
    let range = self.starts.get(index).zip(self.starts.get(index + 1));
    range
      .and_then(|(start, end)| self.story.get(*start..*end))
      .unwrap_or(&[])
  }

  /// The box whose shape is anchored at the mark at `at` in the main text, and where its
  /// text goes.
  pub fn anchored_at(&self, at: usize) -> Option<(usize, Placing)> {
    self.anchored.get(&at).copied()
  }
}
