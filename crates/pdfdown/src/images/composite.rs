//! One raster for a page's one-bit images.
//!
//! Copiers and scanning software save a page as mixed raster content: its text and line art go
//! into one-bit stencil masks, one for each ink color, painted over a picture of whatever is
//! left. Such a page carries from a handful of masks to dozens, many of them no larger than a
//! word. Read one at a time, each mask costs an OCR pass of its own, and a word that lies across
//! two masks comes back as two fragments.
//!
//! This module paints those images where the content stream puts them, on a raster covering the
//! area they occupy, so that the OCR tier reads them together. Every image's ink becomes black on
//! white whatever color the page paints it in, as it does when the image is decoded alone. Masks
//! painted in a light color go on a second raster: light ink is legible only against something
//! dark, and when that is another mask (a dark bar with its lettering cut out, the lettering
//! painted white into the gaps) the two would fuse into one black shape on a shared raster.

use super::{PageImageSource, get_inherited_resources, get_page_content_bytes, page_image_sources};
use crate::layout::interpreter::{Matrix, bbox_of, deref, number};
use image::{DynamicImage, GrayImage, Luma};
use lopdf::content::Content;
use lopdf::{Dictionary, Document, Object, ObjectId};

const INK: u8 = 0;
const PAPER: u8 = 255;
/// A decoded one-bit image holds only the two values above; this separates them.
const INK_BELOW: u8 = 128;
/// A stencil mask filled with a gray level at or above this is painted light.
const LIGHT_FILL: f64 = 0.5;
/// A page that paints fewer one-bit images than this has nothing to merge.
const MIN_PIECES: usize = 2;
/// The largest raster painted, in pixels. An A3 sheet at 600 ppi (69.7 million) fits; a larger
/// area is painted at a lower resolution.
const MAX_COMPOSITE_PIXELS: f64 = 72_000_000.0;

/// Which of a page's two rasters a one-bit image is painted on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InkTone {
  /// Stencil masks filled with a dark color, and one-bit pictures.
  Dark,
  /// Stencil masks filled with a light color.
  Light,
}

/// A page's one-bit images of one tone, painted where the page paints them.
#[derive(Debug, Clone)]
pub struct OneBitComposite {
  /// Black ink on white paper.
  pub image: GrayImage,
  /// `[x0, y0, x1, y1]` of the area the raster covers, in user space (y up).
  pub bbox: [f64; 4],
  pub tone: InkTone,
  /// How many times an image was painted onto the raster.
  pub pieces: usize,
}

/// A page's decoded images as the OCR tier reads them.
#[derive(Debug, Clone, Default)]
pub struct OcrImages {
  /// Images read one at a time, with their XObject resource names.
  pub named: Vec<(String, DynamicImage)>,
  /// The page's one-bit images, merged: none when the page paints fewer than two, otherwise one
  /// for each tone it paints them in.
  pub composites: Vec<OneBitComposite>,
}

/// Decode the images a page paints, merging its one-bit images into one raster for each tone
/// when it paints two or more of them directly (images inside form XObjects are not merged).
/// `clip` is the page box, `[x0, y0, x1, y1]` in user space: nothing is painted outside it.
///
/// A merged image is not also returned by name. That includes one the page paints only outside
/// `clip` or through a matrix that flattens it, which shows nothing on the page.
pub fn collect_page_ocr_images(doc: &Document, page_id: ObjectId, clip: [f64; 4]) -> OcrImages {
  let sources = page_image_sources(doc, page_id);
  let paints = one_bit_paints(doc, page_id, &sources);
  let merging = paints.len() >= MIN_PIECES;
  // Each painted one-bit image is decoded once, however often it is painted.
  let decoded: Vec<Option<GrayImage>> = sources
    .iter()
    .enumerate()
    .map(|(at, source)| {
      if merging && paints.iter().any(|paint| paint.source == at) {
        source.decode().map(DynamicImage::into_luma8)
      } else {
        None
      }
    })
    .collect();
  let pieces: Vec<Piece<'_>> = paints
    .iter()
    .filter_map(|paint| {
      Some(Piece {
        image: decoded.get(paint.source)?.as_ref()?,
        ctm: paint.ctm,
        tone: paint.tone,
      })
    })
    .collect();
  let composites = if pieces.len() >= MIN_PIECES {
    paint_composites(&pieces, clip)
  } else {
    Vec::new()
  };
  let merged = !composites.is_empty();
  let named = sources
    .iter()
    .zip(decoded)
    .filter_map(|(source, decoded)| match decoded {
      Some(_) if merged => None,
      // Decoded for a raster that did not come about.
      Some(image) => Some((source.name.clone(), DynamicImage::ImageLuma8(image))),
      None => Some((source.name.clone(), source.decode()?)),
    })
    .collect();
  OcrImages { named, composites }
}

