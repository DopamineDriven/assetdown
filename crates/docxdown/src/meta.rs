//! What a document is, in numbers: its sections and their paper, and counts of what it holds.

use contentdown::{PaperSize, paper_size};

use crate::annotations::AnnotationKind;
use crate::model::{Block, Document, Margins, NoteKind};
use crate::walk::each_paragraph;

/// A section's page setup, with the standard paper it is nearest to. Lengths are in points.
#[derive(Debug, Clone, PartialEq)]
pub struct SectionInfo {
  /// Absent when the section states no page size.
  pub width: Option<f64>,
  pub height: Option<f64>,
  pub landscape: bool,
  pub margins: Margins,
  /// Text columns.
  pub columns: u32,
  /// The standard size nearest to the page, in either orientation; unknown when the section
  /// states no size or none is near.
  pub paper_size: PaperSize,
  /// The page the section starts on, from 1.
  pub first_page: u32,
}

/// Counts and page facts. What the document says about itself (title, author, dates) is in
/// [`Document::props`].
#[derive(Debug, Clone, PartialEq)]
pub struct Metadata {
  /// The page count the application recorded when it saved the file; the reader's own count
  /// when it recorded none.
  pub page_count: u32,
  pub pages_recorded: Option<u32>,
  /// The pages the reader counted from page breaks, which is what `page` on a block counts in.
  pub pages_counted: u32,
  /// Words of the body, counted from its text.
  pub word_count: u32,
  pub words_recorded: Option<u32>,
  /// Characters of the body that are not white space.
  pub character_count: u32,
  pub characters_recorded: Option<u32>,
  /// Lines as the application counted them when it saved the file. The reader has no count of
  /// its own to set beside it: a docx records no lines.
  pub lines_recorded: Option<u32>,
  /// Paragraphs that hold text, tables' included.
  pub paragraph_count: u32,
  /// Paragraphs as the application counted them, by rules of its own.
  pub paragraphs_recorded: Option<u32>,
  pub heading_count: u32,
  pub list_item_count: u32,
  /// Tables, the ones inside cells included.
  pub table_count: u32,
  /// Places a picture is used: the length of [`Document::placements`].
  pub image_count: u32,
  /// Links and comments: the length of [`Document::annotations`].
  pub annotation_count: u32,
  pub link_count: u32,
  pub comment_count: u32,
  pub footnote_count: u32,
  pub endnote_count: u32,
  /// Content controls, form fields and merge fields.
  pub field_count: u32,
  pub header_count: u32,
  pub footer_count: u32,
  /// Whether every section has the same page size.
  pub page_size_uniform: bool,
  pub sections: Vec<SectionInfo>,
}

fn count(n: usize) -> u32 {
  u32::try_from(n).unwrap_or(u32::MAX)
}

fn count_tables(blocks: &[Block]) -> usize {
  blocks
    .iter()
    .map(|block| match block {
      Block::Paragraph(_) => 0,
      Block::Table(table) => {
        1 + table
          .rows
          .iter()
          .flat_map(|row| &row.cells)
          .map(|cell| count_tables(&cell.blocks))
          .sum::<usize>()
      }
    })
    .sum()
}

impl Document {
  pub fn metadata(&self) -> Metadata {
    // Counted paragraph by paragraph: a list item's marker is not one of its words.
    let (mut words, mut characters) = (0, 0);
    let (mut paragraphs, mut headings, mut list_items) = (0, 0, 0);
    each_paragraph(&self.blocks, &mut |paragraph| {
      if paragraph.inlines.is_empty() {
        return;
      }
      let text = paragraph.text();
      words += text.split_whitespace().count();
      characters += text.chars().filter(|c| !c.is_whitespace()).count();
      paragraphs += 1;
      headings += usize::from(paragraph.heading.is_some());
      list_items += usize::from(paragraph.list.is_some());
    });

    // A section starts on the page of its first block; one with no block of its own, where
    // the section before it left off.
    let mut first_pages = vec![0_u32; self.sections.len()];
    for block in &self.blocks {
      if let Some(first) = first_pages.get_mut(block.section() as usize)
        && *first == 0
      {
        *first = block.page();
      }
    }
    let mut previous = 1;
    let sections: Vec<SectionInfo> = self
      .sections
      .iter()
      .zip(first_pages)
      .map(|(section, first_page)| {
        previous = if first_page == 0 {
          previous
        } else {
          first_page
        };
        SectionInfo {
          width: section.width,
          height: section.height,
          landscape: section.landscape,
          margins: section.margins,
          columns: section.columns,
          paper_size: match (section.width, section.height) {
            (Some(width), Some(height)) => paper_size(width, height),
            _ => PaperSize::Unknown,
          },
          first_page: previous,
        }
      })
      .collect();
    let same_size = |a: Option<f64>, b: Option<f64>| match (a, b) {
      (Some(a), Some(b)) => (a - b).abs() < 0.5,
      (None, None) => true,
      _ => false,
    };
    let page_size_uniform = self.sections.windows(2).all(|pair| {
      same_size(pair[0].width, pair[1].width) && same_size(pair[0].height, pair[1].height)
    });

    let annotations = self.annotations();
    let links = annotations
      .iter()
      .filter(|annotation| annotation.subtype == AnnotationKind::Link)
      .count();
    let notes = |kind: NoteKind| self.notes.iter().filter(|note| note.kind == kind).count();
    Metadata {
      page_count: self.props.pages.unwrap_or(self.page_count),
      pages_recorded: self.props.pages,
      pages_counted: self.page_count,
      word_count: count(words),
      words_recorded: self.props.words,
      character_count: count(characters),
      characters_recorded: self.props.characters,
      lines_recorded: self.props.lines,
      paragraph_count: count(paragraphs),
      paragraphs_recorded: self.props.paragraphs,
      heading_count: count(headings),
      list_item_count: count(list_items),
      table_count: count(count_tables(&self.blocks)),
      image_count: count(self.placements().len()),
      annotation_count: count(annotations.len()),
      link_count: count(links),
      comment_count: count(self.comments.len()),
      footnote_count: count(notes(NoteKind::Footnote)),
      endnote_count: count(notes(NoteKind::Endnote)),
      field_count: count(self.fields.len()),
      header_count: count(self.headers.len()),
      footer_count: count(self.footers.len()),
      page_size_uniform,
      sections,
    }
  }
}
