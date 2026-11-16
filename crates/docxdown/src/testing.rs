//! Documents made from fragments of XML, for the tests of the reader and of what is built on it.

use std::collections::HashMap;

use ooxmldown::{DocProps, Relationship};

use crate::body::{Media, Reader, Shared, View};
use crate::error::DocumentError;
use crate::model::{Block, Comment, Document, Field, Image, Note, NoteKind, Part, Section};
use crate::numbering::Numbering;
use crate::styles::Styles;
use crate::xml::child;

const NAMESPACES: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape" xmlns:w14="http://schemas.microsoft.com/office/word/2010/wordml" xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math" xmlns:v="urn:schemas-microsoft-com:vml""#;

/// What a document is made from besides its body.
#[derive(Default)]
pub(crate) struct Setup<'a> {
  /// What stands inside `w:styles`.
  pub styles: Option<&'a str>,
  /// What stands inside `w:numbering`.
  pub numbering: Option<&'a str>,
  /// Relationships of the body: id, target, and whether the target is outside the package.
  pub rels: &'a [(&'a str, &'a str, bool)],
  pub view: View,
  /// Headers and footers: the path a relationship names each by, and what stands inside it.
  pub headers: &'a [(&'a str, &'a str)],
  pub footers: &'a [(&'a str, &'a str)],
  /// Footnotes 1, 2, 3, in order: what stands inside each.
  pub footnotes: &'a [&'a str],
  /// Comments 1, 2, 3, in order: the author of each, and what stands inside it.
  pub comments: &'a [(&'a str, &'a str)],
}

/// What reading a body gives, with what the reader learned on the way.
pub(crate) struct Read {
  pub blocks: Vec<Block>,
  pub sections: Vec<Section>,
  pub pages: u32,
  pub fields: Vec<Field>,
  pub tracked: bool,
  pub anchors: HashMap<i64, String>,
  pub pictures: Vec<(String, bool)>,
}

fn part(root: &str, inner: &str) -> String {
  format!("<w:{root} {NAMESPACES}>{inner}</w:{root}>")
}

fn styles_of(setup: &Setup<'_>) -> Styles {
  setup.styles.map_or_else(Styles::default, |xml| {
    let text = part("styles", xml);
    Styles::parse(&roxmltree::Document::parse(&text).unwrap())
  })
}

fn numbering_of(setup: &Setup<'_>) -> Numbering {
  setup.numbering.map_or_else(Numbering::default, |xml| {
    let text = part("numbering", xml);
    Numbering::parse(&roxmltree::Document::parse(&text).unwrap())
  })
}

fn rels_of(setup: &Setup<'_>) -> HashMap<String, Relationship> {
  setup
    .rels
    .iter()
    .map(|(id, target, external)| {
      (
        (*id).to_owned(),
        Relationship {
          id: (*id).to_owned(),
          kind: String::new(),
          target: (*target).to_owned(),
          external: *external,
        },
      )
    })
    .collect()
}

/// Ids 1 to `count`, each at the place before its own number.
fn places(count: usize) -> impl Iterator<Item = (i64, u32)> {
  (0..count as u32).map(|at| (i64::from(at) + 1, at))
}

/// Reads a body on its own. Notes and comments 1 to 3 are taken to exist.
pub(crate) fn read_with(body: &str, setup: Setup<'_>) -> Result<Read, DocumentError> {
  let (styles, numbering, rels) = (styles_of(&setup), numbering_of(&setup), rels_of(&setup));
  let notes: HashMap<(NoteKind, i64), u32> = places(3)
    .map(|(id, at)| ((NoteKind::Footnote, id), at))
    .collect();
  let comments: HashMap<i64, u32> = places(3).collect();
  let shared = Shared {
    styles: &styles,
    numbering: &numbering,
    notes: &notes,
    comments: &comments,
    view: setup.view,
  };
  let text = part("document", &format!("<w:body>{body}</w:body>"));
  let doc = roxmltree::Document::parse(&text).unwrap();
  let body = child(doc.root_element(), "body").unwrap();
  let mut media = Media::default();
  let mut reader = Reader::for_body(&shared, &rels, &mut media, body);
  let mut blocks = Vec::new();
  reader.blocks(body, &mut blocks, 0)?;
  let Reader {
    sections,
    page,
    fields,
    tracked,
    comment_anchors,
    ..
  } = reader;
  Ok(Read {
    blocks,
    sections,
    pages: page,
    fields,
    tracked,
    anchors: comment_anchors,
    pictures: media.targets,
  })
}