/// One `Do` of a one-bit image in a page's content stream.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Paint {
  /// Index of the image among the page's image sources.
  source: usize,
  ctm: Matrix,
  tone: InkTone,
}

/// The nonstroking color, as far as it tells a dark fill from a light one.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Fill {
  /// A gray level from 0 (black) to 1 (white), in a color space whose components are gray, RGB
  /// or CMYK values.
  Gray(f64),
  /// A color in a space whose components say nothing by themselves: a spot ink, a palette index,
  /// a pattern.
  Unreadable,
}

#[derive(Debug, Clone, Copy)]
struct GraphicsState {
  ctm: Matrix,
  fill: Fill,
}

/// The gray level of one, three or four color components read as gray, RGB or CMYK, by the
/// conversions between the device color spaces (ISO 32000-1 §10.3).
fn gray_level(components: &[Object]) -> Option<f64> {
  let values: Vec<f64> = components.iter().map(number).collect::<Option<_>>()?;
  let of_rgb = |r: f64, g: f64, b: f64| 0.3 * r + 0.59 * g + 0.11 * b;
  match values.as_slice() {
    [gray] => Some(*gray),
    [r, g, b] => Some(of_rgb(*r, *g, *b)),
    [c, m, y, k] => Some(of_rgb(
      1.0 - (c + k).min(1.0),
      1.0 - (m + k).min(1.0),
      1.0 - (y + k).min(1.0),
    )),
    _ => None,
  }
}

/// Whether the components of the color space a `cs` operator names are gray, RGB or CMYK values,
/// which their count tells apart (ISO 32000-1 §8.6). The spaces that are not: `Lab`, `Indexed`,
/// `Separation`, `DeviceN` and `Pattern`.
fn components_are_levels(doc: &Document, resources: Option<&Dictionary>, name: &[u8]) -> bool {
  let is_device = |n: &[u8]| matches!(n, b"DeviceGray" | b"DeviceRGB" | b"DeviceCMYK");
  if is_device(name) {
    return true;
  }
  let space = resources
    .and_then(|r| r.get(b"ColorSpace").ok())
    .and_then(|o| deref(doc, o))
    .and_then(|o| o.as_dict().ok())
    .and_then(|spaces| spaces.get(name).ok())
    .and_then(|o| deref(doc, o));
  match space {
    Some(Object::Name(n)) => is_device(n),
    Some(Object::Array(family)) => match family.first().and_then(|o| deref(doc, o)) {
      Some(Object::Name(n)) => {
        is_device(n) || matches!(n.as_slice(), b"ICCBased" | b"CalGray" | b"CalRGB")
      }
      _ => false,
    },
    _ => false,
  }
}

/// The raster a one-bit image is painted on: a stencil mask by the color that fills it, a
/// picture as dark, since its ink shows black.
fn tone_of(doc: &Document, source: &PageImageSource<'_>, fill: Fill) -> InkTone {
  let is_mask = matches!(
    source
      .stream
      .dict
      .get(b"ImageMask")
      .ok()
      .and_then(|o| deref(doc, o)),
    Some(Object::Boolean(true))
  );
  match fill {
    Fill::Gray(level) if is_mask && level >= LIGHT_FILL => InkTone::Light,
    _ => InkTone::Dark,
  }
}

