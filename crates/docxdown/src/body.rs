//! A part's XML turned into blocks.
//!
//! The walker takes text wherever a document keeps it (the body, tables, tables in tables,
//! content controls, text boxes, hyperlinks, smart tags) and skips only what a reader of the
//! document would not see: deleted text, field codes, and the second copy of a text box that
//! `mc:Fallback` holds for older applications.

use std::collections::HashMap;

use ooxmldown::Relationship;
use roxmltree::Node;

use crate::error::DocumentError;
use crate::model::{
  Block, BreakKind, Cell, Field, FieldKind, ImageRef, Inline, Link, Margins, NoteKind, Paragraph,
  PartRefs, Row, Section, Table, TextRun,
};
use crate::numbering::{Counters, Numbering};
use crate::styles::{RunProps, Styles, read_align, read_heading, read_list};
use crate::xml::{
  attr, child, descendant, emu, is_mc, local, number, on_off, plain_attr, rel_attr, twips, val, wml,
};

/// Elements nest this deep at most: tables in cells, content controls, text boxes.
pub(crate) const MAX_DEPTH: u32 = 64;
/// A comment's anchor text is kept up to this many bytes.
const MAX_ANCHOR: usize = 2_000;

/// Which side of the tracked changes to read.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum View {
  /// As the document reads with every change accepted: insertions kept, deletions dropped.
  #[default]
  Final,
  /// As it read before the changes: insertions dropped, deletions kept.
  Original,
}

/// What every part of one document shares.
pub(crate) struct Shared<'a> {
  pub styles: &'a Styles,
  pub numbering: &'a Numbering,
  /// A note's place in `Document::notes`, by kind and id.
  pub notes: &'a HashMap<(NoteKind, i64), u32>,
  /// A comment's place in `Document::comments`, by id.
  pub comments: &'a HashMap<i64, u32>,
  pub view: View,
}

/// The pictures a document uses, each target once.
#[derive(Debug, Default)]
pub(crate) struct Media {
  /// The part path of an embedded picture or the address of a linked one, and which it is.
  pub targets: Vec<(String, bool)>,
  index: HashMap<String, u32>,
}

impl Media {
  fn intern(&mut self, target: &str, linked: bool) -> u32 {
    if let Some(&at) = self.index.get(target) {
      return at;
    }
    let at = u32::try_from(self.targets.len()).unwrap_or(u32::MAX);
    self.targets.push((target.to_owned(), linked));
    self.index.insert(target.to_owned(), at);
    at
  }
}

/// A legacy form field's definition, from `w:ffData`.
#[derive(Debug, Clone)]
struct FormData {
  name: Option<String>,
  kind: FieldKind,
  /// What the field holds when its result does not say: a checkbox's state, a list's choice.
  value: Option<String>,
}

/// A complex field in progress: `begin`, its instruction, `separate`, its result, `end`.
#[derive(Debug)]
struct OpenField {
  instruction: String,
  form: Option<FormData>,
  /// Where the result starts in the inlines, once `separate` has been seen.
  result_from: Option<usize>,
  /// The page the field starts on.
  page: u32,
}

/// The size and description a drawing gives the pictures inside it.
#[derive(Debug, Clone, Default)]
struct Frame {
  width: Option<f64>,
  height: Option<f64>,
  alt_text: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Merge {
  None,
  Restart,
  Continue,
}

struct RawCell {
  cell: Cell,
  merge: Merge,
}

/// The parts a section names for one of header or footer, as their paths in the package.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct NamedParts {
  pub default: Option<String>,
  pub first: Option<String>,
  pub even: Option<String>,
}

/// The headers and footers one section names.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct SectionParts {
  pub headers: NamedParts,
  pub footers: NamedParts,
}

/// The pages a table row covers while it is being read.
#[derive(Debug, Clone, Copy)]
struct RowPages {
  /// The page the row starts on, which every one of its cells starts on.
  start: u32,
  /// The furthest page any of its cells has reached.
  end: u32,
  /// Whether the row has put text down, and whether the cell being read has.
  has_text: bool,
  cell_has_text: bool,
  /// Whether a marker above all of the row's text put the whole row on a new page.
  moved: bool,
  /// Whether a page ended part of the way through the row.
  split: bool,
}

pub(crate) struct Reader<'a> {
  shared: &'a Shared<'a>,
  /// The relationships of the part being read, by id.
  rels: &'a HashMap<String, Relationship>,
  media: &'a mut Media,
  counters: Counters,
  /// Pages and sections are tracked in the body only.
  paged: bool,
  pub page: u32,
  /// Whether each section, in order, starts on a new page.
  section_starts: Vec<bool>,
  pub sections: Vec<Section>,
  /// An author's break has been counted and no text has followed it yet, so Word's own marker
  /// for the same break is not counted again.
  after_explicit_break: bool,
  /// The rows being read, innermost last.
  rows: Vec<RowPages>,
  /// A row that a page ended part of the way through has just closed and no text has followed
  /// it. Word marks that page twice: where the row broke, and again on whatever starts next.
  after_split_row: bool,
  any_text: bool,
  /// The page the paragraph being read first put anything on.
  first_page: Option<u32>,
  /// The next text starts a run of its own, whatever stands before it.
  split_run: bool,
  text_box_depth: u32,
  open_fields: Vec<OpenField>,
  /// Comment ranges that have started and not ended, with the text seen since.
  open_comments: Vec<(i64, String)>,
  pub comment_anchors: HashMap<i64, String>,
  pub comment_pages: HashMap<i64, u32>,
  /// The page each note is first referred to on, by its place in `Document::notes`.
  pub note_pages: HashMap<u32, u32>,
  /// The headers and footers each section names, in step with `sections`.
  pub section_parts: Vec<SectionParts>,
  pub fields: Vec<Field>,
  pub tracked: bool,
}

