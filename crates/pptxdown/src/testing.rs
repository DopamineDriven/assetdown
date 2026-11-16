//! Decks made from fragments of XML and held in memory, for the tests of the reader and of what
//! is built on it. A deck made here goes through the same assembly as one opened from a file.

use std::collections::HashMap;

use ooxmldown::{DocProps, PackageError, Relationship};

use crate::error::PresentationError;
use crate::model::Presentation;
use crate::source::Source;
use crate::{Kind, Options, assemble};

const NAMESPACES: &str = r#"xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:dgm="http://schemas.openxmlformats.org/drawingml/2006/diagram" xmlns:dsp="http://schemas.microsoft.com/office/drawing/2008/diagram" xmlns:p14="http://schemas.microsoft.com/office/powerpoint/2010/main" xmlns:asvg="http://schemas.microsoft.com/office/drawing/2016/SVG/main" xmlns:p188="http://schemas.microsoft.com/office/powerpoint/2018/8/main" xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math""#;

const MAIN: &str = "ppt/presentation.xml";
const MASTER: &str = "ppt/slideMasters/slideMaster1.xml";
const LAYOUT: &str = "ppt/slideLayouts/slideLayout1.xml";

/// Parts held in memory. A part or a relationships list that was never given is missing, which
/// the reader is told the way a package tells it.
#[derive(Default)]
struct Parts {
  parts: HashMap<String, Vec<u8>>,
  rels: HashMap<String, Vec<Relationship>>,
  types: HashMap<String, String>,
}

impl Source for Parts {
  fn part(&mut self, path: &str) -> Result<Vec<u8>, PackageError> {
    self
      .parts
      .get(path)
      .cloned()
      .ok_or_else(|| PackageError::Malformed(format!("no part {path}")))
  }

  fn relationships(&mut self, part: &str) -> Result<Vec<Relationship>, PackageError> {
    self
      .rels
      .get(part)
      .cloned()
      .ok_or_else(|| PackageError::Malformed(format!("no relationships for {part}")))
  }

  fn content_type(&mut self, part: &str) -> Result<Option<String>, PackageError> {
    Ok(self.types.get(part).cloned())
  }
}

/// An element `root` of PresentationML holding `inner`, with every namespace the tests use.
fn element(root: &str, attributes: &str, inner: &str) -> String {
  format!("<p:{root} {NAMESPACES} {attributes}>{inner}</p:{root}>")
}

/// A length in points as the whole number of English Metric Units a file writes.
pub(crate) fn emu(points: f64) -> i64 {
  (points * 12_700.0).round() as i64
}

/// `a:xfrm` for a box given in points: its left, its top, its width and its height.
pub(crate) fn xfrm(x: f64, y: f64, width: f64, height: f64) -> String {
  format!(
    r#"<a:xfrm><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/></a:xfrm>"#,
    emu(x),
    emu(y),
    emu(width),
    emu(height)
  )
}

/// A paragraph holding one run of `text`.
pub(crate) fn para(text: &str) -> String {
  format!("<a:p><a:r><a:t>{text}</a:t></a:r></a:p>")
}

/// A shape with a text body. `placeholder` is what stands inside `p:nvPr` (a `p:ph`, or
/// nothing for a plain text box), `properties` what stands inside `p:spPr` (an `a:xfrm`, or
/// nothing), and `body` what stands inside `p:txBody`: body properties, a list style,
/// paragraphs.
pub(crate) fn shape(placeholder: &str, properties: &str, body: &str) -> String {
  format!(
    r#"<p:sp><p:nvSpPr><p:cNvPr id="2" name="Shape"/><p:cNvSpPr/><p:nvPr>{placeholder}</p:nvPr></p:nvSpPr><p:spPr>{properties}</p:spPr><p:txBody>{body}</p:txBody></p:sp>"#
  )
}

/// A placeholder of the kind `kind` names (`type="title"`, `idx="1"`), with no box of its own.
pub(crate) fn placeholder(kind: &str, body: &str) -> String {
  shape(&format!("<p:ph {kind}/>"), "", body)
}