pub(crate) fn read(body: &str) -> Read {
  read_with(body, Setup::default()).unwrap()
}

/// A whole document from a body and what `setup` gives beside it, put together the way
/// [`Document::open`] puts one together from a package.
pub(crate) fn document(body: &str, setup: Setup<'_>) -> Document {
  let (styles, numbering, rels) = (styles_of(&setup), numbering_of(&setup), rels_of(&setup));
  let note_places: HashMap<(NoteKind, i64), u32> = places(setup.footnotes.len())
    .map(|(id, at)| ((NoteKind::Footnote, id), at))
    .collect();
  let comment_places: HashMap<i64, u32> = places(setup.comments.len()).collect();
  let shared = Shared {
    styles: &styles,
    numbering: &numbering,
    notes: &note_places,
    comments: &comment_places,
    view: setup.view,
  };
  let mut media = Media::default();
  let no_rels = HashMap::new();
  let mut tracked = false;
  let mut read_part = |root: &str, inner: &str, media: &mut Media| -> Vec<Block> {
    let text = part(root, inner);
    let doc = roxmltree::Document::parse(&text).unwrap();
    let mut reader = Reader::new(&shared, &no_rels, media);
    let mut blocks = Vec::new();
    reader.blocks(doc.root_element(), &mut blocks, 0).unwrap();
    tracked |= reader.tracked;
    blocks
  };
  let mut parts = |root: &str, named: &[(&str, &str)], media: &mut Media| -> Vec<Part> {
    named
      .iter()
      .map(|(name, inner)| Part {
        name: (*name).to_owned(),
        blocks: read_part(root, inner, media),
      })
      .collect()
  };
  let headers = parts("hdr", setup.headers, &mut media);
  let footers = parts("ftr", setup.footers, &mut media);
  drop(parts);
  let mut notes: Vec<Note> = places(setup.footnotes.len())
    .zip(setup.footnotes)
    .map(|((id, _), inner)| Note {
      kind: NoteKind::Footnote,
      id,
      blocks: read_part("footnote", inner, &mut media),
      page: 0,
    })
    .collect();
  let mut comments: Vec<Comment> = places(setup.comments.len())
    .zip(setup.comments)
    .map(|((id, _), (author, inner))| Comment {
      id,
      author: Some((*author).to_owned()),
      initials: None,
      date: Some("2026-11-15T09:30:00Z".to_owned()),
      blocks: read_part("comment", inner, &mut media),
      anchor_text: String::new(),
      page: 0,
    })
    .collect();
  drop(read_part);

  let text = part("document", &format!("<w:body>{body}</w:body>"));
  let doc = roxmltree::Document::parse(&text).unwrap();
  let body = child(doc.root_element(), "body").unwrap();
  let mut reader = Reader::for_body(&shared, &rels, &mut media, body);
  let mut blocks = Vec::new();
  reader.blocks(body, &mut blocks, 0).unwrap();
  tracked |= reader.tracked;
  let Reader {
    mut sections,
    page,
    fields,
    mut comment_anchors,
    comment_pages,
    note_pages,
    section_parts,
    ..
  } = reader;
  crate::place_parts(&mut sections, &section_parts, &headers, &footers, &blocks);
  for (at, note) in notes.iter_mut().enumerate() {
    note.page = note_pages.get(&(at as u32)).copied().unwrap_or(0);
  }
  for comment in &mut comments {
    comment.anchor_text = comment_anchors.remove(&comment.id).unwrap_or_default();
    comment.page = comment_pages.get(&comment.id).copied().unwrap_or(0);
  }
  let images = media
    .targets
    .into_iter()
    .map(|(name, linked)| Image {
      name,
      mime: None,
      width: None,
      height: None,
      data: Vec::new(),
      linked,
    })
    .collect();
  Document {
    props: DocProps::default(),
    language: styles.language.clone(),
    has_macros: false,
    is_template: false,
    has_tracked_changes: tracked,
    sections,
    blocks,
    headers,
    footers,
    notes,
    comments,
    images,
    fields,
    page_count: page,
    warnings: Vec::new(),
  }
}

/// A paragraph holding one run of `text`.
pub(crate) fn p(text: &str) -> String {
  format!("<w:p><w:r><w:t>{text}</w:t></w:r></w:p>")
}
