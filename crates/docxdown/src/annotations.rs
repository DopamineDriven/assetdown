//! Hyperlinks and comments as one flat list, the way a PDF lists its annotations.
//!
//! A link made in Word is a `w:hyperlink` element or a `HYPERLINK` field; the reader has already
//! turned both into [`Link`]s. A comment is found where the text refers to it.

use crate::model::{Block, Comment, Document, Inline, NoteKind};
use crate::text::blocks_text;
use crate::walk::{each_inline, each_paragraph};

/// Where in a document something sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartKind {
  Body,
  Header,
  Footer,
  Footnote,
  Endnote,
  /// Inside the text of a comment.
  Comment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnotationKind {
  Link,
  Comment,
}

/// A hyperlink or a comment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Annotation {
  /// From 1, by the reader's page count. Absent in a header or a footer, which belong to no one
  /// page, and for a note or a comment that nothing refers to.
  pub page: Option<u32>,
  pub part: PartKind,
  pub subtype: AnnotationKind,
  /// Where an external link leads.
  pub uri: Option<String>,
  /// The bookmark an internal link jumps to.
  pub dest: Option<String>,
  /// What a comment says.
  pub content: Option<String>,
  /// The text the link covers or the comment is attached to.
  pub text: String,
  /// Who wrote a comment, and when, as the document writes the date.
  pub author: Option<String>,
  pub date: Option<String>,
}

fn comment_annotation(comment: &Comment, page: Option<u32>, part: PartKind) -> Annotation {
  Annotation {
    page,
    part,
    subtype: AnnotationKind::Comment,
    uri: None,
    dest: None,
    content: Some(blocks_text(&comment.blocks)),
    text: comment.anchor_text.clone(),
    author: comment.author.clone(),
    date: comment.date.clone(),
  }
}

/// A page number the reader counted; 0 stands for none.
pub(crate) fn counted(page: u32) -> Option<u32> {
  (page > 0).then_some(page)
}

struct Collector<'a> {
  document: &'a Document,
  /// Whether each comment has been listed, by its place in the document's comments.
  listed: Vec<bool>,
  out: Vec<Annotation>,
}

impl Collector<'_> {
  /// The links and comment references under `blocks`. Outside the body everything is on `page`;
  /// in the body each says where it is.
  fn collect(&mut self, blocks: &[Block], part: PartKind, page: Option<u32>) {
    let in_body = part == PartKind::Body;
    each_paragraph(blocks, &mut |paragraph| {
      each_inline(&paragraph.inlines, &mut |inline| match inline {
        Inline::Link(link) => {
          let text = link.text();
          self.out.push(Annotation {
            page: if in_body { Some(link.page) } else { page },
            part,
            subtype: AnnotationKind::Link,
            uri: link.uri.clone(),
            dest: link.dest.clone(),
            content: None,
            text,
            author: None,
            date: None,
          });
        }
        Inline::CommentRef(at) => {
          let at = *at as usize;
          if let Some(comment) = self.document.comments.get(at)
            && !std::mem::replace(&mut self.listed[at], true)
          {
            let page = if in_body {
              counted(comment.page).or(Some(paragraph.page))
            } else {
              page
            };
            self.out.push(comment_annotation(comment, page, part));
          }
        }
        _ => {}
      });
    });
  }
}

impl Document {
  /// Every hyperlink and comment, page by page and in reading order within a page: the body,
  /// then the notes referred to on that page. Those of headers and footers come last.
  pub fn annotations(&self) -> Vec<Annotation> {
    let mut collector = Collector {
      document: self,
      listed: vec![false; self.comments.len()],
      out: Vec::new(),
    };
    collector.collect(&self.blocks, PartKind::Body, None);
    for note in &self.notes {
      let part = match note.kind {
        NoteKind::Footnote => PartKind::Footnote,
        NoteKind::Endnote => PartKind::Endnote,
      };
      collector.collect(&note.blocks, part, counted(note.page));
    }
    for comment in &self.comments {
      collector.collect(&comment.blocks, PartKind::Comment, counted(comment.page));
    }
    for header in &self.headers {
      collector.collect(&header.blocks, PartKind::Header, None);
    }
    for footer in &self.footers {
      collector.collect(&footer.blocks, PartKind::Footer, None);
    }
    // A comment that nothing in the text refers to is still a comment.
    let Collector {
      listed, mut out, ..
    } = collector;
    for (comment, listed) in self.comments.iter().zip(listed) {
      if !listed {
        out.push(comment_annotation(comment, None, PartKind::Body));
      }
    }
    out.sort_by_key(|annotation| annotation.page.unwrap_or(u32::MAX));
    out
  }
}

#[cfg(test)]
mod tests;