/// A plain text box at a place given in points, holding `paragraphs`.
pub(crate) fn text_box(x: f64, y: f64, width: f64, height: f64, paragraphs: &str) -> String {
  shape("", &xfrm(x, y, width, height), paragraphs)
}

/// A graphic frame at a place given in points (its left, top, width and height), holding
/// `data`: a table, or what names a chart or a diagram.
pub(crate) fn graphic_frame(at: (f64, f64, f64, f64), data: &str) -> String {
  format!(
    r#"<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="5" name="Frame"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr><p:xfrm><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/></p:xfrm><a:graphic><a:graphicData uri="x">{data}</a:graphicData></a:graphic></p:graphicFrame>"#,
    emu(at.0),
    emu(at.1),
    emu(at.2),
    emu(at.3)
  )
}

/// A deck being put together.
pub(crate) struct Deck {
  parts: Parts,
  /// Each slide's part, in order.
  slides: Vec<String>,
  size: String,
  defaults: String,
  extensions: String,
  options: Options,
}

impl Deck {
  /// A deck with no slides yet, 960 by 540 points.
  pub(crate) fn new() -> Deck {
    Deck {
      parts: Parts::default(),
      slides: Vec::new(),
      size: format!(r#"<p:sldSz cx="{}" cy="{}"/>"#, emu(960.0), emu(540.0)),
      defaults: String::new(),
      extensions: String::new(),
      options: Options::default(),
    }
  }

  /// What stands in `presentation.xml` for the slide size.
  pub(crate) fn size(mut self, element: &str) -> Deck {
    self.size = element.to_owned();
    self
  }

  /// What stands inside `p:defaultTextStyle`.
  pub(crate) fn defaults(mut self, levels: &str) -> Deck {
    self.defaults = format!("<p:defaultTextStyle>{levels}</p:defaultTextStyle>");
    self
  }

  /// What stands after the default text style: an extension list, where sections are kept.
  pub(crate) fn extensions(mut self, list: &str) -> Deck {
    self.extensions = list.to_owned();
    self
  }

  pub(crate) fn options(mut self, options: Options) -> Deck {
    self.options = options;
    self
  }

  /// A part at `path` holding `content`.
  pub(crate) fn part(mut self, path: &str, content: &str) -> Deck {
    self
      .parts
      .parts
      .insert(path.to_owned(), content.as_bytes().to_vec());
    self
  }

  /// A part holding `root` of PresentationML around `inner`.
  pub(crate) fn xml(self, path: &str, root: &str, inner: &str) -> Deck {
    let content = element(root, "", inner);
    self.part(path, &content)
  }

  pub(crate) fn content_type(mut self, path: &str, kind: &str) -> Deck {
    self.parts.types.insert(path.to_owned(), kind.to_owned());
    self
  }

  /// A relationship of the part at `from`: its id, the end of its type, and its target, which
  /// is a part of the deck unless `external` is set.
  pub(crate) fn link(
    mut self,
    from: &str,
    id: &str,
    kind: &str,
    target: &str,
    external: bool,
  ) -> Deck {
    self
      .parts
      .rels
      .entry(from.to_owned())
      .or_default()
      .push(Relationship {
        id: id.to_owned(),
        kind: format!("http://schemas.openxmlformats.org/officeDocument/2006/relationships{kind}"),
        target: target.to_owned(),
        external,
      });
    self
  }

  /// A relationship of the slide added last.
  pub(crate) fn rel(self, id: &str, kind: &str, target: &str, external: bool) -> Deck {
    let slide = self.slides.last().cloned().expect("a slide to relate from");
    self.link(&slide, id, kind, target, external)
  }

  /// The master: the shapes on it, and what stands inside its `p:txStyles`.
  pub(crate) fn master(self, tree: &str, text_styles: &str) -> Deck {
    let inner =
      format!("<p:cSld><p:spTree>{tree}</p:spTree></p:cSld><p:txStyles>{text_styles}</p:txStyles>");
    self.xml(MASTER, "sldMaster", &inner)
  }

  /// The layout every slide added after it is built on: its name and the shapes on it.
  pub(crate) fn layout(self, name: &str, tree: &str) -> Deck {
    let inner = format!(r#"<p:cSld name="{name}"><p:spTree>{tree}</p:spTree></p:cSld>"#);
    self
      .xml(LAYOUT, "sldLayout", &inner)
      .link(LAYOUT, "rId1", "/slideMaster", MASTER, false)
  }

  /// A slide whose shape tree holds `tree`. `attributes` go on `p:sld`, and `before` stands
  /// in `p:cSld` ahead of the tree, where a background is kept.
  pub(crate) fn slide_with(mut self, attributes: &str, before: &str, tree: &str) -> Deck {
    let path = format!("ppt/slides/slide{}.xml", self.slides.len() + 1);
    let inner =
      format!("<p:cSld>{before}<p:spTree><p:nvGrpSpPr/><p:grpSpPr/>{tree}</p:spTree></p:cSld>");
    let content = element("sld", attributes, &inner);
    self.slides.push(path.clone());
    let with_part = self.part(&path, &content);
    if with_part.parts.parts.contains_key(LAYOUT) {
      with_part.link(&path, "rIdLayout", "/slideLayout", LAYOUT, false)
    } else {
      with_part
    }
  }

  pub(crate) fn slide(self, tree: &str) -> Deck {
    self.slide_with("", "", tree)
  }

  /// Speaker notes for the slide added last: a notes page with the slide's picture, the notes
  /// `body`, and a slide number, as PowerPoint writes one.
  pub(crate) fn notes(self, body: &str) -> Deck {
    let path = format!("ppt/notesSlides/notesSlide{}.xml", self.slides.len());
    let tree = format!(
      "{}{}{}",
      r#"<p:sp><p:nvSpPr><p:cNvPr id="2" name="Slide Image"/><p:cNvSpPr/><p:nvPr><p:ph type="sldImg"/></p:nvPr></p:nvSpPr><p:spPr/></p:sp>"#,
      shape(r#"<p:ph type="body" idx="1"/>"#, "", body),
      shape(
        r#"<p:ph type="sldNum" sz="quarter" idx="5"/>"#,
        "",
        r#"<a:p><a:fld id="{1}" type="slidenum"><a:t>7</a:t></a:fld></a:p>"#
      ),
    );
    let inner = format!("<p:cSld><p:spTree>{tree}</p:spTree></p:cSld>");
    self
      .xml(&path, "notes", &inner)
      .rel("rIdNotes", "/notesSlide", &path, false)
  }

  /// Reads the deck, or says why it cannot be read.
  pub(crate) fn read(mut self) -> Result<Presentation, PresentationError> {
    let listed: String = (0..self.slides.len())
      .map(|at| format!(r#"<p:sldId id="{}" r:id="rIdSlide{at}"/>"#, 256 + at))
      .collect();
    let inner = format!(
      "<p:sldIdLst>{listed}</p:sldIdLst>{}{}{}",
      self.size, self.defaults, self.extensions
    );
    let slides = std::mem::take(&mut self.slides);
    let mut deck = self.xml(MAIN, "presentation", &inner);
    for (at, slide) in slides.iter().enumerate() {
      deck = deck.link(MAIN, &format!("rIdSlide{at}"), "/slide", slide, false);
    }
    // Every part may be asked for its relationships; one that was given none has none.
    let paths: Vec<String> = deck.parts.parts.keys().cloned().collect();
    for path in paths {
      deck.parts.rels.entry(path).or_default();
    }
    let kind = Kind {
      has_macros: false,
      is_template: false,
      is_show: false,
    };
    assemble(
      &mut deck.parts,
      MAIN,
      DocProps::default(),
      &kind,
      deck.options,
    )
  }

  pub(crate) fn open(self) -> Presentation {
    self.read().unwrap()
  }
}