/// The branch of an `mc:AlternateContent` to read: the first choice, or the fallback when there
/// is none. Reading both would repeat the content.
fn chosen<'a>(alternate: Node<'a, 'a>) -> Option<Node<'a, 'a>> {
  alternate
    .children()
    .find(|c| is_mc(*c, "Choice"))
    .or_else(|| alternate.children().find(|c| is_mc(*c, "Fallback")))
}

fn elements<'a>(parent: Node<'a, 'a>) -> impl Iterator<Item = Node<'a, 'a>> {
  parent.children().filter(|n| n.is_element())
}

/// The text of a stretch of inlines, links included.
pub(crate) fn inline_text(inlines: &[Inline], out: &mut String) {
  for inline in inlines {
    match inline {
      Inline::Text(run) => out.push_str(&run.text),
      Inline::Link(link) => inline_text(&link.inlines, out),
      Inline::Tab => out.push('\t'),
      Inline::Break(_) => out.push('\n'),
      Inline::Image(_) | Inline::NoteRef(_) | Inline::CommentRef(_) => {}
    }
  }
}

/// Whether two runs differ in nothing but their text.
fn formatted_alike(a: &TextRun, b: &TextRun) -> bool {
  a.bold == b.bold
    && a.italic == b.italic
    && a.underline == b.underline
    && a.strike == b.strike
    && a.vertical == b.vertical
    && a.size == b.size
    && a.font == b.font
}

/// Puts a field's result back where it was read, joined to the text before it when the two are
/// formatted alike.
fn rejoin(out: &mut Vec<Inline>, result: Vec<Inline>) {
  let mut result = result.into_iter();
  match (out.last_mut(), result.next()) {
    (Some(Inline::Text(last)), Some(Inline::Text(first))) if formatted_alike(last, &first) => {
      last.text.push_str(&first.text);
    }
    (_, Some(first)) => out.push(first),
    (_, None) => {}
  }
  out.extend(result);
}

fn text_of(inlines: &[Inline]) -> String {
  let mut out = String::new();
  inline_text(inlines, &mut out);
  out
}

/// A quoted or bare word from a field instruction, and what follows it.
fn next_token(input: &str) -> Option<(&str, &str)> {
  let input = input.trim_start();
  if let Some(rest) = input.strip_prefix('"') {
    let end = rest.find('"').unwrap_or(rest.len());
    return Some((&rest[..end], rest.get(end + 1..).unwrap_or("")));
  }
  if input.is_empty() {
    return None;
  }
  let end = input.find(char::is_whitespace).unwrap_or(input.len());
  Some((&input[..end], &input[end..]))
}

/// The target and bookmark of a `HYPERLINK` instruction: `HYPERLINK "https://…" \l "anchor"`.
fn hyperlink_target(arguments: &str) -> (Option<String>, Option<String>) {
  let (mut uri, mut dest) = (None, None);
  let mut rest = arguments;
  while let Some((token, after)) = next_token(rest) {
    rest = after;
    match token {
      "\\l" => {
        if let Some((anchor, after)) = next_token(rest) {
          dest = Some(anchor.to_owned());
          rest = after;
        }
      }
      // Other switches take one argument each, which is not the address.
      "\\o" | "\\t" | "\\m" => {
        if let Some((_, after)) = next_token(rest) {
          rest = after;
        }
      }
      switch if switch.starts_with('\\') => {}
      address => {
        if uri.is_none() && !address.is_empty() {
          uri = Some(address.to_owned());
        }
      }
    }
  }
  (uri, dest)
}

/// A `w:sym`: a character named by its code in a symbol font. Codes in the fonts' private range
/// mean nothing outside the font, so the common ones are mapped and the rest become a box.
fn symbol(node: Node<'_, '_>) -> String {
  let code = attr(node, "char").and_then(|c| u32::from_str_radix(c, 16).ok());
  let font = attr(node, "font").unwrap_or("").to_ascii_lowercase();
  let low = code.map(|c| c & 0xFF);
  let mapped = match (font.as_str(), low) {
    ("symbol", Some(0xB7)) | ("wingdings", Some(0x9F | 0xA7 | 0x6C)) => Some('•'),
    ("wingdings", Some(0xFC)) => Some('✓'),
    ("wingdings", Some(0xFB)) => Some('✗'),
    ("wingdings", Some(0xA8)) => Some('☐'),
    ("wingdings", Some(0xFE)) => Some('☑'),
    _ => None,
  };
  mapped
    .or_else(|| {
      code
        .and_then(char::from_u32)
        .filter(|c| !('\u{E000}'..='\u{F8FF}').contains(c) && !c.is_control())
    })
    .unwrap_or('□')
    .to_string()
}

/// `width:120pt;height:90pt` in a VML shape's style, as points.
fn vml_size(style: &str) -> (Option<f64>, Option<f64>) {
  let length = |name: &str| {
    style.split(';').find_map(|part| {
      let (key, value) = part.split_once(':')?;
      (key.trim() == name)
        .then(|| value.trim().strip_suffix("pt")?.trim().parse::<f64>().ok())
        .flatten()
    })
  };
  (length("width"), length("height"))
}

