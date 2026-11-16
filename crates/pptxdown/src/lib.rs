//! PresentationML decks: a `.pptx` read into slides.
//!
//! [`Presentation::open`] takes the bytes of a file. The package layer (`ooxmldown`) opens the
//! zip within limits and finds parts through relationships; `inherit` reads the layouts and
//! masters a slide is built on; `slide` reads a slide's shapes, taking each one's box and text
//! styles from those layers where the shape states none; `body` reads the text in them. Nothing
//! here depends on Node.

mod body;
mod error;
mod inherit;
mod layout;
mod lists;
mod markdown;
mod meta;
mod model;
mod slide;
mod source;
mod text;
mod xml;

use std::collections::HashMap;

use ooxmldown::{Package, PackageError, PackageKind, image_info, parse_xml};
use roxmltree::Node;

use body::{Links, read_paragraphs};
use inherit::{Layer, Layers, ListStyle, Master, read_placeholder};
use slide::{Store, Transform, TreeReader, order};
use source::{Source, optional_part, relationships_of, target_of};
use xml::{attr, child, emu, flag, is, rel_attr};

pub use error::PresentationError;
pub use lists::{
  Annotation, AnnotationKind, ChartUse, ClipUse, GridCell, PartKind, Placement, TableGrid,
};
pub use meta::Metadata;
pub use model::{
  Align, Anchor, ChartShape, Comment, Diagram, Frame, Group, Image, Insets, LinkTarget, Media,
  MediaKind, MediaShape, Paragraph, Picture, Placeholder, PlaceholderKind, Presentation, Run,
  Shape, ShapeInfo, Slide, Table, TableCell, TableRow, TextShape,
};
pub use ooxmldown::{Chart, ChartKind, ChartSeries, DocProps, cell_ref, column_letters};
pub use text::StructuredText;

/// How a presentation is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
  /// Whether hidden slides are read. They are by default; left out, the slides after one keep
  /// their numbers.
  pub hidden_slides: bool,
}

impl Default for Options {
  fn default() -> Self {
    Options {
      hidden_slides: true,
    }
  }
}

/// The first bytes of an OLE compound file.
const OLE_MAGIC: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
/// A deck holds this many slides at most.
const MAX_SLIDES: usize = 10_000;
/// The size of a slide when the deck states none: ten inches by seven and a half.
const DEFAULT_SIZE: (f64, f64) = (720.0, 540.0);

/// What the package says about the deck besides its slides.
struct Kind {
  has_macros: bool,
  is_template: bool,
  is_show: bool,
}

/// A layout, and the master it is built on.
struct LoadedLayout {
  layer: Layer,
  master: Option<String>,
}

/// The layouts and masters read so far, by part. A deck has few of each and many slides on them.
#[derive(Default)]
struct LayerCache {
  layouts: HashMap<String, Option<LoadedLayout>>,
  masters: HashMap<String, Option<Master>>,
}

impl LayerCache {
  /// Reads the layout at `part`, and the master under it, unless they have been read.
  fn load(
    &mut self,
    source: &mut dyn Source,
    part: &str,
    warnings: &mut Vec<String>,
  ) -> Result<(), PresentationError> {
    if self.layouts.contains_key(part) {
      return Ok(());
    }
    let mut loaded = None;
    if let Some(bytes) = optional_part(source, part, warnings)? {
      match parse_xml(&bytes) {
        Ok(doc) => {
          let master = target_of(&relationships_of(source, part)?, "/slideMaster");
          loaded = Some(LoadedLayout {
            layer: Layer::read(doc.root_element()),
            master,
          });
        }
        Err(error) => warnings.push(format!("{part} was left out: {error}")),
      }
    }
    let master = loaded.as_ref().and_then(|layout| layout.master.clone());
    self.layouts.insert(part.to_owned(), loaded);
    if let Some(part) = master
      && !self.masters.contains_key(&part)
    {
      let master = match optional_part(source, &part, warnings)? {
        Some(bytes) => parse_xml(&bytes)
          .ok()
          .map(|doc| Master::read(doc.root_element())),
        None => None,
      };
      self.masters.insert(part, master);
    }
    Ok(())
  }