/// Every `Do` of a one-bit image in the page's own content stream, in paint order, with the
/// matrix and fill color in force. Form XObjects are not entered.
fn one_bit_paints(
  doc: &Document,
  page_id: ObjectId,
  sources: &[PageImageSource<'_>],
) -> Vec<Paint> {
  if !sources.iter().any(PageImageSource::is_one_bit) {
    return Vec::new();
  }
  let Ok(content) = Content::decode(&get_page_content_bytes(doc, page_id)) else {
    return Vec::new();
  };
  let resources = get_inherited_resources(doc, page_id);
  // The initial color is black in DeviceGray (ISO 32000-1 §8.6.4.2).
  let mut state = GraphicsState {
    ctm: Matrix::IDENTITY,
    fill: Fill::Gray(0.0),
  };
  let mut saved = Vec::new();
  let mut paints = Vec::new();
  for op in &content.operations {
    let operands = &op.operands;
    match op.operator.as_str() {
      "q" => saved.push(state),
      "Q" => {
        if let Some(restored) = saved.pop() {
          state = restored;
        }
      }
      "cm" => {
        if let Some(m) = Matrix::from_operands(operands) {
          state.ctm = m.mul(state.ctm);
        }
      }
      "g" | "rg" | "k" => {
        if let Some(level) = gray_level(operands) {
          state.fill = Fill::Gray(level);
        }
      }
      "cs" => {
        if let Some(Object::Name(name)) = operands.first() {
          // Selecting a space also selects its initial color, which is black or near it.
          state.fill = if components_are_levels(doc, resources.as_ref(), name) {
            Fill::Gray(0.0)
          } else {
            Fill::Unreadable
          };
        }
      }
      "sc" | "scn" => {
        if let (Fill::Gray(_), Some(level)) = (state.fill, gray_level(operands)) {
          state.fill = Fill::Gray(level);
        }
      }
      "Do" => {
        if let Some(Object::Name(name)) = operands.first() {
          let name = String::from_utf8_lossy(name);
          let painted = sources
            .iter()
            .enumerate()
            .find(|(_, source)| source.is_one_bit() && source.name == name);
          if let Some((at, source)) = painted {
            paints.push(Paint {
              source: at,
              ctm: state.ctm,
              tone: tone_of(doc, source, state.fill),
            });
          }
        }
      }
      _ => {}
    }
  }
  paints
}

/// A decoded one-bit image and where one `Do` paints it.
struct Piece<'a> {
  /// Black ink on white paper.
  image: &'a GrayImage,
  ctm: Matrix,
  tone: InkTone,
}

/// A piece that shows inside the clip.
struct Placed<'a> {
  image: &'a GrayImage,
  /// User space to the unit square the image fills.
  to_unit: Matrix,
  /// The piece's box in user space, cut to the clip.
  bbox: [f64; 4],
  /// Pixels per point of user space along the finer of the image's two sides as painted.
  density: f64,
}

impl<'a> Placed<'a> {
  fn new(piece: &Piece<'a>, clip: [f64; 4]) -> Option<Self> {
    let to_unit = piece.ctm.inverse()?;
    let (x0, y0, x1, y1) = bbox_of(piece.ctm, 0.0, 0.0, 1.0, 1.0);
    let bbox = [
      x0.max(clip[0]),
      y0.max(clip[1]),
      x1.min(clip[2]),
      y1.min(clip[3]),
    ];
    if !(bbox[0] < bbox[2] && bbox[1] < bbox[3]) {
      return None;
    }
    let across = f64::from(piece.image.width()) / piece.ctm.a.hypot(piece.ctm.b);
    let down = f64::from(piece.image.height()) / piece.ctm.c.hypot(piece.ctm.d);
    let density = across.max(down);
    (density.is_finite() && density > 0.0).then_some(Placed {
      image: piece.image,
      to_unit,
      bbox,
      density,
    })
  }

