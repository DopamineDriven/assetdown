//! The text as paragraphs, the rows of tables, and page breaks. A paragraph ends at its mark,
//! a cell at a mark of its own, and a row at one more of those whose properties say it ends
//! a row. A field keeps what it shows and drops what it is told.

use contentdown::document::{Align, Block, Inline, Paragraph, Placing, Row, Run};

use crate::boxes::Boxes;
use crate::pieces::Unit;
use crate::props::{Stated, Styles, properties};

/// The marks in the text: the end of a paragraph, the end of a cell or of a row, a page
/// break or the end of a section, and the place a shape is anchored at.
const PARAGRAPH: u16 = 0x0D;
const CELL: u16 = 0x07;
const PAGE: u16 = 0x0C;
const ANCHOR: u16 = 0x08;
/// The size of text that states none, in points.
const DEFAULT_SIZE: f64 = 10.0;

/// What a story is read with: the properties stated for any place in the main stream, the
/// styles, and the names of the fonts.
pub(crate) struct Source<'a> {
  pub paragraphs: &'a Stated<'a>,
  pub characters: &'a Stated<'a>,
  pub styles: &'a Styles<'a>,
  pub fonts: &'a [String],
}

/// How a run of text is set.
#[derive(Clone, PartialEq)]
struct Format {
  size: f64,
  bold: bool,
  italic: bool,
  font: usize,
  hidden: bool,
}

/// What a paragraph's properties state. Lengths are in points.
#[derive(Default)]
struct Stating {
  align: Align,
  indent_left: f64,
  indent_right: f64,
  space_before: f64,
  space_after: f64,
  in_table: bool,
  row_end: bool,
  /// A row's left side, and the right side of each of its cells.
  edges: Option<(f64, Vec<f64>)>,
  break_before: bool,
}

fn points(twips: i16) -> f64 {
  f64::from(twips) / 20.0
}

/// A property that is on or off: off, on, as the style has it, or the other way from it.
fn switched(value: u8, styled: bool) -> bool {
  match value {
    0 => false,
    0x80 => styled,
    0x81 => !styled,
    _ => true,
  }
}

fn set_characters(format: &mut Format, stated: &[u8]) {
  let styled = format.clone();
  for property in properties(stated) {
    match (property.code, property.u8(), property.u16()) {
      (0x0835, Some(value), _) => format.bold = switched(value, styled.bold),
      (0x0836, Some(value), _) => format.italic = switched(value, styled.italic),
      (0x083C, Some(value), _) => format.hidden = switched(value, styled.hidden),
      // The size, in half points.
      (0x4A43, _, Some(halves)) => format.size = f64::from(halves) / 2.0,
      (0x4A4F, _, Some(font)) => format.font = usize::from(font),
      _ => {}
    }
  }
}

fn set_paragraph(stating: &mut Stating, stated: &[u8]) {
  for property in properties(stated) {
    let length = property.i16().map(points);
    match (property.code, property.u8(), length) {
      // How the paragraph is lined up, as Word 97 states it and as later ones do.
      (0x2403 | 0x2461, Some(value), _) => {
        stating.align = match value {
          0 => Align::Left,
          1 => Align::Center,
          2 => Align::Right,
          _ => Align::Justified,
        };
      }
      (0x840F | 0x845E, _, Some(length)) => stating.indent_left = length,
      (0x840E | 0x845D, _, Some(length)) => stating.indent_right = length,
      (0xA413, _, Some(length)) => stating.space_before = length,
      (0xA414, _, Some(length)) => stating.space_after = length,
      (0x2416, Some(value), _) => stating.in_table = value != 0,
      (0x2417, Some(value), _) => stating.row_end = value != 0,
      (0x2407, Some(value), _) => stating.break_before = value != 0,
      // A row's edges: how many cells, the left side, and the right side of each cell.
      (0xD608, Some(cells), _) => {
        let edges: Vec<f64> = property
          .value
          .get(1..)
          .unwrap_or(&[])
          .as_chunks::<2>()
          .0
          .iter()
          .take(usize::from(cells) + 1)
          .map(|pair| points(i16::from_le_bytes(*pair)))
          .collect();
        if let Some((left, rest)) = edges.split_first() {
          stating.edges = Some((*left, rest.to_vec()));
        }
      }
      _ => {}
    }
  }
}

