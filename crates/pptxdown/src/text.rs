//! A presentation as plain text.

use crate::model::{Paragraph, Presentation, Shape, Slide, Table, TableCell, TableRow};

/// The text of one slide with what repeats on every slide set apart, as pdfdown sets a page's
/// header and footer apart from its body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuredText {
  /// The slide number, from 1.
  pub page: u32,
  pub header: String,
  pub body: String,
  pub footer: String,
}

/// Paragraphs as lines, with none left empty at either end.
pub(crate) fn lines_of(paragraphs: &[Paragraph]) -> String {
  paragraphs
    .iter()
    .map(Paragraph::line)
    .collect::<Vec<_>>()
    .join("\n")
    .trim_matches('\n')
    .to_owned()
}

/// The texts of shapes, one under the other, leaving out the shapes that have none.
fn stacked<'a>(shapes: impl Iterator<Item = &'a Shape>) -> String {
  shapes
    .map(Shape::text)
    .filter(|text| !text.is_empty())
    .collect::<Vec<_>>()
    .join("\n")
}

impl Paragraph {
  /// The paragraph's own text: line breaks kept, nothing added in front.
  pub fn text(&self) -> String {
    self.runs.iter().map(|run| run.text.as_str()).collect()
  }

  /// The paragraph as a line of the slide: its bullet in front of it, as it stands there.
  pub fn line(&self) -> String {
    match &self.bullet {
      Some(bullet) if !bullet.is_empty() => format!("{bullet} {}", self.text()),
      _ => self.text(),
    }
  }
}

impl TableCell {
  /// The cell's text on one line: its paragraphs joined by spaces, so that a row stays a row.
  pub fn text(&self) -> String {
    self
      .paragraphs
      .iter()
      .map(Paragraph::line)
      .filter(|line| !line.is_empty())
      .collect::<Vec<_>>()
      .join(" ")
      .replace(['\n', '\t'], " ")
  }

  /// The cell's text as it is laid out: one line per paragraph.
  pub fn lines(&self) -> String {
    lines_of(&self.paragraphs)
  }
}

impl TableRow {
  /// The row on one line, cells separated by tabs.
  pub fn text(&self) -> String {
    self
      .cells
      .iter()
      .map(TableCell::text)
      .collect::<Vec<_>>()
      .join("\t")
  }
}

impl Table {
  /// One line per row, cells separated by tabs.
  pub fn text(&self) -> String {
    self
      .rows
      .iter()
      .map(TableRow::text)
      .collect::<Vec<_>>()
      .join("\n")
  }
}

impl Shape {
  /// The text the shape holds: a text box's paragraphs, a table's rows, what a group or a
  /// diagram holds. A picture, a clip and a chart hold none; a chart's words are its data.
  pub fn text(&self) -> String {
    match self {
      Shape::Text(shape) => lines_of(&shape.paragraphs),
      Shape::Table(table) => table.text(),
      Shape::Group(group) => stacked(group.shapes.iter()),
      Shape::Diagram(diagram) if diagram.shapes.is_empty() => diagram.nodes.join("\n"),
      Shape::Diagram(diagram) => stacked(diagram.shapes.iter()),
      Shape::Picture(_) | Shape::Chart(_) | Shape::Media(_) => String::new(),
    }
  }
}

impl Slide {
  /// The slide's text in reading order, without the date, footer and slide number that repeat
  /// on every slide.
  pub fn text(&self) -> String {
    stacked(self.shapes.iter().filter(|shape| !shape.is_furniture()))
  }

  /// The speaker notes, one line per paragraph.
  pub fn notes_text(&self) -> String {
    lines_of(&self.notes)
  }
}

impl Presentation {
  /// The deck as plain text: each slide's text, with an empty line between slides.
  pub fn text(&self) -> String {
    self
      .slides
      .iter()
      .map(Slide::text)
      .collect::<Vec<_>>()
      .join("\n\n")
  }

  /// The text of each slide, with its number.
  pub fn text_per_page(&self) -> Vec<(u32, String)> {
    self
      .slides
      .iter()
      .map(|slide| (slide.number, slide.text()))
      .collect()
  }

  /// The text of each slide with its header and footer apart from its body. A date, a footer
  /// or a slide number placeholder is header when it sits in the top half of the slide and
  /// footer otherwise. A deck of one slide has nothing that repeats, so all of it is body.
  pub fn structured_text(&self) -> Vec<StructuredText> {
    let repeats = self.slides.len() >= 2;
    let middle = self.height / 2.0;
    self
      .slides
      .iter()
      .map(|slide| {
        if !repeats {
          return StructuredText {
            page: slide.number,
            header: String::new(),
            body: stacked(slide.shapes.iter()),
            footer: String::new(),
          };
        }
        let at_top = |shape: &&Shape| {
          shape
            .info()
            .frame
            .is_some_and(|frame| frame.y + frame.height / 2.0 < middle)
        };
        let furniture = || slide.shapes.iter().filter(|shape| shape.is_furniture());
        StructuredText {
          page: slide.number,
          header: stacked(furniture().filter(at_top)),
          body: slide.text(),
          footer: stacked(furniture().filter(|shape| !at_top(shape))),
        }
      })
      .collect()
  }
}