  /// Paint the piece's ink onto `canvas`, which covers `frame` at `density` pixels per point.
  /// Each canvas pixel takes the image pixel under its center.
  fn paint(&self, canvas: &mut GrayImage, frame: [f64; 4], density: f64) {
    let (width, height) = canvas.dimensions();
    let (image_width, image_height) = self.image.dimensions();
    let (across, down) = (f64::from(image_width), f64::from(image_height));
    let columns = within((self.bbox[0] - frame[0]) * density, width)
      ..within(((self.bbox[2] - frame[0]) * density).ceil(), width);
    let rows = within((frame[3] - self.bbox[3]) * density, height)
      ..within(((frame[3] - self.bbox[1]) * density).ceil(), height);
    for row in rows {
      let y = frame[3] - (f64::from(row) + 0.5) / density;
      for column in columns.clone() {
        let x = frame[0] + (f64::from(column) + 0.5) / density;
        let (u, v) = self.to_unit.apply(x, y);
        // An image's first row is the top of the unit square.
        let (sx, sy) = ((u * across).floor(), ((1.0 - v) * down).floor());
        if sx >= 0.0
          && sy >= 0.0
          && sx < across
          && sy < down
          && self.image.get_pixel(sx as u32, sy as u32).0[0] < INK_BELOW
        {
          canvas.put_pixel(column, row, Luma([INK]));
        }
      }
    }
  }
}

/// A raster coordinate held inside `0..=limit`. The value is clamped before the cast, and a NaN
/// casts to zero.
fn within(v: f64, limit: u32) -> u32 {
  v.clamp(0.0, f64::from(limit)) as u32
}

/// The density at or under which half of the pieces' pixels lie. One image painted very small
/// among the others does not set the raster's size.
fn median_density(placed: &[Placed<'_>]) -> Option<f64> {
  let mut by_density: Vec<(f64, u64)> = placed
    .iter()
    .map(|p| {
      let (width, height) = p.image.dimensions();
      (p.density, u64::from(width) * u64::from(height))
    })
    .collect();
  by_density.sort_by(|a, b| a.0.total_cmp(&b.0));
  let total: u64 = by_density.iter().map(|(_, pixels)| pixels).sum();
  let mut seen = 0u64;
  by_density
    .into_iter()
    .find(|(_, pixels)| {
      seen += pixels;
      seen.saturating_mul(2) >= total
    })
    .map(|(density, _)| density)
}

/// The raster's width and height for `frame` at `density` pixels per point, with the density
/// lowered where it would take more than [`MAX_COMPOSITE_PIXELS`]. Returns the density used.
fn raster_size(frame: [f64; 4], density: f64) -> Option<(u32, u32, f64)> {
  let (width, height) = (frame[2] - frame[0], frame[3] - frame[1]);
  let wanted = width * density * height * density;
  if !wanted.is_finite() || wanted <= 0.0 {
    return None;
  }
  let density = if wanted > MAX_COMPOSITE_PIXELS {
    density * (MAX_COMPOSITE_PIXELS / wanted).sqrt()
  } else {
    density
  };
  // At least one pixel a side, and no side longer than the pixel limit.
  let side = |points: f64| within((points * density).ceil().max(1.0), u32::MAX);
  Some((side(width), side(height), density))
}

/// One raster of the pieces of one tone, or `None` when none of them shows inside the clip.
fn paint_tone(pieces: &[Piece<'_>], tone: InkTone, clip: [f64; 4]) -> Option<OneBitComposite> {
  let placed: Vec<Placed<'_>> = pieces
    .iter()
    .filter(|piece| piece.tone == tone)
    .filter_map(|piece| Placed::new(piece, clip))
    .collect();
  let frame = placed.iter().map(|p| p.bbox).reduce(|a, b| {
    [
      a[0].min(b[0]),
      a[1].min(b[1]),
      a[2].max(b[2]),
      a[3].max(b[3]),
    ]
  })?;
  let (width, height, density) = raster_size(frame, median_density(&placed)?)?;
  let mut canvas = GrayImage::from_pixel(width, height, Luma([PAPER]));
  for piece in &placed {
    piece.paint(&mut canvas, frame, density);
  }
  Some(OneBitComposite {
    image: canvas,
    bbox: frame,
    tone,
    pieces: placed.len(),
  })
}

/// The dark raster, then the light one, for whichever tones the pieces have.
fn paint_composites(pieces: &[Piece<'_>], clip: [f64; 4]) -> Vec<OneBitComposite> {
  [InkTone::Dark, InkTone::Light]
    .into_iter()
    .filter_map(|tone| paint_tone(pieces, tone, clip))
    .collect()
}

#[cfg(test)]
mod tests;
