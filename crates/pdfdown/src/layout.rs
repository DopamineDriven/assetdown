//! Positioned text: words and lines with boxes, plus where images were painted.
//! The foundation for layout-aware extraction (tables, key/value anchors).

pub(crate) mod fonts;
pub(crate) mod interpreter;
pub(crate) mod standard14;
pub(crate) mod tounicode;
pub(crate) mod words;

use crate::images::get_inherited_resources;
use crate::meta::get_inherited_page_box;
use contentdown::{FontMeta, Word};
use contentdown::{PageLayout, TextSource};
use interpreter::{Interpreter, Matrix};
use lopdf::{Dictionary, Document, Object, ObjectId};
use rayon::prelude::*;

/// Per-page bound on decoded content-stream bytes.
const MAX_CONTENT_BYTES: usize = 64 * 1024 * 1024;

fn inherited_int(doc: &Document, page_id: ObjectId, key: &[u8]) -> Option<i64> {
  let mut current = Some(page_id);
  let mut hops = 0;
  while let Some(id) = current {
    let dict = doc.get_dictionary(id).ok()?;
    if let Ok(v) = dict.get(key) {
      let v = match v {
        Object::Reference(r) => doc.get_object(*r).ok()?,
        other => other,
      };
      if let Object::Integer(i) = v {
        return Some(*i);
      }
    }
    current = dict.get(b"Parent").ok().and_then(|p| match p {
      Object::Reference(r) => Some(*r),
      _ => None,
    });
    hops += 1;
    if hops > 64 {
      break;
    }
  }
  None
}

pub struct PageGeometry {
  pub width: f64,
  pub height: f64,
  pub rotate: u32,
  /// `[x0, y0, x1, y1]` of the CropBox (fallback MediaBox) in user space.
  pub bbox: [f64; 4],
}

pub fn page_geometry(doc: &Document, page_id: ObjectId) -> PageGeometry {
  let bbox = get_inherited_page_box(doc, page_id, b"CropBox")
    .or_else(|| get_inherited_page_box(doc, page_id, b"MediaBox"))
    .unwrap_or([0.0, 0.0, 612.0, 792.0]);
  let rotate = inherited_int(doc, page_id, b"Rotate")
    .map(|r| r.rem_euclid(360) as u32)
    .unwrap_or(0);
  PageGeometry {
    width: (bbox[2] - bbox[0]).abs(),
    height: (bbox[3] - bbox[1]).abs(),
    rotate,
    bbox: [
      bbox[0].min(bbox[2]),
      bbox[1].min(bbox[3]),
      bbox[0].max(bbox[2]),
      bbox[1].max(bbox[3]),
    ],
  }
}

/// Native layout of one page: glyph positions from the content stream.
/// A page's native layout together with the raw words and font metadata the lines were
/// clustered from, so an OCR tier can merge its own words in and re-cluster once.
pub struct NativeLayout {
  pub layout: PageLayout,
  pub words: Vec<Word>,
  pub fonts: Vec<FontMeta>,
  /// How each of `layout.images` was painted, in the same order. An image's entry there is
  /// the box it fills, which does not say which way up it lies in it; this does.
  pub placements: Vec<Placement>,
}

/// How a painted image lies on its page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lie {
  /// The way up it is kept. A hair of skew, a thousandth of a side, is not a turn.
  AsKept,
  /// Turned clockwise by this many degrees: 90, 180 or 270.
  Turned(u32),
  /// Mirrored left to right, and then turned clockwise by this many degrees: 0, 90, 180 or
  /// 270. Its text reads backwards however the page is turned, until the image is flipped.
  Mirrored(u32),
  /// Turned by something that is no quarter turn, sheared, or flattened to nothing.
  Askew,
}

/// The matrix an image was painted through: what carries the image's unit square, across from
/// 0 to 1 and up from 0 to 1 with the image's top at 1, onto the page.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
  matrix: [f64; 6],
}

impl Placement {
  /// A placement by its matrix, `[a b c d e f]` as a PDF writes it.
  pub fn new(matrix: [f64; 6]) -> Self {
    Placement { matrix }
  }