  fn layout(&self, part: Option<&str>) -> Option<&LoadedLayout> {
    self.layouts.get(part?)?.as_ref()
  }

  fn master(&self, layout: Option<&LoadedLayout>) -> Option<&Master> {
    self.masters.get(layout?.master.as_ref()?)?.as_ref()
  }
}

/// Someone who wrote comments, as the deck lists them.
struct Author {
  name: Option<String>,
  initials: Option<String>,
}

/// Who wrote the comments, by the id a comment names its author with.
fn read_authors(
  source: &mut dyn Source,
  part: Option<String>,
  warnings: &mut Vec<String>,
) -> Result<HashMap<String, Author>, PresentationError> {
  let mut authors = HashMap::new();
  if let Some(part) = part
    && let Some(bytes) = optional_part(source, &part, warnings)?
    && let Ok(doc) = parse_xml(&bytes)
  {
    for author in doc
      .descendants()
      .filter(|node| is(*node, "cmAuthor") || is(*node, "author"))
    {
      if let Some(id) = attr(author, "id") {
        let field = |local: &str| attr(author, local).map(str::to_owned);
        authors.insert(
          id.to_owned(),
          Author {
            name: field("name"),
            initials: field("initials"),
          },
        );
      }
    }
  }
  Ok(authors)
}

/// The comments of one comments part, replies after the comment they answer. The older form
/// keeps a comment's words in a text element, the threaded form in a text body.
fn read_comments(
  root: Node<'_, '_>,
  authors: &HashMap<String, Author>,
  links: &Links<'_>,
) -> Vec<Comment> {
  root
    .descendants()
    .filter(|node| is(*node, "cm") || is(*node, "reply"))
    .filter_map(|comment| {
      let text = match child(comment, "txBody") {
        Some(body) => text::lines_of(&read_paragraphs(body, &ListStyle::default(), links)),
        None => child(comment, "text")?.text().unwrap_or("").to_owned(),
      };
      let author = attr(comment, "authorId").and_then(|id| authors.get(id));
      Some(Comment {
        author: author.and_then(|author| author.name.clone()),
        initials: author.and_then(|author| author.initials.clone()),
        date: attr(comment, "created")
          .or_else(|| attr(comment, "dt"))
          .map(str::to_owned),
        text,
      })
    })
    .collect()
}

