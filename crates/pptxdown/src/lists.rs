//! What a deck holds as flat lists: links and comments, pictures, tables, charts and clips, each
//! with the slide it is on. The same things stay in the slides' shapes; these are for a caller
//! who wants every one of a kind without walking the tree.

use ooxmldown::{Chart, cell_ref};

use crate::model::{
  Frame, LinkTarget, Paragraph, Presentation, Run, Shape, ShapeInfo, Slide, Table,
};
use crate::text::lines_of;

/// Calls `f` with every shape under `shapes`, a group or a diagram before what it holds.
pub(crate) fn each_shape<'a>(shapes: &'a [Shape], f: &mut impl FnMut(&'a Shape)) {
  for shape in shapes {
    f(shape);
    match shape {
      Shape::Group(group) => each_shape(&group.shapes, f),
      Shape::Diagram(diagram) => each_shape(&diagram.shapes, f),
      _ => {}
    }
  }
}

fn count(n: usize) -> u32 {
  u32::try_from(n).unwrap_or(u32::MAX)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnotationKind {
  Link,
  Comment,
}

/// Where on a slide's page something sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartKind {
  Slide,
  Notes,
}

/// A hyperlink or a comment.
#[derive(Debug, Clone, PartialEq)]
pub struct Annotation {
  /// The slide number, from 1.
  pub page: u32,
  pub part: PartKind,
  pub subtype: AnnotationKind,
  /// The box of the shape the link is in. Absent for a comment, and for a link in the notes.
  pub rect: Option<Frame>,
  /// Where an external link leads.
  pub uri: Option<String>,
  /// Where a link inside the deck leads: the number of a slide, or a jump such as `nextslide`.
  pub dest: Option<String>,
  /// What a comment says.
  pub content: Option<String>,
  /// The text the link covers. Empty for a comment, which is attached to a slide.
  pub text: String,
  /// Who wrote a comment, and when, as the file writes the date.
  pub author: Option<String>,
  pub date: Option<String>,
}

fn link_annotation(
  page: u32,
  part: PartKind,
  rect: Option<Frame>,
  target: &LinkTarget,
  text: String,
) -> Annotation {
  let (uri, dest) = match target {
    LinkTarget::External(uri) => (Some(uri.clone()), None),
    LinkTarget::Slide(number) => (None, Some(number.to_string())),
    LinkTarget::Action(action) => (None, Some(action.clone())),
  };
  Annotation {
    page,
    part,
    subtype: AnnotationKind::Link,
    rect,
    uri,
    dest,
    content: None,
    text,
    author: None,
    date: None,
  }
}

/// The links in a stretch of paragraphs. Runs that follow each other to one target are one link.
fn text_links(
  paragraphs: &[Paragraph],
  page: u32,
  part: PartKind,
  rect: Option<Frame>,
  out: &mut Vec<Annotation>,
) {
  for paragraph in paragraphs {
    let mut open: Option<(&LinkTarget, String)> = None;
    let close = |open: &mut Option<(&LinkTarget, String)>, out: &mut Vec<Annotation>| {
      if let Some((target, text)) = open.take() {
        out.push(link_annotation(page, part, rect, target, text));
      }
    };
    for Run { text, link, .. } in &paragraph.runs {
      match (&mut open, link) {
        (Some((target, covered)), Some(link)) if *target == link => covered.push_str(text),
        (_, Some(link)) => {
          close(&mut open, out);
          open = Some((link, text.clone()));
        }
        (_, None) => close(&mut open, out),
      }
    }
    close(&mut open, out);
  }
}

/// One use of a picture.
#[derive(Debug, Clone, PartialEq)]
pub struct Placement {
  /// The slide number, from 1.
  pub page: u32,
  /// Its place among the pictures of the same slide, from 0.
  pub image_index: u32,
  /// Index into [`Presentation::images`].
  pub image: u32,
  /// The SVG the picture is drawn from, when the file keeps one: index into the same list.
  pub vector: Option<u32>,
  /// Where the picture is on the slide.
  pub frame: Option<Frame>,
  pub alt_text: Option<String>,
  /// The picture is the slide's background.
  pub background: bool,
}

/// One cell of a table, at the top-left corner of the range it covers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GridCell {
  /// From 0.
  pub row: u32,
  pub column: u32,
  pub col_span: u32,
  pub row_span: u32,
  /// The cell's text, one line per paragraph.
  pub text: String,
}

/// A table with its cells addressed, the way a sheet's cells are.
#[derive(Debug, Clone, PartialEq)]
pub struct TableGrid {
  /// The table's place among the deck's tables, in reading order, from 0.
  pub index: u32,
  /// The slide number, from 1.
  pub page: u32,
  pub frame: Option<Frame>,
  /// Column widths in points.
  pub columns: Vec<f64>,
  pub row_count: u32,
  pub column_count: u32,
  /// The ranges merged cells cover, in A1 form: `A1:B2`.
  pub merges: Vec<String>,
  /// Row by row, left to right. A cell merged into another is not listed.
  pub cells: Vec<GridCell>,
}