fn read_form(data: Node<'_, '_>) -> FormData {
  let name = child(data, "name").and_then(val).map(str::to_owned);
  if let Some(check) = child(data, "checkBox") {
    let checked = on_off(child(check, "checked"))
      .or(on_off(child(check, "default")))
      .unwrap_or(false);
    return FormData {
      name,
      kind: FieldKind::FormCheckbox,
      value: Some(checked.to_string()),
    };
  }
  if let Some(list) = child(data, "ddList") {
    let chosen = number::<usize>(child(list, "result").and_then(val)).unwrap_or(0);
    let entry = list
      .children()
      .filter(|n| wml(*n) == Some("listEntry"))
      .nth(chosen)
      .and_then(val);
    return FormData {
      name,
      kind: FieldKind::FormDropDown,
      value: Some(entry.unwrap_or("").to_owned()),
    };
  }
  FormData {
    name,
    kind: FieldKind::FormText,
    value: None,
  }
}

pub(crate) fn read_section(node: Node<'_, '_>) -> Section {
  let size = child(node, "pgSz");
  let width = size.and_then(|s| twips(attr(s, "w")));
  let height = size.and_then(|s| twips(attr(s, "h")));
  let margin = |name: &str| {
    child(node, "pgMar")
      .and_then(|m| twips(attr(m, name)))
      .map_or(0.0, f64::abs)
  };
  Section {
    width,
    height,
    landscape: size.and_then(|s| attr(s, "orient")) == Some("landscape")
      || matches!((width, height), (Some(w), Some(h)) if w > h),
    margins: Margins {
      top: margin("top"),
      right: margin("right"),
      bottom: margin("bottom"),
      left: margin("left"),
    },
    columns: number::<u32>(child(node, "cols").and_then(|c| attr(c, "num")))
      .unwrap_or(1)
      .max(1),
    starts_page: child(node, "type").and_then(val) != Some("continuous"),
    title_page: on_off(child(node, "titlePg")).unwrap_or(false),
    headers: PartRefs::default(),
    footers: PartRefs::default(),
  }
}

/// The section properties of a body, in order: one in the last paragraph of each section but the
/// last, and the last section's at the end of the body.
fn section_nodes<'a>(body: Node<'a, 'a>) -> Vec<Node<'a, 'a>> {
  body
    .descendants()
    .filter(|n| wml(*n) == Some("sectPr"))
    .filter(|n| {
      n.parent()
        .is_some_and(|p| wml(p) == Some("pPr") || wml(p) == Some("body"))
    })
    .collect()
}

impl<'a> Reader<'a> {
  /// A reader for a part that has no pages of its own: a header, a note, a comment.
  pub(crate) fn new(
    shared: &'a Shared<'a>,
    rels: &'a HashMap<String, Relationship>,
    media: &'a mut Media,
  ) -> Self {
    Reader {
      shared,
      rels,
      media,
      counters: Counters::default(),
      paged: false,
      page: 1,
      section_starts: Vec::new(),
      sections: Vec::new(),
      after_explicit_break: false,
      rows: Vec::new(),
      after_split_row: false,
      any_text: false,
      first_page: None,
      split_run: false,
      text_box_depth: 0,
      open_fields: Vec::new(),
      open_comments: Vec::new(),
      comment_anchors: HashMap::new(),
      comment_pages: HashMap::new(),
      note_pages: HashMap::new(),
      section_parts: Vec::new(),
      fields: Vec::new(),
      tracked: false,
    }
  }

  /// A reader for the body, which counts pages and sections as it goes.
  pub(crate) fn for_body(
    shared: &'a Shared<'a>,
    rels: &'a HashMap<String, Relationship>,
    media: &'a mut Media,
    body: Node<'_, '_>,
  ) -> Self {
    let mut reader = Reader::new(shared, rels, media);
    reader.paged = true;
    reader.section_starts = section_nodes(body)
      .into_iter()
      .map(|node| read_section(node).starts_page)
      .collect();
    reader
  }

  fn deeper(depth: u32) -> Result<u32, DocumentError> {
    if depth >= MAX_DEPTH {
      return Err(DocumentError::LimitExceeded(format!(
        "content is nested more than {MAX_DEPTH} levels deep"
      )));
    }
    Ok(depth + 1)
  }

  fn section_index(&self) -> u32 {
    u32::try_from(self.sections.len()).unwrap_or(u32::MAX)
  }

  /// Counts a page break. An author's break always counts. The marker Word leaves where it last
  /// broke a page counts unless it stands for a break already counted: the one an author just
  /// made, or the one that fell inside the table row just closed.
  fn page_break(&mut self, explicit: bool) {
    // A text box floats beside the flow; a marker inside one says nothing about the page.
    if !self.paged || self.text_box_depth > 0 {
      return;
    }
    if explicit {
      self.page += 1;
      self.after_explicit_break = true;
      self.after_split_row = false;
    } else if self.after_split_row {
      self.after_split_row = false;
    } else if !self.after_explicit_break {
      // Word marks a row that starts a page in one cell. Another cell saying so adds nothing.
      if self
        .rows
        .last()
        .is_some_and(|row| row.moved && !row.cell_has_text)
      {
        return;
      }
      self.page += 1;
      for row in &mut self.rows {
        if row.has_text {
          // A break below text in a row is the row carried over the page.
          row.split = true;
        } else {
          // A break above all of a row's text is the row starting on the new page.
          row.start = self.page;
          row.moved = true;
        }
      }
    }
  }

  /// Fixes the page of the paragraph being read at the first thing it holds.
  fn pin_page(&mut self) {
    if self.first_page.is_none() {
      self.first_page = Some(self.page);
    }
  }