/// Reads a deck from its parts. `main` is the presentation part.
fn assemble(
  source: &mut dyn Source,
  main: &str,
  props: DocProps,
  kind: &Kind,
  options: Options,
) -> Result<Presentation, PresentationError> {
  let main_bytes = source.part(main)?;
  let main_xml = parse_xml(&main_bytes)?;
  let root = main_xml.root_element();
  let main_rels = relationships_of(source, main)?;
  let mut warnings = Vec::new();

  let size = child(root, "sldSz");
  let dimension = |local: &str| size.and_then(|size| emu(attr(size, local)));
  let (width, height) = match (dimension("cx"), dimension("cy")) {
    (Some(width), Some(height)) if width > 0.0 && height > 0.0 => (width, height),
    _ => DEFAULT_SIZE,
  };
  let defaults = ListStyle::read(child(root, "defaultTextStyle"));

  // The order of the slides is the order of this list, not of the parts' names.
  let listed: Vec<(Option<&str>, String)> = child(root, "sldIdLst")
    .into_iter()
    .flat_map(|list| list.children())
    .filter(|node| is(*node, "sldId"))
    .filter_map(|node| {
      let relationship = main_rels.get(rel_attr(node, "id")?)?;
      (!relationship.external).then(|| (attr(node, "id"), relationship.target.clone()))
    })
    .collect();
  if listed.len() > MAX_SLIDES {
    return Err(PresentationError::LimitExceeded(format!(
      "the deck lists {} slides; the limit is {MAX_SLIDES}",
      listed.len()
    )));
  }
  let numbers: HashMap<String, u32> = listed
    .iter()
    .zip(1u32..)
    .map(|((_, part), number)| (part.clone(), number))
    .collect();
  // A section names the slides it holds by the ids of that same list.
  let sections: HashMap<&str, &str> = root
    .descendants()
    .filter(|node| is(*node, "section"))
    .flat_map(|section| {
      let name = attr(section, "name").unwrap_or("");
      section
        .descendants()
        .filter(|node| is(*node, "sldId"))
        .filter_map(move |slide| Some((attr(slide, "id")?, name)))
    })
    .collect();

  let authors = {
    let mut authors = read_authors(
      source,
      target_of(&main_rels, "/commentAuthors"),
      &mut warnings,
    )?;
    authors.extend(read_authors(
      source,
      target_of(&main_rels, "/authors"),
      &mut warnings,
    )?);
    authors
  };

  let mut cache = LayerCache::default();
  let mut store = Store::default();
  // How often each language is named by the text of the slides.
  let mut languages: HashMap<String, usize> = HashMap::new();
  let mut slides = Vec::with_capacity(listed.len());
  for ((id, part), number) in listed.iter().zip(1u32..) {
    let Some(bytes) = optional_part(source, part, &mut warnings)? else {
      continue;
    };
    let doc = match parse_xml(&bytes) {
      Ok(doc) => doc,
      Err(error) => {
        warnings.push(format!("{part} was left out: {error}"));
        continue;
      }
    };
    let slide = doc.root_element();
    let hidden = flag(attr(slide, "show")) == Some(false);
    if hidden && !options.hidden_slides {
      continue;
    }
    for language in doc
      .descendants()
      .filter(|node| is(*node, "rPr"))
      .filter_map(|node| attr(node, "lang"))
    {
      *languages.entry(language.to_owned()).or_default() += 1;
    }
    let rels = relationships_of(source, part)?;
    let layout_part = target_of(&rels, "/slideLayout");
    if let Some(layout) = &layout_part {
      cache.load(source, layout, &mut warnings)?;
    }
    let layout = cache.layout(layout_part.as_deref());
    let layers = Layers {
      layout: layout.map(|loaded| &loaded.layer),
      master: cache.master(layout),
      defaults: &defaults,
    };

    let mut shapes = Vec::new();
    let mut reader = TreeReader {
      source: &mut *source,
      rels: &rels,
      layers,
      slides: &numbers,
      store: &mut store,
      warnings: &mut warnings,
      met: 0,
    };
    if let Some(content) = child(slide, "cSld") {
      shapes.extend(reader.background(content, width, height));
      if let Some(tree) = child(content, "spTree") {
        shapes.extend(reader.shapes(tree, &Transform::IDENTITY, 0)?);
      }
    }
    let shapes = order(shapes);
    let title = shapes
      .iter()
      .find(|shape| {
        shape
          .info()
          .placeholder
          .is_some_and(|placeholder| placeholder.kind.is_title())
      })
      .map(|shape| shape.text().replace('\n', " ").trim().to_owned())
      .filter(|title| !title.is_empty());

    // The notes page holds a picture of the slide, the notes, and a slide number; only the
    // notes are taken.
    let mut notes = Vec::new();
    if let Some(notes_part) = target_of(&rels, "/notesSlide")
      && let Some(bytes) = optional_part(source, &notes_part, &mut warnings)?
      && let Ok(doc) = parse_xml(&bytes)
    {
      let notes_rels = relationships_of(source, &notes_part)?;
      let links = Links {
        rels: &notes_rels,
        slides: &numbers,
      };
      let body = doc
        .descendants()
        .filter(|node| is(*node, "sp"))
        .find(|shape| {
          read_placeholder(child(*shape, "nvSpPr"))
            .is_some_and(|placeholder| placeholder.kind == PlaceholderKind::Body)
        })
        .and_then(|shape| child(shape, "txBody"));
      if let Some(body) = body {
        notes = read_paragraphs(body, &defaults, &links)
          .into_iter()
          .filter(|paragraph| !paragraph.runs.is_empty())
          .collect();
      }
    }

    let mut comments = Vec::new();
    let mut comment_parts: Vec<&String> = rels
      .values()
      .filter(|relationship| !relationship.external && relationship.kind.ends_with("/comments"))
      .map(|relationship| &relationship.target)
      .collect();
    comment_parts.sort();
    for comments_part in comment_parts {
      if let Some(bytes) = optional_part(source, comments_part, &mut warnings)?
        && let Ok(doc) = parse_xml(&bytes)
      {
        let links = Links {
          rels: &rels,
          slides: &numbers,
        };
        comments.extend(read_comments(doc.root_element(), &authors, &links));
      }
    }

    slides.push(Slide {
      number,
      name: part.clone(),
      title,
      layout: layout.and_then(|loaded| loaded.layer.name.clone()),
      section: id
        .and_then(|id| sections.get(id))
        .filter(|name| !name.is_empty())
        .map(|name| (*name).to_owned()),
      hidden,
      shapes,
      notes,
      comments,
    });
  }

  let mut images = Vec::with_capacity(store.pictures.len());
  for (target, linked) in store.pictures {
    let data = if linked {
      Vec::new()
    } else {
      optional_part(source, &target, &mut warnings)?.unwrap_or_default()
    };
    let info = image_info(&data);
    images.push(Image {
      name: target,
      mime: info.map(|i| i.mime),
      width: info.and_then(|i| i.width),
      height: info.and_then(|i| i.height),
      data,
      linked,
    });
  }
  let mut media = Vec::with_capacity(store.clips.len());
  for (target, linked, kind) in store.clips {
    let (data, mime) = if linked {
      (Vec::new(), None)
    } else {
      let mime = match source.content_type(&target) {
        Ok(mime) => mime,
        Err(PackageError::Malformed(_)) => None,
        Err(limit) => return Err(limit.into()),
      };
      (
        optional_part(source, &target, &mut warnings)?.unwrap_or_default(),
        mime,
      )
    };
    media.push(Media {
      name: target,
      kind,
      mime,
      data,
      linked,
    });
  }

  Ok(Presentation {
    props,
    has_macros: kind.has_macros,
    is_template: kind.is_template,
    is_show: kind.is_show,
    width,
    height,
    size_kind: size.and_then(|size| attr(size, "type")).map(str::to_owned),
    // The most named; of two named as often, the one that sorts first, so the answer is stable.
    language: languages
      .into_iter()
      .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))
      .map(|(language, _)| language),
    slide_count: u32::try_from(listed.len()).unwrap_or(u32::MAX),
    slides,
    images,
    media,
    warnings,
  })
}