  /// Where a point of the image lands on the page.
  pub fn onto_page(&self, across: f64, up: f64) -> (f64, f64) {
    let [a, b, c, d, e, f] = self.matrix;
    (a * across + c * up + e, b * across + d * up + f)
  }

  /// The placement that lays the image, flipped left to right, where this one lays it as it
  /// is. A mirrored placement of an image is a placement, not mirrored, of its flipped self.
  pub fn of_the_flipped_image(&self) -> Placement {
    let [a, b, c, d, e, f] = self.matrix;
    Placement {
      matrix: [-a, -b, c, d, e + a, f + b],
    }
  }

  /// How the image lies on the page.
  pub fn lie(&self) -> Lie {
    const HAIR: f64 = 1e-3;
    let [a, b, c, d, ..] = self.matrix;
    let (across, up) = (a.hypot(b), c.hypot(d));
    let area = a * d - b * c;
    if !(across > 0.0 && up > 0.0 && area.is_finite() && area != 0.0) {
      return Lie::Askew;
    }
    let mirrored = area < 0.0;
    // A mirrored image's turn is that of its flipped self, whose side across points the other way.
    let (a, b) = match mirrored {
      true => (-a, -b),
      false => (a, b),
    };
    let nothing = |part: f64, side: f64| part.abs() <= HAIR * side;
    let turn = if nothing(b, across) && nothing(c, up) {
      match (a > 0.0, d > 0.0) {
        (true, true) => Some(0),
        (false, false) => Some(180),
        _ => None,
      }
    } else if nothing(a, across) && nothing(d, up) {
      // Turned clockwise a quarter, an image's top points to the right and its right side down.
      match (b < 0.0, c > 0.0) {
        (true, true) => Some(90),
        (false, false) => Some(270),
        _ => None,
      }
    } else {
      None
    };
    match (mirrored, turn) {
      (false, Some(0)) => Lie::AsKept,
      (false, Some(turn)) => Lie::Turned(turn),
      (true, Some(turn)) => Lie::Mirrored(turn),
      (_, None) => Lie::Askew,
    }
  }
}

pub fn native_layout(doc: &Document, page_num: u32, page_id: ObjectId) -> NativeLayout {
  let geo = page_geometry(doc, page_id);
  let mut layout = PageLayout {
    page: page_num,
    width: geo.width,
    height: geo.height,
    rotate: geo.rotate,
    content_rotation: 0,
    source: TextSource::Native,
    words: Vec::new(),
    lines: Vec::new(),
    images: Vec::new(),
    warnings: Vec::new(),
  };

  let content = match doc.get_page_content_with_limit(page_id, MAX_CONTENT_BYTES) {
    Ok(c) => c,
    Err(e) => {
      layout
        .warnings
        .push(format!("page {page_num}: content stream not decoded: {e}"));
      return NativeLayout {
        layout,
        words: Vec::new(),
        fonts: Vec::new(),
        placements: Vec::new(),
      };
    }
  };
  // Resources with page-tree inheritance; an empty dictionary when the page has none.
  let resources: Dictionary = get_inherited_resources(doc, page_id).unwrap_or_default();

  let mut interp = Interpreter::new(doc);
  interp.run(&content, &resources, Matrix::IDENTITY);
  let fonts = interp.font_metas();
  let raw = words::glyphs_to_words(&interp.glyphs);
  let (words, lines) = contentdown::words_to_lines(raw.clone(), &fonts);
  layout.words = words;
  layout.lines = lines;
  layout.images = interp.images;
  layout.warnings.extend(interp.warnings);
  NativeLayout {
    layout,
    words: raw,
    fonts,
    placements: interp.placements,
  }
}

pub fn page_layout(doc: &Document, page_num: u32, page_id: ObjectId) -> PageLayout {
  native_layout(doc, page_num, page_id).layout
}

/// Native layout of every page, in page order.
pub fn extract_layout(doc: &Document) -> Vec<PageLayout> {
  let pages = doc.get_pages();
  let entries: Vec<(u32, ObjectId)> = pages.iter().map(|(&k, &v)| (k, v)).collect();
  let mut out: Vec<PageLayout> = entries
    .par_iter()
    .map(|&(num, id)| page_layout(doc, num, id))
    .collect();
  out.sort_unstable_by_key(|p| p.page);
  out
}

#[cfg(test)]
mod tests;