  /// A section ends: its properties are recorded, and the next one may start a new page.
  fn end_section(&mut self, node: Node<'_, '_>) {
    self.sections.push(read_section(node));
    let mut parts = SectionParts::default();
    for reference in elements(node) {
      let named = match wml(reference) {
        Some("headerReference") => &mut parts.headers,
        Some("footerReference") => &mut parts.footers,
        _ => continue,
      };
      let Some(target) = rel_attr(reference, "id")
        .and_then(|id| self.rels.get(id))
        .filter(|relationship| !relationship.external)
        .map(|relationship| relationship.target.clone())
      else {
        continue;
      };
      match attr(reference, "type") {
        Some("first") => named.first = Some(target),
        Some("even") => named.even = Some(target),
        _ => named.default = Some(target),
      }
    }
    self.section_parts.push(parts);
    let next_starts_page = self
      .section_starts
      .get(self.sections.len())
      .copied()
      .unwrap_or(false);
    if next_starts_page && self.any_text {
      self.page_break(true);
    }
  }

  /// Whether a tracked change is read in this view: an insertion in the final one, a deletion
  /// in the original one.
  fn tracked_kept(&mut self, inserted: bool) -> bool {
    self.tracked = true;
    inserted == (self.shared.view == View::Final)
  }

  /// For an element that wraps a tracked change, whether its content is read; `None` for any
  /// other element.
  fn tracked_change(&mut self, name: Option<&str>) -> Option<bool> {
    match name {
      Some("ins" | "moveTo") => Some(self.tracked_kept(true)),
      Some("del" | "moveFrom") => Some(self.tracked_kept(false)),
      _ => None,
    }
  }

  // ── Blocks ──────────────────────────────────────────────────────

  pub(crate) fn blocks(
    &mut self,
    parent: Node<'_, '_>,
    out: &mut Vec<Block>,
    depth: u32,
  ) -> Result<(), DocumentError> {
    let depth = Self::deeper(depth)?;
    for node in elements(parent) {
      if is_mc(node, "AlternateContent") {
        if let Some(branch) = chosen(node) {
          self.blocks(branch, out, depth)?;
        }
        continue;
      }
      if let Some(kept) = self.tracked_change(wml(node)) {
        if kept {
          self.blocks(node, out, depth)?;
        }
        continue;
      }
      match wml(node) {
        Some("p") => self.paragraph(node, out, depth)?,
        Some("tbl") => {
          let table = self.table(node, depth)?;
          out.push(Block::Table(table));
        }
        Some("sdt") => {
          let from = out.len();
          if let Some(content) = child(node, "sdtContent") {
            self.blocks(content, out, depth)?;
          }
          let value = out[from..]
            .iter()
            .map(Block::text)
            .collect::<Vec<_>>()
            .join("\n");
          self.record_control(node, value);
        }
        Some("customXml" | "smartTag") => self.blocks(node, out, depth)?,
        // The last section's properties close the body.
        Some("sectPr") if self.paged => self.end_section(node),
        _ => {}
      }
    }
    Ok(())
  }

  fn paragraph(
    &mut self,
    node: Node<'_, '_>,
    out: &mut Vec<Block>,
    depth: u32,
  ) -> Result<(), DocumentError> {
    let properties = child(node, "pPr");
    let style = self
      .shared
      .styles
      .paragraph(properties.and_then(|p| child(p, "pStyle")).and_then(val));
    // A list id of 0 on the paragraph turns off a list its style would give it.
    let list = match read_list(properties) {
      Some((0, _)) => None,
      Some(list) => Some(list),
      None => style.list.filter(|(id, _)| *id != 0),
    }
    .and_then(|(id, level)| self.counters.next(self.shared.numbering, id, level));
    let break_before = on_off(properties.and_then(|p| child(p, "pageBreakBefore")))
      .unwrap_or(style.page_break_before);
    if break_before && self.any_text {
      self.page_break(true);
    }

    // A paragraph inside this one (in a text box) keeps a first page of its own.
    let outer_first_page = self.first_page.take();
    let mut inlines = Vec::new();
    let mut boxes = Vec::new();
    let open_fields = self.open_fields.len();
    self.inlines(node, &mut inlines, &style.run, &mut boxes, depth)?;
    // A field that does not end in its paragraph keeps its result as ordinary text.
    self.open_fields.truncate(open_fields);
    // An empty paragraph is on the page it ends on: a marker in it moved it there.
    let page = self.first_page.take().unwrap_or(self.page);
    self.first_page = outer_first_page;

    out.push(Block::Paragraph(Paragraph {
      style: style.name,
      heading: read_heading(properties).or(style.heading),
      list,
      align: read_align(properties).or(style.align),
      inlines,
      section: self.section_index(),
      page,
      text_box: self.text_box_depth > 0,
    }));
    // What its text boxes hold follows the paragraph they are anchored in.
    out.append(&mut boxes);
    if self.paged
      && let Some(section) = properties.and_then(|p| child(p, "sectPr"))
    {
      self.end_section(section);
    }
    Ok(())
  }

  // ── Tables ──────────────────────────────────────────────────────

  fn table(&mut self, node: Node<'_, '_>, depth: u32) -> Result<Table, DocumentError> {
    let columns = child(node, "tblGrid")
      .map(|grid| {
        grid
          .children()
          .filter(|n| wml(*n) == Some("gridCol"))
          .map(|column| twips(attr(column, "w")).unwrap_or(0.0))
          .collect()
      })
      .unwrap_or_default();
    let (section, start_page) = (self.section_index(), self.page);
    let mut rows = Vec::new();
    self.rows(node, &mut rows, depth)?;
    // A table is on the page its first cell starts on, which a marker there may have moved.
    let page = rows
      .first()
      .and_then(|(_, cells)| cells.first())
      .and_then(|raw| raw.cell.blocks.first())
      .map_or(start_page, Block::page);
    Ok(Table {
      columns,
      rows: merge_vertically(rows),
      section,
      page,
    })
  }