/// The inlines of a paragraph as they are gathered: text set one way is one run.
struct Gathered<'a> {
  fonts: &'a [String],
  inlines: Vec<Inline>,
  pending: Vec<u16>,
  format: Option<Format>,
}

impl Gathered<'_> {
  fn flush(&mut self) {
    if let Some(format) = self.format.take()
      && !self.pending.is_empty()
    {
      self.inlines.push(Inline::Text(Run {
        text: String::from_utf16_lossy(&self.pending),
        size: format.size,
        bold: format.bold,
        italic: format.italic,
        font: self.fonts.get(format.font).cloned(),
      }));
    }
    self.pending.clear();
  }

  fn character(&mut self, code: u16, format: &Format) {
    if self.format.as_ref() != Some(format) {
      self.flush();
      self.format = Some(format.clone());
    }
    self.pending.push(code);
  }

  fn inline(&mut self, inline: Inline) {
    self.flush();
    self.inlines.push(inline);
  }
}

/// A table whose rows are being gathered.
#[derive(Default)]
struct Table {
  cell: Vec<Paragraph>,
  cells: Vec<Vec<Paragraph>>,
}

impl Table {
  /// The row gathered so far, when there is one.
  fn row(&mut self, edges: Option<(f64, Vec<f64>)>) -> Option<Block> {
    if !self.cell.is_empty() {
      self.cells.push(std::mem::take(&mut self.cell));
    }
    if self.cells.is_empty() {
      return None;
    }
    let (left, edges) = edges.unwrap_or_default();
    Some(Block::Row(Row {
      left,
      edges,
      cells: std::mem::take(&mut self.cells),
    }))
  }
}

/// The blocks of a text box, each put where the box is: its paragraphs one under another in
/// it, and a row of a table as one paragraph with a tab between its cells.
fn boxed(blocks: Vec<Block>, placing: Placing) -> Vec<Block> {
  let paragraphs = blocks.into_iter().filter_map(|block| match block {
    Block::Paragraph(paragraph) => Some(paragraph),
    Block::Row(row) => {
      let mut cells = row.cells.into_iter().map(|cell| {
        let mut inlines = Vec::new();
        for paragraph in cell {
          if !inlines.is_empty() {
            inlines.push(Inline::LineBreak);
          }
          inlines.extend(paragraph.inlines);
        }
        inlines
      });
      let mut inlines = cells.next().unwrap_or_default();
      for cell in cells {
        inlines.push(Inline::Tab);
        inlines.extend(cell);
      }
      Some(Paragraph {
        inlines,
        ..Paragraph::default()
      })
    }
    Block::PageBreak => None,
  });
  paragraphs
    .enumerate()
    .map(|(nth, paragraph)| {
      Block::Paragraph(Paragraph {
        placing: Some(placing),
        continues_frame: nth > 0,
        ..paragraph
      })
    })
    .collect()
}

