//! Slides as pages of positioned words, and invoice extraction over those pages.
//!
//! A slide records where every shape is, so a page here starts from real boxes: the shape's own,
//! or the one its layout or master gives it. What a slide does not record is where the lines
//! break inside a box. Those are set by the estimate the other format crates use: a character
//! is half its font size wide, a line 1.2 times its size tall, and a word that does not fit
//! starts a new line.
//!
//! The size of the page, which box a word is in, the cell of a table it is in, and its font
//! size and weight are the file's. Where a line wraps, and so how far down a box its words
//! stand, are estimated. Text that the estimate makes taller than its box is set closer
//! together so that it stays inside the slide.

use contentdown::flow::{Fonts, LINE_EM, Set, Tokens, base_size, line_height, set_lines};
use contentdown::invoice::{InvoiceOptions, InvoiceResult, extract_from_pages};
use contentdown::{PageLayout, PlacedImage, TextSource, Word, words_to_lines};

use crate::model::{
  Align, Anchor, Frame, Insets, Paragraph, Presentation, Shape, Slide, Table, TextShape,
};

/// The size of text whose size nothing states: PowerPoint's own default.
const DEFAULT_SIZE: f64 = 18.0;
/// How much further in each level of a list starts.
const LEVEL_INDENT: f64 = 18.0;
/// The space between a table cell's edges and its text.
const CELL_INSETS: Insets = Insets {
  left: 7.2,
  top: 3.6,
  right: 7.2,
  bottom: 3.6,
};
/// How far from the slide's edges a shape with no box of its own is set.
const UNPLACED_MARGIN: f64 = 36.0;

const ESTIMATED: &str = "the boxes are the file's own; where lines wrap inside a box is estimated";

/// A word on a slide, measured down from the top edge.
struct Placed {
  text: String,
  x0: f64,
  x1: f64,
  top: f64,
  bottom: f64,
  size: f64,
  font: usize,
}

/// A line of set words with how tall it is.
struct Line {
  words: Vec<Set>,
  height: f64,
}

/// How text sits in a box.
struct Setting {
  insets: Insets,
  anchor: Anchor,
  wrap: bool,
  /// The fraction of its stated size text is set at.
  scale: f64,
}

struct Flow<'a> {
  deck: &'a Presentation,
  fonts: Fonts,
  /// The words of the slide being set.
  words: Vec<Placed>,
  images: Vec<PlacedImage>,
  /// Shapes that state no box and were set in a default one.
  unplaced: u32,
}