  /// The rows under `parent`, which a content control or a tracked change may wrap.
  fn rows(
    &mut self,
    parent: Node<'_, '_>,
    rows: &mut Vec<(bool, Vec<RawCell>)>,
    depth: u32,
  ) -> Result<(), DocumentError> {
    let depth = Self::deeper(depth)?;
    for node in elements(parent) {
      if is_mc(node, "AlternateContent") {
        if let Some(branch) = chosen(node) {
          self.rows(branch, rows, depth)?;
        }
        continue;
      }
      if let Some(kept) = self.tracked_change(wml(node)) {
        if kept {
          self.rows(node, rows, depth)?;
        }
        continue;
      }
      match wml(node) {
        Some("tr") => {
          let properties = child(node, "trPr");
          // A row can itself be a tracked insertion or deletion.
          let inserted = properties.is_some_and(|p| child(p, "ins").is_some());
          let deleted = properties.is_some_and(|p| child(p, "del").is_some());
          if (inserted && !self.tracked_kept(true)) || (deleted && !self.tracked_kept(false)) {
            continue;
          }
          let header = on_off(properties.and_then(|p| child(p, "tblHeader"))).unwrap_or(false);
          let mut column = number::<u32>(
            properties
              .and_then(|p| child(p, "gridBefore"))
              .and_then(val),
          )
          .unwrap_or(0);
          self.rows.push(RowPages {
            start: self.page,
            end: self.page,
            has_text: false,
            cell_has_text: false,
            moved: false,
            split: false,
          });
          let mut cells = Vec::new();
          let read = self.cells(node, &mut cells, &mut column, depth);
          // Cells stand side by side: the row ends on the furthest page any of them reached.
          if let Some(row) = self.rows.pop() {
            self.page = row.end;
            if self.paged && self.text_box_depth == 0 {
              self.after_split_row = row.split;
            }
          }
          read?;
          rows.push((header, cells));
        }
        Some("sdt") => {
          if let Some(content) = child(node, "sdtContent") {
            self.rows(content, rows, depth)?;
          }
        }
        Some("customXml" | "smartTag") => self.rows(node, rows, depth)?,
        _ => {}
      }
    }
    Ok(())
  }

  fn cells(
    &mut self,
    parent: Node<'_, '_>,
    cells: &mut Vec<RawCell>,
    column: &mut u32,
    depth: u32,
  ) -> Result<(), DocumentError> {
    let depth = Self::deeper(depth)?;
    for node in elements(parent) {
      if is_mc(node, "AlternateContent") {
        if let Some(branch) = chosen(node) {
          self.cells(branch, cells, column, depth)?;
        }
        continue;
      }
      if let Some(kept) = self.tracked_change(wml(node)) {
        if kept {
          self.cells(node, cells, column, depth)?;
        }
        continue;
      }
      match wml(node) {
        Some("tc") => {
          let properties = child(node, "tcPr");
          let col_span = number::<u32>(properties.and_then(|p| child(p, "gridSpan")).and_then(val))
            .unwrap_or(1)
            .max(1);
          // A merge with no value continues the cell above; `restart` begins a new one.
          let merge = match properties.and_then(|p| child(p, "vMerge")) {
            None => Merge::None,
            Some(merge) if val(merge) == Some("restart") => Merge::Restart,
            Some(_) => Merge::Continue,
          };
          // Every cell of a row starts on the page the row starts on.
          if let Some(row) = self.rows.last_mut() {
            row.cell_has_text = false;
            self.page = row.start;
          }
          let mut blocks = Vec::new();
          self.blocks(node, &mut blocks, depth)?;
          if let Some(row) = self.rows.last_mut() {
            row.end = row.end.max(self.page);
          }
          cells.push(RawCell {
            cell: Cell {
              blocks,
              column: *column,
              col_span,
              row_span: 1,
            },
            merge,
          });
          *column = column.saturating_add(col_span);
        }
        Some("sdt") => {
          let from = cells.len();
          if let Some(content) = child(node, "sdtContent") {
            self.cells(content, cells, column, depth)?;
          }
          let value = cells[from..]
            .iter()
            .flat_map(|raw| raw.cell.blocks.iter().map(Block::text))
            .collect::<Vec<_>>()
            .join("\n");
          self.record_control(node, value);
        }
        Some("customXml" | "smartTag") => self.cells(node, cells, column, depth)?,
        _ => {}
      }
    }
    Ok(())
  }

  // ── Inlines ─────────────────────────────────────────────────────