/// The blocks of one story: the document's own text, or a header, or a footer, or a text
/// box. With `boxes`, the story is the document's own text: a text box is put in where its
/// shape is anchored, before the paragraph the anchor is in, and one that no shape leads to
/// is read after the text, so that its words are not lost.
pub(crate) fn blocks(units: &[Unit], source: &Source<'_>, boxes: Option<&Boxes<'_>>) -> Vec<Block> {
  let mut blocks = Vec::new();
  let mut placed = vec![false; boxes.map_or(0, Boxes::count)];
  let mut table = Table::default();
  // The fields that are open, and for each whether what it shows has been reached.
  let mut fields: Vec<bool> = Vec::new();
  let mut start = 0;
  while start < units.len() {
    let end = units[start..]
      .iter()
      .position(|unit| matches!(unit.code, PARAGRAPH | CELL | PAGE))
      .map_or(units.len(), |offset| start + offset);
    let mark = units.get(end);
    // A paragraph's properties are stated for where its mark is kept.
    let stated = mark
      .or(units[start..end].last())
      .map_or(&[][..], |unit| source.paragraphs.at(unit.at));
    let style = stated.get(..2).map_or(0, |style| {
      usize::from(u16::from_le_bytes([style[0], style[1]]))
    });
    let chain = source.styles.chain(style);
    let mut stating = Stating::default();
    let mut base = Format {
      size: DEFAULT_SIZE,
      bold: false,
      italic: false,
      font: source.styles.default_font,
      hidden: false,
    };
    for (paragraph, characters) in &chain {
      set_paragraph(&mut stating, paragraph);
      set_characters(&mut base, characters);
    }
    set_paragraph(&mut stating, stated.get(2..).unwrap_or(&[]));

    let mut gathered = Gathered {
      fonts: source.fonts,
      inlines: Vec::new(),
      pending: Vec::new(),
      format: None,
    };
    // Characters kept side by side mostly state the same properties: they are read once.
    let mut last: Option<(&[u8], Format)> = None;
    for (at, unit) in units.iter().enumerate().take(end).skip(start) {
      match unit.code {
        0x13 => fields.push(false),
        0x14 => {
          if let Some(shown) = fields.last_mut() {
            *shown = true;
          }
        }
        0x15 => {
          fields.pop();
        }
        _ if !fields.iter().all(|shown| *shown) => {}
        ANCHOR => {
          let anchored = boxes.and_then(|boxes| Some((boxes, boxes.anchored_at(at)?)));
          if let Some((boxes, (text_box, placing))) = anchored
            && let Some(first) = placed.get_mut(text_box).filter(|placed| !**placed)
          {
            *first = true;
            let held = self::blocks(boxes.text(text_box), source, None);
            blocks.extend(boxed(held, placing));
          }
        }
        code => {
          let own = source.characters.at(unit.at);
          let format = match &last {
            Some((seen, format)) if std::ptr::eq(*seen, own) => format,
            _ => {
              let mut format = base.clone();
              set_characters(&mut format, own);
              &last.insert((own, format)).1
            }
          };
          match code {
            _ if format.hidden => {}
            0x09 => gathered.inline(Inline::Tab),
            0x0B => gathered.inline(Inline::LineBreak),
            // A hyphen that a line may not break at.
            0x1E => gathered.character(0x2011, format),
            // The rest below a space stand for something that is not text: a picture, a
            // drawing, a note, a hyphen that shows only where a line breaks.
            0x00..=0x1F => {}
            code => gathered.character(code, format),
          }
        }
      }
    }
    gathered.flush();
    let paragraph = Paragraph {
      inlines: gathered.inlines,
      align: stating.align,
      indent_left: stating.indent_left,
      indent_right: stating.indent_right,
      placing: None,
      continues_frame: false,
      space_before: stating.space_before,
      space_after: stating.space_after,
    };
    match mark.map(|unit| unit.code) {
      Some(CELL) if stating.row_end => blocks.extend(table.row(stating.edges)),
      Some(CELL) => {
        table.cell.push(paragraph);
        table.cells.push(std::mem::take(&mut table.cell));
      }
      Some(PARAGRAPH) if stating.in_table => table.cell.push(paragraph),
      ended => {
        // A table whose last row has no end is still a row.
        blocks.extend(table.row(None));
        if stating.break_before && !blocks.is_empty() {
          blocks.push(Block::PageBreak);
        }
        // What stands before a page break in its paragraph, and a last paragraph with no
        // mark, are paragraphs when they hold anything.
        if ended == Some(PARAGRAPH) || !paragraph.inlines.is_empty() {
          blocks.push(Block::Paragraph(paragraph));
        }
        if ended == Some(PAGE) {
          blocks.push(Block::PageBreak);
        }
      }
    }
    start = end + 1;
  }
  blocks.extend(table.row(None));
  if let Some(boxes) = boxes {
    for (text_box, _) in placed.iter().enumerate().filter(|(_, placed)| !**placed) {
      blocks.extend(self::blocks(boxes.text(text_box), source, None));
    }
  }
  blocks
}