impl Flow<'_> {
  /// The part of a box that lies on the slide. A box that lies off it is moved to the nearest
  /// edge, so that what it holds is still on the page.
  fn on_slide(&self, frame: Frame) -> (f64, f64, f64, f64) {
    let (width, height) = (self.deck.width, self.deck.height);
    let left = frame.x.clamp(0.0, (width - 1.0).max(0.0));
    let top = frame.y.clamp(0.0, (height - 1.0).max(0.0));
    let right = (frame.x + frame.width).clamp(left + 1.0, width.max(left + 1.0));
    let bottom = (frame.y + frame.height).clamp(top + 1.0, height.max(top + 1.0));
    (left, top, right, bottom)
  }

  /// The box a shape with none is set in: the slide, less a margin.
  fn fallback(&mut self) -> Frame {
    self.unplaced += 1;
    Frame {
      x: UNPLACED_MARGIN,
      y: UNPLACED_MARGIN,
      width: (self.deck.width - 2.0 * UNPLACED_MARGIN).max(1.0),
      height: (self.deck.height - 2.0 * UNPLACED_MARGIN).max(1.0),
      rotation: 0.0,
    }
  }

  /// One paragraph as lines between `left` and `right`.
  fn lines(
    &mut self,
    paragraph: &Paragraph,
    left: f64,
    right: f64,
    setting: &Setting,
  ) -> Vec<Line> {
    let mut tokens = Tokens::default();
    let size_of = |size: Option<f64>| (size.unwrap_or(DEFAULT_SIZE) * setting.scale).max(1.0);
    // A bullet is set like the text it stands in front of.
    if let Some(bullet) = paragraph.bullet.as_deref().filter(|b| !b.is_empty()) {
      let first = paragraph.runs.first();
      let font = self.fonts.intern(
        first.and_then(|run| run.font.as_deref()),
        first.is_some_and(|run| run.bold),
        first.is_some_and(|run| run.italic),
      );
      tokens.word(bullet, size_of(first.and_then(|run| run.size)), font);
    }
    for run in &paragraph.runs {
      let font = self.fonts.intern(run.font.as_deref(), run.bold, run.italic);
      tokens.text(&run.text, size_of(run.size), font);
    }
    let tokens = tokens.finish();
    let base = base_size(&tokens).unwrap_or(DEFAULT_SIZE * setting.scale);
    let left = (left + LEVEL_INDENT * f64::from(paragraph.level)).min(right - 1.0);
    set_lines(tokens, left, right, setting.wrap)
      .into_iter()
      .map(|mut words| {
        let end = words.last().map_or(left, |set| set.x1);
        let shift = match paragraph.align {
          Some(Align::Center) => ((right - end) / 2.0).max(0.0),
          Some(Align::Right) => (right - end).max(0.0),
          _ => 0.0,
        };
        for set in &mut words {
          set.x0 += shift;
          set.x1 += shift;
        }
        Line {
          height: line_height(&words, base),
          words,
        }
      })
      .collect()
  }

  /// Sets paragraphs in a box. They start at the top, the middle or the bottom of it as the
  /// box says, and when the estimate makes them taller than the room there is down to the
  /// slide's bottom edge, their lines are set closer together to fit.
  fn text(&mut self, paragraphs: &[Paragraph], frame: Frame, setting: &Setting) {
    let (left, top, right, bottom) = self.on_slide(frame);
    let inner_left = (left + setting.insets.left).min(right - 1.0);
    let inner_right = (right - setting.insets.right).max(inner_left + 1.0);
    let inner_top = (top + setting.insets.top).min(bottom - 1.0);
    let inner_bottom = (bottom - setting.insets.bottom).max(inner_top + 1.0);

    let lines: Vec<Line> = paragraphs
      .iter()
      .flat_map(|paragraph| self.lines(paragraph, inner_left, inner_right, setting))
      .collect();
    let needed: f64 = lines.iter().map(|line| line.height).sum();
    if needed <= 0.0 {
      return;
    }
    let room = inner_bottom - inner_top;
    let start = match setting.anchor {
      Anchor::Top => inner_top,
      Anchor::Middle => inner_top + ((room - needed) / 2.0).max(0.0),
      Anchor::Bottom => inner_top + (room - needed).max(0.0),
    };
    // Text may run past its box, but not past the slide.
    let limit = (self.deck.height - start).max(1.0);
    let squeeze = if needed > limit { limit / needed } else { 1.0 };
    let mut cursor = start;
    for line in lines {
      let bottom = cursor + line.height * squeeze;
      for set in line.words {
        // A line that runs on past the slide's right edge is cut off at it.
        let x0 = set.x0.clamp(0.0, (self.deck.width - 0.5).max(0.0));
        let x1 = set.x1.clamp(x0 + 0.5, self.deck.width.max(x0 + 0.5));
        self.words.push(Placed {
          text: set.text,
          x0,
          x1,
          top: bottom - LINE_EM * set.size * squeeze,
          bottom,
          size: set.size,
          font: set.font,
        });
      }
      cursor = bottom;
    }
  }

  fn text_shape(&mut self, shape: &TextShape) {
    let frame = match shape.info.frame {
      Some(frame) => frame,
      None => self.fallback(),
    };
    let setting = Setting {
      insets: shape.insets,
      anchor: shape.anchor,
      wrap: shape.wrap,
      scale: if shape.font_scale > 0.0 {
        shape.font_scale
      } else {
        1.0
      },
    };
    self.text(&shape.paragraphs, frame, &setting);
  }

  /// A table: each cell's text in the cell's own rectangle, which the column widths and row
  /// heights give. A table that states none is divided evenly.
  fn table(&mut self, table: &Table) {
    let frame = match table.info.frame {
      Some(frame) => frame,
      None => self.fallback(),
    };
    let columns = table
      .rows
      .iter()
      .flat_map(|row| &row.cells)
      .map(|cell| usize::try_from(cell.column.saturating_add(cell.col_span.max(1))).unwrap_or(0))
      .max()
      .unwrap_or(0)
      .max(table.columns.len())
      .max(1);
    let edges = |sizes: Vec<f64>, count: usize, whole: f64| -> Vec<f64> {
      let stated = sizes.len() == count && sizes.iter().all(|size| *size > 0.0);
      let sizes = if stated {
        sizes
      } else {
        vec![whole / count as f64; count]
      };
      let mut at = 0.0;
      std::iter::once(0.0)
        .chain(sizes.into_iter().map(|size| {
          at += size;
          at
        }))
        .collect()
    };
    let across = edges(table.columns.clone(), columns, frame.width);
    let down = edges(
      table.rows.iter().map(|row| row.height).collect(),
      table.rows.len().max(1),
      frame.height,
    );
    let setting = Setting {
      insets: CELL_INSETS,
      anchor: Anchor::Top,
      wrap: true,
      scale: 1.0,
    };
    let edge =
      |edges: &[f64], at: usize| edges.get(at).or(edges.last()).copied().unwrap_or_default();
    for (row, listed) in table.rows.iter().enumerate() {
      for cell in &listed.cells {
        let column = usize::try_from(cell.column).unwrap_or(usize::MAX);
        let (x0, y0) = (edge(&across, column), edge(&down, row));
        let x1 = edge(
          &across,
          column.saturating_add(usize::try_from(cell.col_span.max(1)).unwrap_or(1)),
        );
        let y1 = edge(
          &down,
          row.saturating_add(usize::try_from(cell.row_span.max(1)).unwrap_or(1)),
        );
        let cell_frame = Frame {
          x: frame.x + x0,
          y: frame.y + y0,
          width: (x1 - x0).max(1.0),
          height: (y1 - y0).max(1.0),
          rotation: 0.0,
        };
        self.text(&cell.paragraphs, cell_frame, &setting);
      }
    }
  }

  fn shape(&mut self, shape: &Shape) {
    match shape {
      Shape::Text(shape) => self.text_shape(shape),
      Shape::Table(table) => self.table(table),
      Shape::Picture(picture) => {
        if let (Some(frame), Some(image)) = (
          picture.info.frame,
          self.deck.images.get(picture.image as usize),
        ) {
          let (left, top, right, bottom) = self.on_slide(frame);
          self.images.push(PlacedImage {
            xobject_name: image.name.clone(),
            x0: left,
            y0: self.deck.height - bottom,
            x1: right,
            y1: self.deck.height - top,
          });
        }
      }
      Shape::Group(group) => {
        for inner in &group.shapes {
          self.shape(inner);
        }
      }
      Shape::Diagram(diagram) => {
        for inner in &diagram.shapes {
          self.shape(inner);
        }
        // A diagram with no drawing has only the text of its nodes: one to a line in its box.
        if diagram.shapes.is_empty() && !diagram.nodes.is_empty() {
          let frame = match diagram.info.frame {
            Some(frame) => frame,
            None => self.fallback(),
          };
          let paragraphs: Vec<Paragraph> = diagram
            .nodes
            .iter()
            .map(|node| Paragraph {
              runs: vec![crate::model::Run {
                text: node.clone(),
                ..Default::default()
              }],
              ..Default::default()
            })
            .collect();
          let setting = Setting {
            insets: CELL_INSETS,
            anchor: Anchor::Top,
            wrap: true,
            scale: 1.0,
          };
          self.text(&paragraphs, frame, &setting);
        }
      }
      // A chart is data, and a clip has no words.
      Shape::Chart(_) | Shape::Media(_) => {}
    }
  }

  /// Sets one slide and turns what was set into a page.
  fn page(&mut self, slide: &Slide) -> PageLayout {
    self.unplaced = 0;
    for shape in &slide.shapes {
      self.shape(shape);
    }
    let height = self.deck.height;
    let words: Vec<Word> = std::mem::take(&mut self.words)
      .into_iter()
      .map(|placed| {
        let y0 = (height - placed.bottom).clamp(0.0, height);
        Word {
          text: placed.text,
          x0: placed.x0,
          y0,
          x1: placed.x1,
          y1: (height - placed.top).clamp(y0, height),
          baseline: y0,
          font_size: Some(placed.size),
          font: Some(placed.font),
          confidence: None,
        }
      })
      .filter(|word| word.x1 > word.x0 && word.y1 > word.y0)
      .collect();
    let (mut words, lines) = words_to_lines(words, self.fonts.metas());
    for word in &mut words {
      // A run whose font nothing names was set in a font with a blank name.
      word.font_name = word.font_name.take().filter(|name| !name.is_empty());
    }
    let mut warnings = vec![ESTIMATED.to_owned()];
    if self.unplaced > 0 {
      warnings.push(format!(
        "{} shape(s) state no box, and neither does the layout or the master; they are set across the slide",
        self.unplaced
      ));
    }
    PageLayout {
      page: slide.number,
      width: self.deck.width,
      height,
      rotate: 0,
      content_rotation: 0,
      source: TextSource::Native,
      words,
      lines,
      images: std::mem::take(&mut self.images),
      warnings,
    }
  }
}

impl Presentation {
  /// The deck as pages of positioned words, one for each slide, with the pictures' boxes. The
  /// boxes are the file's; see the module for what is estimated.
  pub fn layout(&self) -> Vec<PageLayout> {
    let mut flow = Flow {
      deck: self,
      fonts: Fonts::default(),
      words: Vec::new(),
      images: Vec::new(),
      unplaced: 0,
    };
    self.slides.iter().map(|slide| flow.page(slide)).collect()
  }

  /// Invoice fields read from the slides as [`Presentation::layout`] sets them.
  pub fn invoice(&self, options: Option<&InvoiceOptions>) -> InvoiceResult {
    extract_from_pages(&self.layout(), options)
  }
}

#[cfg(test)]
mod tests;