  fn inlines(
    &mut self,
    parent: Node<'_, '_>,
    out: &mut Vec<Inline>,
    paragraph: &RunProps,
    boxes: &mut Vec<Block>,
    depth: u32,
  ) -> Result<(), DocumentError> {
    let depth = Self::deeper(depth)?;
    for node in elements(parent) {
      if is_mc(node, "AlternateContent") {
        if let Some(branch) = chosen(node) {
          self.inlines(branch, out, paragraph, boxes, depth)?;
        }
        continue;
      }
      if let Some(kept) = self.tracked_change(wml(node)) {
        if kept {
          self.inlines(node, out, paragraph, boxes, depth)?;
        }
        continue;
      }
      match wml(node) {
        Some("r") => self.run(node, out, paragraph, boxes, depth)?,
        Some("hyperlink") => {
          let uri = rel_attr(node, "id")
            .and_then(|id| self.rels.get(id))
            .filter(|relationship| relationship.external)
            .map(|relationship| relationship.target.clone());
          let page = self.page;
          let mut inner = Vec::new();
          self.inlines(node, &mut inner, paragraph, boxes, depth)?;
          out.push(Inline::Link(Link {
            uri,
            dest: attr(node, "anchor").map(str::to_owned),
            inlines: inner,
            page,
          }));
        }
        Some("fldSimple") => {
          let page = self.page;
          let mut result = Vec::new();
          self.inlines(node, &mut result, paragraph, boxes, depth)?;
          let field = OpenField {
            instruction: attr(node, "instr").unwrap_or("").to_owned(),
            form: None,
            result_from: None,
            page,
          };
          self.finish_field(field, result, out);
        }
        Some("sdt") => {
          let from = out.len();
          if let Some(content) = child(node, "sdtContent") {
            self.inlines(content, out, paragraph, boxes, depth)?;
          }
          let value = text_of(out.get(from..).unwrap_or(&[]));
          self.record_control(node, value);
        }
        Some("smartTag" | "customXml" | "dir" | "bdo") => {
          self.inlines(node, out, paragraph, boxes, depth)?;
        }
        Some("commentRangeStart") => {
          if let Some(id) = number::<i64>(attr(node, "id")) {
            self.open_comments.push((id, String::new()));
          }
        }
        Some("commentRangeEnd") => {
          if let Some(id) = number::<i64>(attr(node, "id"))
            && let Some(at) = self.open_comments.iter().position(|(open, _)| *open == id)
          {
            let (_, text) = self.open_comments.remove(at);
            self.comment_anchors.insert(id, text);
          }
        }
        None if matches!(local(node), Some("oMath" | "oMathPara")) => {
          // An equation, as the characters it is written with.
          let text: String = node
            .descendants()
            .filter(|n| local(*n) == Some("t"))
            .filter_map(|n| n.text())
            .collect();
          let format = self.format(&RunProps::default(), paragraph);
          self.text(&text, &format, out);
        }
        _ => {}
      }
    }
    Ok(())
  }

  /// A run's formatting: its own, then its character style's, then the paragraph style's, then
  /// the document's defaults. Bold, italic and strike are toggles between styles: set by two
  /// of them, they cancel. What the run sets itself always wins.
  fn format(&self, direct: &RunProps, paragraph: &RunProps) -> TextRun {
    let styles = self.shared.styles;
    let character = direct
      .style
      .as_deref()
      .map(|id| styles.character(id))
      .unwrap_or_default();
    let defaults = &styles.defaults;
    let toggle = |pick: fn(&RunProps) -> Option<bool>| {
      let mut on = pick(defaults) == Some(true);
      for level in [paragraph, &character] {
        if pick(level) == Some(true) {
          on = !on;
        }
      }
      pick(direct).unwrap_or(on)
    };
    let nearest = |pick: fn(&RunProps) -> Option<bool>| {
      pick(direct)
        .or(pick(&character))
        .or(pick(paragraph))
        .or(pick(defaults))
        .unwrap_or(false)
    };
    TextRun {
      text: String::new(),
      bold: toggle(|p| p.bold),
      italic: toggle(|p| p.italic),
      strike: toggle(|p| p.strike),
      underline: nearest(|p| p.underline),
      vertical: direct
        .vertical
        .or(character.vertical)
        .or(paragraph.vertical)
        .or(defaults.vertical)
        .unwrap_or_default(),
      size: direct
        .size
        .or(character.size)
        .or(paragraph.size)
        .or(defaults.size),
      font: direct
        .font
        .clone()
        .or(character.font)
        .or_else(|| paragraph.font.clone())
        .or_else(|| defaults.font.clone()),
    }
  }

  fn run(
    &mut self,
    node: Node<'_, '_>,
    out: &mut Vec<Inline>,
    paragraph: &RunProps,
    boxes: &mut Vec<Block>,
    depth: u32,
  ) -> Result<(), DocumentError> {
    let depth = Self::deeper(depth)?;
    let format = self.format(&RunProps::read(child(node, "rPr")), paragraph);
    for item in elements(node) {
      if is_mc(item, "AlternateContent") {
        if let Some(branch) = chosen(item) {
          for inner in elements(branch) {
            self.run_item(inner, &format, out, paragraph, boxes, depth)?;
          }
        }
        continue;
      }
      self.run_item(item, &format, out, paragraph, boxes, depth)?;
    }
    Ok(())
  }

  /// Whether the innermost open field is still in its instruction, which is not shown.
  fn in_instruction(&self) -> bool {
    self
      .open_fields
      .last()
      .is_some_and(|field| field.result_from.is_none())
  }