fn grid(table: &Table, index: u32, page: u32) -> TableGrid {
  let mut cells = Vec::new();
  let mut merges = Vec::new();
  let mut column_count = count(table.columns.len());
  for (row, cells_of_row) in table.rows.iter().enumerate() {
    let row = count(row);
    for cell in &cells_of_row.cells {
      let last_row = row.saturating_add(cell.row_span.max(1) - 1);
      let last_column = cell.column.saturating_add(cell.col_span.max(1) - 1);
      column_count = column_count.max(last_column.saturating_add(1));
      if (last_row, last_column) != (row, cell.column) {
        merges.push(format!(
          "{}:{}",
          cell_ref(row, cell.column),
          cell_ref(last_row, last_column)
        ));
      }
      cells.push(GridCell {
        row,
        column: cell.column,
        col_span: cell.col_span,
        row_span: cell.row_span,
        text: cell.lines(),
      });
    }
  }
  TableGrid {
    index,
    page,
    frame: table.info.frame,
    columns: table.columns.clone(),
    row_count: count(table.rows.len()),
    column_count,
    merges,
    cells,
  }
}

/// A chart on a slide.
#[derive(Debug, Clone, PartialEq)]
pub struct ChartUse<'a> {
  /// The slide number, from 1.
  pub page: u32,
  pub info: &'a ShapeInfo,
  pub chart: &'a Chart,
}

/// An audio or a video clip on a slide.
#[derive(Debug, Clone, PartialEq)]
pub struct ClipUse {
  /// The slide number, from 1.
  pub page: u32,
  /// Index into [`Presentation::media`].
  pub media: u32,
  /// The picture shown for the clip: index into [`Presentation::images`].
  pub poster: Option<u32>,
  pub frame: Option<Frame>,
}

impl Slide {
  fn annotations(&self, out: &mut Vec<Annotation>) {
    each_shape(&self.shapes, &mut |shape| {
      let info = shape.info();
      if let Some(target) = &info.link {
        let text = shape.text().replace('\n', " ");
        out.push(link_annotation(
          self.number,
          PartKind::Slide,
          info.frame,
          target,
          text,
        ));
      }
      match shape {
        Shape::Text(shape) => text_links(
          &shape.paragraphs,
          self.number,
          PartKind::Slide,
          info.frame,
          out,
        ),
        Shape::Table(table) => {
          for cell in table.rows.iter().flat_map(|row| &row.cells) {
            text_links(
              &cell.paragraphs,
              self.number,
              PartKind::Slide,
              info.frame,
              out,
            );
          }
        }
        _ => {}
      }
    });
    text_links(&self.notes, self.number, PartKind::Notes, None, out);
    for comment in &self.comments {
      out.push(Annotation {
        page: self.number,
        part: PartKind::Slide,
        subtype: AnnotationKind::Comment,
        rect: None,
        uri: None,
        dest: None,
        content: Some(comment.text.clone()),
        text: String::new(),
        author: comment.author.clone(),
        date: comment.date.clone(),
      });
    }
  }
}

impl Presentation {
  /// Every hyperlink and comment, slide by slide: a slide's links in reading order, then the
  /// links in its notes, then its comments.
  pub fn annotations(&self) -> Vec<Annotation> {
    let mut out = Vec::new();
    for slide in &self.slides {
      slide.annotations(&mut out);
    }
    out
  }

  /// Every place a picture is used, slide by slide and in reading order within a slide.
  pub fn placements(&self) -> Vec<Placement> {
    let mut out = Vec::new();
    for slide in &self.slides {
      let mut next = 0;
      each_shape(&slide.shapes, &mut |shape| {
        if let Shape::Picture(picture) = shape {
          out.push(Placement {
            page: slide.number,
            image_index: next,
            image: picture.image,
            vector: picture.vector,
            frame: picture.info.frame,
            alt_text: picture.info.alt_text.clone(),
            background: picture.background,
          });
          next += 1;
        }
      });
    }
    out
  }

  /// The tables of the deck, in the order they are read.
  pub fn tables(&self) -> Vec<TableGrid> {
    let mut out = Vec::new();
    for slide in &self.slides {
      each_shape(&slide.shapes, &mut |shape| {
        if let Shape::Table(table) = shape {
          out.push(grid(table, count(out.len()), slide.number));
        }
      });
    }
    out
  }

  /// The charts of the deck, as data, with the slide each is on.
  pub fn charts(&self) -> Vec<ChartUse<'_>> {
    let mut out = Vec::new();
    for slide in &self.slides {
      each_shape(&slide.shapes, &mut |shape| {
        if let Shape::Chart(chart) = shape {
          out.push(ChartUse {
            page: slide.number,
            info: &chart.info,
            chart: &chart.chart,
          });
        }
      });
    }
    out
  }

  /// Every place an audio or a video clip is used.
  pub fn clips(&self) -> Vec<ClipUse> {
    let mut out = Vec::new();
    for slide in &self.slides {
      each_shape(&slide.shapes, &mut |shape| {
        if let Shape::Media(clip) = shape {
          out.push(ClipUse {
            page: slide.number,
            media: clip.media,
            poster: clip.poster,
            frame: clip.info.frame,
          });
        }
      });
    }
    out
  }

  /// The speaker notes of each slide that has any, with the slide's number.
  pub fn notes(&self) -> Vec<(u32, String)> {
    self
      .slides
      .iter()
      .filter(|slide| !slide.notes.is_empty())
      .map(|slide| (slide.number, lines_of(&slide.notes)))
      .collect()
  }
}
