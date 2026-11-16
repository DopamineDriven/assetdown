//! WordprocessingML documents: a `.docx` read into blocks.
//!
//! [`Document::open`] takes the bytes of a file. The package layer (`ooxmldown`) opens the zip
//! within limits and finds parts through relationships; `styles` and `numbering` say what a
//! paragraph takes from its style and what stands in front of a list item; `body` walks a part
//! into paragraphs and tables, taking text wherever the document keeps it. Nothing here depends
//! on Node.

mod annotations;
mod body;
mod error;
mod grid;
mod layout;
mod markdown;
mod media;
mod meta;
mod model;
mod numbering;
mod pages;
mod styles;
mod text;
mod walk;
mod xml;

use std::collections::HashMap;

use ooxmldown::{
  Package, PackageError, PackageKind, Relationship, image_info, parse_xml, rels_path,
};
use roxmltree::Node;

use body::{Media, NamedParts, Reader, SectionParts, Shared};
use numbering::Numbering;
use styles::Styles;
use xml::{attr, child, number, wml};

pub use annotations::{Annotation, AnnotationKind, PartKind};
pub use body::View;
pub use error::DocumentError;
pub use grid::{GridCell, TableGrid, cell_ref, column_letters};
pub use media::Placement;
pub use meta::{Metadata, SectionInfo};
pub use model::{
  Align, Block, BreakKind, Cell, Comment, Document, Field, FieldKind, Image, ImageRef, Inline,
  Link, ListItem, Margins, Note, NoteKind, Paragraph, Part, PartRefs, Row, Section, Table, TextRun,
  VerticalAlign,
};
pub use ooxmldown::DocProps;

/// How a document is read.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Options {
  /// Which side of the tracked changes to read. The final one by default.
  pub view: View,
}

/// The first bytes of an OLE compound file.
const OLE_MAGIC: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

/// A part's relationships by id; none when it has no relationships part.
fn relationships_of(
  package: &mut Package,
  part: &str,
) -> Result<HashMap<String, Relationship>, DocumentError> {
  match package.relationships(&rels_path(part)) {
    Ok(relationships) => Ok(
      relationships
        .into_iter()
        .map(|relationship| (relationship.id.clone(), relationship))
        .collect(),
    ),
    Err(PackageError::Malformed(_)) => Ok(HashMap::new()),
    Err(limit) => Err(limit.into()),
  }
}

/// A part the document can do without: `None`, with a warning, when it is named and cannot be
/// read. A limit error is still an error.
fn optional_part(
  package: &mut Package,
  path: Option<&String>,
  warnings: &mut Vec<String>,
) -> Result<Option<Vec<u8>>, DocumentError> {
  let Some(path) = path else {
    return Ok(None);
  };
  match package.part(path) {
    Ok(bytes) => Ok(Some(bytes)),
    Err(PackageError::Malformed(detail)) => {
      warnings.push(format!("{path} was left out: {detail}"));
      Ok(None)
    }
    Err(limit) => Err(limit.into()),
  }
}

/// The parsed XML of an optional part, with a warning when it is not well-formed.
fn optional_xml<'a>(
  bytes: Option<&'a Vec<u8>>,
  path: Option<&String>,
  warnings: &mut Vec<String>,
) -> Option<roxmltree::Document<'a>> {
  match parse_xml(bytes?) {
    Ok(doc) => Some(doc),
    Err(error) => {
      let name = path.map_or("a part", String::as_str);
      warnings.push(format!("{name} was left out: {error}"));
      None
    }
  }
}

/// The notes or comments a part lists, with their ids, skipping the separator notes Word keeps
/// beside the real ones.
fn entries<'a>(doc: Option<&'a roxmltree::Document<'a>>, name: &str) -> Vec<(i64, Node<'a, 'a>)> {
  let Some(doc) = doc else {
    return Vec::new();
  };
  doc
    .root_element()
    .children()
    .filter(|n| wml(*n) == Some(name))
    .filter(|n| {
      !matches!(
        attr(*n, "type"),
        Some("separator" | "continuationSeparator" | "continuationNotice")
      )
    })
    .filter_map(|n| Some((number::<i64>(attr(n, "id"))?, n)))
    .collect()
}

/// Gives every block a section to name, and every section the headers and footers it uses: the
/// ones it names, or else the ones the section before it uses.
fn place_parts(
  sections: &mut Vec<Section>,
  named: &[SectionParts],
  headers: &[Part],
  footers: &[Part],
  blocks: &[Block],
) {
  // Every block names a section, so there is always at least one, stated or not.
  let last_used = blocks.last().map_or(0, Block::section);
  while sections.len() <= last_used as usize {
    sections.push(Section {
      width: None,
      height: None,
      landscape: false,
      margins: Margins::default(),
      columns: 1,
      starts_page: true,
      title_page: false,
      headers: PartRefs::default(),
      footers: PartRefs::default(),
    });
  }
  let resolve = |kept: &mut PartRefs, parts: &[Part], named: &NamedParts| {
    let place_of = |name: &Option<String>| {
      let name = name.as_ref()?;
      let at = parts.iter().position(|part| &part.name == name)?;
      u32::try_from(at).ok()
    };
    kept.default = place_of(&named.default).or(kept.default);
    kept.first = place_of(&named.first).or(kept.first);
    kept.even = place_of(&named.even).or(kept.even);
    *kept
  };
  let unnamed = SectionParts::default();
  let (mut header_refs, mut footer_refs) = (PartRefs::default(), PartRefs::default());
  for (at, section) in sections.iter_mut().enumerate() {
    let named = named.get(at).unwrap_or(&unnamed);
    section.headers = resolve(&mut header_refs, headers, &named.headers);
    section.footers = resolve(&mut footer_refs, footers, &named.footers);
  }
}