  fn run_item(
    &mut self,
    item: Node<'_, '_>,
    format: &TextRun,
    out: &mut Vec<Inline>,
    paragraph: &RunProps,
    boxes: &mut Vec<Block>,
    depth: u32,
  ) -> Result<(), DocumentError> {
    let name = wml(item);
    if self.in_instruction() && !matches!(name, Some("instrText" | "fldChar")) {
      return Ok(());
    }
    match name {
      Some("t") => self.text(item.text().unwrap_or(""), format, out),
      Some("delText") => {
        if self.shared.view == View::Original {
          self.text(item.text().unwrap_or(""), format, out);
        }
      }
      Some("instrText") => {
        if let Some(field) = self.open_fields.last_mut()
          && field.result_from.is_none()
        {
          field.instruction.push_str(item.text().unwrap_or(""));
        }
      }
      Some("tab" | "ptab") => {
        self.seen_text("\t");
        out.push(Inline::Tab);
      }
      Some("br") => match attr(item, "type") {
        Some("page") => {
          // The break ends the page the paragraph is on; it does not move the paragraph.
          self.pin_page();
          out.push(Inline::Break(BreakKind::Page));
          if self.any_text {
            self.page_break(true);
          }
        }
        Some("column") => out.push(Inline::Break(BreakKind::Column)),
        _ => {
          self.seen_text("\n");
          out.push(Inline::Break(BreakKind::Line));
        }
      },
      Some("cr") => {
        self.seen_text("\n");
        out.push(Inline::Break(BreakKind::Line));
      }
      Some("noBreakHyphen") => self.text("\u{2011}", format, out),
      Some("sym") => self.text(&symbol(item), format, out),
      Some("lastRenderedPageBreak") => self.page_break(false),
      Some("footnoteReference" | "endnoteReference") => {
        let kind = if name == Some("footnoteReference") {
          NoteKind::Footnote
        } else {
          NoteKind::Endnote
        };
        if let Some(id) = number::<i64>(attr(item, "id"))
          && let Some(&note) = self.shared.notes.get(&(kind, id))
        {
          self.pin_page();
          self.note_pages.entry(note).or_insert(self.page);
          out.push(Inline::NoteRef(note));
        }
      }
      Some("commentReference") => {
        if let Some(id) = number::<i64>(attr(item, "id"))
          && let Some(&comment) = self.shared.comments.get(&id)
        {
          self.comment_pages.entry(id).or_insert(self.page);
          out.push(Inline::CommentRef(comment));
        }
      }
      Some("fldChar") => match attr(item, "fldCharType") {
        Some("begin") => self.open_fields.push(OpenField {
          instruction: String::new(),
          form: child(item, "ffData").map(read_form),
          result_from: None,
          page: self.page,
        }),
        Some("separate") => {
          if let Some(field) = self.open_fields.last_mut() {
            field.result_from = Some(out.len());
            // The result is taken back out at `end`, so it must not join the text before it.
            self.split_run = true;
          }
        }
        Some("end") => {
          if let Some(field) = self.open_fields.pop() {
            let from = field.result_from.unwrap_or(out.len()).min(out.len());
            let result = out.split_off(from);
            self.split_run = false;
            self.finish_field(field, result, out);
          }
        }
        _ => {}
      },
      Some("ruby") => {
        // Text with a pronunciation guide above it: the text is kept, the guide is not.
        if let Some(base) = child(item, "rubyBase") {
          self.inlines(base, out, paragraph, boxes, depth)?;
        }
      }
      Some("drawing" | "pict" | "object") => {
        self.graphic(item, &Frame::default(), out, boxes, depth)?;
      }
      _ => {}
    }
    Ok(())
  }

  /// Notes that text has been read: for the page a paragraph starts on, and for the comments
  /// whose ranges are open.
  fn seen_text(&mut self, text: &str) {
    if self.paged && self.text_box_depth == 0 {
      self.after_explicit_break = false;
      self.after_split_row = false;
      for row in &mut self.rows {
        row.has_text = true;
        row.cell_has_text = true;
      }
    }
    self.pin_page();
    self.any_text = true;
    for (_, anchor) in &mut self.open_comments {
      if anchor.len() < MAX_ANCHOR {
        anchor.push_str(text);
      }
    }
  }

  /// Adds text, joining it to the run before it when the two are formatted alike.
  fn text(&mut self, text: &str, format: &TextRun, out: &mut Vec<Inline>) {
    if text.is_empty() {
      return;
    }
    self.seen_text(text);
    let joins = !std::mem::take(&mut self.split_run);
    if joins
      && let Some(Inline::Text(last)) = out.last_mut()
      && formatted_alike(last, format)
    {
      last.text.push_str(text);
      return;
    }
    out.push(Inline::Text(TextRun {
      text: text.to_owned(),
      ..format.clone()
    }));
  }

  /// A drawing, a legacy picture or an embedded object: the pictures in it, and what its text
  /// boxes hold.
  fn graphic(
    &mut self,
    node: Node<'_, '_>,
    frame: &Frame,
    out: &mut Vec<Inline>,
    boxes: &mut Vec<Block>,
    depth: u32,
  ) -> Result<(), DocumentError> {
    let depth = Self::deeper(depth)?;
    for item in elements(node) {
      if is_mc(item, "AlternateContent") {
        if let Some(branch) = chosen(item) {
          self.graphic(branch, frame, out, boxes, depth)?;
        }
        continue;
      }
      if wml(item) == Some("txbxContent") {
        self.text_box_depth += 1;
        let read = self.blocks(item, boxes, depth);
        self.text_box_depth -= 1;
        read?;
        continue;
      }
      match local(item) {
        // An inline or floating drawing states its size and description once for what it holds.
        Some("inline" | "anchor") => {
          let extent = elements(item).find(|n| local(*n) == Some("extent"));
          let properties = elements(item).find(|n| local(*n) == Some("docPr"));
          let inner = Frame {
            width: extent.and_then(|e| emu(plain_attr(e, "cx"))),
            height: extent.and_then(|e| emu(plain_attr(e, "cy"))),
            alt_text: properties
              .and_then(|p| plain_attr(p, "descr").or(plain_attr(p, "title")))
              .filter(|text| !text.is_empty())
              .map(str::to_owned),
          };
          self.graphic(item, &inner, out, boxes, depth)?;
        }
        Some("blip") => {
          let embedded = rel_attr(item, "embed").map(|id| (id, false));
          if let Some((id, _)) = embedded.or(rel_attr(item, "link").map(|id| (id, true))) {
            self.image(id, frame.clone(), out);
          }
        }
        // A legacy picture: its shape carries the size, in a style.
        Some("shape") => {
          let (width, height) = vml_size(plain_attr(item, "style").unwrap_or(""));
          let inner = Frame {
            width,
            height,
            alt_text: plain_attr(item, "alt")
              .filter(|text| !text.is_empty())
              .map(str::to_owned),
          };
          self.graphic(item, &inner, out, boxes, depth)?;
        }
        Some("imagedata") => {
          if let Some(id) = rel_attr(item, "id") {
            self.image(id, frame.clone(), out);
          }
        }
        _ => self.graphic(item, frame, out, boxes, depth)?,
      }
    }
    Ok(())
  }