impl Presentation {
  /// Reads a deck from the bytes of a `.pptx`, `.pptm`, `.potx` or `.ppsx` file.
  pub fn open(bytes: Vec<u8>) -> Result<Presentation, PresentationError> {
    Presentation::open_with(bytes, Options::default())
  }

  pub fn open_with(bytes: Vec<u8>, options: Options) -> Result<Presentation, PresentationError> {
    if bytes.starts_with(&OLE_MAGIC) {
      return Err(PresentationError::UnsupportedFormat(
        "OLE compound file: a legacy .ppt, or a presentation protected with a password".into(),
      ));
    }
    let mut package = Package::open(bytes)?;
    let kind = match package.kind() {
      Ok(kind) => kind,
      Err(PackageError::Malformed(_)) => PackageKind::Other,
      Err(limit) => return Err(limit.into()),
    };
    let kind = match kind {
      PackageKind::Presentation => Kind {
        has_macros: false,
        is_template: false,
        is_show: false,
      },
      PackageKind::MacroPresentation => Kind {
        has_macros: true,
        is_template: false,
        is_show: false,
      },
      PackageKind::TemplatePresentation => Kind {
        has_macros: false,
        is_template: true,
        is_show: false,
      },
      PackageKind::SlideShow => Kind {
        has_macros: false,
        is_template: false,
        is_show: true,
      },
      other => {
        return Err(PresentationError::UnsupportedFormat(
          other.description().to_string(),
        ));
      }
    };
    let main = package.officedocument()?;
    let props = package.doc_props()?;
    assemble(&mut package, &main, props, &kind, options)
  }
}

#[cfg(test)]
mod testing;
#[cfg(test)]
mod tests;