impl Document {
  /// Reads a document from the bytes of a `.docx`, `.docm` or `.dotx` file.
  pub fn open(bytes: Vec<u8>) -> Result<Document, DocumentError> {
    Document::open_with(bytes, Options::default())
  }

  pub fn open_with(bytes: Vec<u8>, options: Options) -> Result<Document, DocumentError> {
    if bytes.starts_with(&OLE_MAGIC) {
      return Err(DocumentError::UnsupportedFormat(
        "OLE compound file: a legacy .doc, or a document protected with a password".into(),
      ));
    }
    let mut package = Package::open(bytes)?;
    let kind = match package.kind() {
      Ok(kind) => kind,
      Err(PackageError::Malformed(_)) => PackageKind::Other,
      Err(limit) => return Err(limit.into()),
    };
    let (has_macros, is_template) = match kind {
      PackageKind::WordDocument => (false, false),
      PackageKind::MacroDocument => (true, false),
      PackageKind::TemplateDocument => (false, true),
      other => {
        return Err(DocumentError::UnsupportedFormat(
          other.description().to_string(),
        ));
      }
    };
    let main = package.officedocument()?;
    let main_rels = relationships_of(&mut package, &main)?;
    // The parts the main part points at, by the last word of the relationship type.
    let targets = |suffix: &str| -> Vec<String> {
      let mut found: Vec<&Relationship> = main_rels
        .values()
        .filter(|r| !r.external && r.kind.ends_with(suffix))
        .collect();
      found.sort_by(|a, b| a.target.cmp(&b.target));
      found.into_iter().map(|r| r.target.clone()).collect()
    };
    let mut warnings = Vec::new();

    let styles_path = targets("/styles").into_iter().next();
    let styles_bytes = optional_part(&mut package, styles_path.as_ref(), &mut warnings)?;
    let styles = optional_xml(styles_bytes.as_ref(), styles_path.as_ref(), &mut warnings)
      .map(|doc| Styles::parse(&doc))
      .unwrap_or_default();
    let numbering_path = targets("/numbering").into_iter().next();
    let numbering_bytes = optional_part(&mut package, numbering_path.as_ref(), &mut warnings)?;
    let numbering = optional_xml(
      numbering_bytes.as_ref(),
      numbering_path.as_ref(),
      &mut warnings,
    )
    .map(|doc| Numbering::parse(&doc))
    .unwrap_or_default();

    let footnotes_path = targets("/footnotes").into_iter().next();
    let footnotes_bytes = optional_part(&mut package, footnotes_path.as_ref(), &mut warnings)?;
    let footnotes_xml = optional_xml(
      footnotes_bytes.as_ref(),
      footnotes_path.as_ref(),
      &mut warnings,
    );
    let endnotes_path = targets("/endnotes").into_iter().next();
    let endnotes_bytes = optional_part(&mut package, endnotes_path.as_ref(), &mut warnings)?;
    let endnotes_xml = optional_xml(
      endnotes_bytes.as_ref(),
      endnotes_path.as_ref(),
      &mut warnings,
    );
    let comments_path = targets("/comments").into_iter().next();
    let comments_bytes = optional_part(&mut package, comments_path.as_ref(), &mut warnings)?;
    let comments_xml = optional_xml(
      comments_bytes.as_ref(),
      comments_path.as_ref(),
      &mut warnings,
    );

    // Every note and comment gets its place before anything is walked, so that a reference to
    // one can be resolved wherever it turns up.
    let note_entries: Vec<(NoteKind, &Option<String>, i64, Node<'_, '_>)> =
      entries(footnotes_xml.as_ref(), "footnote")
        .into_iter()
        .map(|(id, node)| (NoteKind::Footnote, &footnotes_path, id, node))
        .chain(
          entries(endnotes_xml.as_ref(), "endnote")
            .into_iter()
            .map(|(id, node)| (NoteKind::Endnote, &endnotes_path, id, node)),
        )
        .collect();
    let comment_entries = entries(comments_xml.as_ref(), "comment");
    let place = |at: usize| u32::try_from(at).unwrap_or(u32::MAX);
    let note_places: HashMap<(NoteKind, i64), u32> = note_entries
      .iter()
      .enumerate()
      .map(|(at, (kind, _, id, _))| ((*kind, *id), place(at)))
      .collect();
    let comment_places: HashMap<i64, u32> = comment_entries
      .iter()
      .enumerate()
      .map(|(at, (id, _))| (*id, place(at)))
      .collect();
    let shared = Shared {
      styles: &styles,
      numbering: &numbering,
      notes: &note_places,
      comments: &comment_places,
      view: options.view,
    };
    let mut media = Media::default();
    let mut tracked = false;
    // The relationships of the parts that hold notes and comments, read once each.
    let mut part_rels: HashMap<String, HashMap<String, Relationship>> = HashMap::new();
    for path in [&footnotes_path, &endnotes_path, &comments_path]
      .into_iter()
      .flatten()
    {
      part_rels.insert(path.clone(), relationships_of(&mut package, path)?);
    }
    let no_rels = HashMap::new();
    let rels_for = |path: &Option<String>| {
      path
        .as_ref()
        .and_then(|path| part_rels.get(path))
        .unwrap_or(&no_rels)
    };

    let mut notes = Vec::with_capacity(note_entries.len());
    for (kind, path, id, node) in &note_entries {
      let mut reader = Reader::new(&shared, rels_for(path), &mut media);
      let mut blocks = Vec::new();
      reader.blocks(*node, &mut blocks, 0)?;
      tracked |= reader.tracked;
      notes.push(Note {
        kind: *kind,
        id: *id,
        blocks,
        page: 0,
      });
    }
    let mut comments = Vec::with_capacity(comment_entries.len());
    for (id, node) in &comment_entries {
      let mut reader = Reader::new(&shared, rels_for(&comments_path), &mut media);
      let mut blocks = Vec::new();
      reader.blocks(*node, &mut blocks, 0)?;
      comments.push(Comment {
        id: *id,
        author: attr(*node, "author").map(str::to_owned),
        initials: attr(*node, "initials").map(str::to_owned),
        date: attr(*node, "date").map(str::to_owned),
        blocks,
        anchor_text: String::new(),
        page: 0,
      });
    }

    let mut read_parts = |suffix: &str,
                          package: &mut Package,
                          media: &mut Media,
                          warnings: &mut Vec<String>|
     -> Result<Vec<Part>, DocumentError> {
      let mut parts = Vec::new();
      for path in targets(suffix) {
        let Some(bytes) = optional_part(package, Some(&path), warnings)? else {
          continue;
        };
        let Some(doc) = optional_xml(Some(&bytes), Some(&path), warnings) else {
          continue;
        };
        let rels = relationships_of(package, &path)?;
        let mut reader = Reader::new(&shared, &rels, media);
        let mut blocks = Vec::new();
        reader.blocks(doc.root_element(), &mut blocks, 0)?;
        tracked |= reader.tracked;
        parts.push(Part { name: path, blocks });
      }
      Ok(parts)
    };
    let headers = read_parts("/header", &mut package, &mut media, &mut warnings)?;
    let footers = read_parts("/footer", &mut package, &mut media, &mut warnings)?;

    let main_bytes = package.part(&main)?;
    let main_xml = parse_xml(&main_bytes)?;
    let body = child(main_xml.root_element(), "body")
      .ok_or_else(|| DocumentError::Malformed(format!("{main} has no body")))?;
    let mut reader = Reader::for_body(&shared, &main_rels, &mut media, body);
    let mut blocks = Vec::new();
    reader.blocks(body, &mut blocks, 0)?;
    tracked |= reader.tracked;
    let Reader {
      mut sections,
      page: page_count,
      fields,
      mut comment_anchors,
      comment_pages,
      note_pages,
      section_parts,
      ..
    } = reader;
    place_parts(&mut sections, &section_parts, &headers, &footers, &blocks);
    for (at, note) in notes.iter_mut().enumerate() {
      note.page = note_pages.get(&place(at)).copied().unwrap_or(0);
    }
    for comment in &mut comments {
      comment.anchor_text = comment_anchors.remove(&comment.id).unwrap_or_default();
      comment.page = comment_pages.get(&comment.id).copied().unwrap_or(0);
    }

    let mut images = Vec::with_capacity(media.targets.len());
    for (target, linked) in media.targets {
      if linked {
        images.push(Image {
          name: target,
          mime: None,
          width: None,
          height: None,
          data: Vec::new(),
          linked: true,
        });
        continue;
      }
      let data = optional_part(&mut package, Some(&target), &mut warnings)?.unwrap_or_default();
      let info = image_info(&data);
      images.push(Image {
        name: target,
        mime: info.map(|i| i.mime),
        width: info.and_then(|i| i.width),
        height: info.and_then(|i| i.height),
        data,
        linked: false,
      });
    }

    let props = package.doc_props()?;
    Ok(Document {
      language: styles.language.clone().or_else(|| props.language.clone()),
      props,
      has_macros,
      is_template,
      has_tracked_changes: tracked,
      sections,
      blocks,
      headers,
      footers,
      notes,
      comments,
      images,
      fields,
      page_count,
      warnings,
    })
  }
}

#[cfg(test)]
mod testing;
#[cfg(test)]
mod tests;