  fn image(&mut self, relationship: &str, frame: Frame, out: &mut Vec<Inline>) {
    let Some(relationship) = self.rels.get(relationship) else {
      return;
    };
    let image = self
      .media
      .intern(&relationship.target, relationship.external);
    self.pin_page();
    out.push(Inline::Image(ImageRef {
      image,
      display_width: frame.width,
      display_height: frame.height,
      alt_text: frame.alt_text,
      page: self.page,
    }));
  }

  // ── Fields ──────────────────────────────────────────────────────

  /// A field has ended: its result goes back into the paragraph, wrapped in a link when the
  /// field is one, and a form or merge field is recorded with its value.
  fn finish_field(&mut self, field: OpenField, result: Vec<Inline>, out: &mut Vec<Inline>) {
    // A field inside another field's instruction feeds that instruction; it shows nothing.
    let shown = !self.in_instruction();
    let (name, arguments) = next_token(&field.instruction).unwrap_or(("", ""));
    match name.to_ascii_uppercase().as_str() {
      "HYPERLINK" if shown => {
        let (uri, dest) = hyperlink_target(arguments);
        out.push(Inline::Link(Link {
          uri,
          dest,
          inlines: result,
          page: field.page,
        }));
        return;
      }
      "MERGEFIELD" => {
        self.fields.push(Field {
          kind: FieldKind::Merge,
          name: next_token(arguments).map(|(name, _)| name.to_owned()),
          title: None,
          value: text_of(&result),
          page: field.page,
        });
      }
      _ => {
        if let Some(form) = field.form {
          self.fields.push(Field {
            kind: form.kind,
            name: form.name,
            title: None,
            value: form.value.unwrap_or_else(|| text_of(&result)),
            page: field.page,
          });
        }
      }
    }
    if shown {
      rejoin(out, result);
    }
  }

  /// Records a content control with the value it holds. Controls that only group content, such
  /// as a table of contents or a bibliography, are not fields.
  fn record_control(&mut self, control: Node<'_, '_>, value: String) {
    let Some(properties) = child(control, "sdtPr") else {
      return;
    };
    if [
      "docPartObj",
      "docPartList",
      "bibliography",
      "citation",
      "group",
      "equation",
    ]
    .iter()
    .any(|name| child(properties, name).is_some())
    {
      return;
    }
    let checkbox = descendant(properties, "checkbox");
    let kind = if let Some(checkbox) = checkbox {
      let checked = descendant(checkbox, "checked")
        .and_then(|c| c.attributes().find(|a| a.name() == "val"))
        .is_some_and(|a| !matches!(a.value(), "0" | "false" | "off"));
      self.fields.push(Field {
        kind: FieldKind::Checkbox,
        name: child(properties, "tag").and_then(val).map(str::to_owned),
        title: child(properties, "alias").and_then(val).map(str::to_owned),
        value: checked.to_string(),
        page: self.page,
      });
      return;
    } else if child(properties, "date").is_some() {
      FieldKind::Date
    } else if child(properties, "dropDownList").is_some() {
      FieldKind::DropDown
    } else if child(properties, "comboBox").is_some() {
      FieldKind::ComboBox
    } else if child(properties, "picture").is_some() {
      FieldKind::Picture
    } else if child(properties, "text").is_some() {
      FieldKind::Text
    } else {
      FieldKind::RichText
    };
    // While a control shows its placeholder it holds nothing.
    let placeholder = on_off(child(properties, "showingPlcHdr")).unwrap_or(false);
    self.fields.push(Field {
      kind,
      name: child(properties, "tag").and_then(val).map(str::to_owned),
      title: child(properties, "alias").and_then(val).map(str::to_owned),
      value: if placeholder { String::new() } else { value },
      page: self.page,
    });
  }
}

/// Turns vertical merges into row spans: a `restart` cell takes one more row for each `continue`
/// cell under it in the same column, and the continuing cells are dropped.
fn merge_vertically(rows: Vec<(bool, Vec<RawCell>)>) -> Vec<Row> {
  let mut out: Vec<Row> = Vec::with_capacity(rows.len());
  // The cell a merge in each grid column started at: its row and its place in that row.
  let mut open: HashMap<u32, (usize, usize)> = HashMap::new();
  for (header, cells) in rows {
    let mut kept = Vec::with_capacity(cells.len());
    let mut continued = Vec::new();
    for raw in cells {
      let column = raw.cell.column;
      match raw.merge {
        Merge::Continue if open.contains_key(&column) => {
          if let Some(&(row, cell)) = open.get(&column)
            && let Some(anchor) = out.get_mut(row).and_then(|r| r.cells.get_mut(cell))
          {
            anchor.row_span += 1;
          }
          continued.push(column);
        }
        Merge::Restart => {
          open.insert(column, (out.len(), kept.len()));
          continued.push(column);
          kept.push(raw.cell);
        }
        // A plain cell, or a `continue` with nothing above it to continue.
        Merge::None | Merge::Continue => kept.push(raw.cell),
      }
    }
    // A merge ends at the first row that does not continue it.
    open.retain(|column, _| continued.contains(column));
    out.push(Row {
      cells: kept,
      header,
    });
  }
  out
}

#[cfg(test)]
mod tests;
