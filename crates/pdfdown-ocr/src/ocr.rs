//! The OCR cascade: a page's own text where it has enough, recognized text from its embedded
//! images where it does not, and, with the `render` feature, recognized text from the rasterized
//! page as the last tier.

use contentdown::{FontMeta, PageLayout, PlacedImage, TextSource, Word, words_to_lines};
use image::DynamicImage;
use lopdf::{Document, ObjectId};
#[cfg(feature = "render")]
use ocrdown::recognize_words_upright;
use ocrdown::{
  DEFAULT_SOURCE_PPI, OcrWord, Upright, place_words, pool, recognize_text,
  recognize_words_upright_expecting, source_ppi, unrotate_pixel_box,
};
use pdfdown::PageText;
use pdfdown::images::composite::{InkTone, OcrImages, OneBitComposite, collect_page_ocr_images};
use pdfdown::images::page_path_op_count;
#[cfg(feature = "render")]
use pdfdown::images::{page_has_form_xobjects, page_has_image_xobjects};
use pdfdown::layout::{
  Lie, NativeLayout, PageGeometry, Placement, native_layout, page_geometry, page_layout,
};
#[cfg(feature = "render")]
use pdfdown::render;
use pdfdown::text::{detect_headers_footers, strip_footer_artifacts, text_of_layout};
use rayon::prelude::*;
#[cfg(feature = "render")]
use std::sync::Arc;

use crate::types::{OcrPageText, OcrStructuredPageText};

/// OCR the page's rasters at the resolution their painted size implies: first its one-bit images,
/// merged onto one raster for each tone (they hold a scanned page's text), then its other images
/// one by one. `page_box` is the page's box in user space.
fn ocr_page_images(
  doc: &Document,
  page_id: ObjectId,
  lang: &str,
  placements: &[PlacedImage],
  page_box: [f64; 4],
) -> String {
  let OcrImages { named, composites } = collect_page_ocr_images(doc, page_id, page_box);
  let merged = composites.into_iter().map(|composite| {
    let ppi = source_ppi(
      composite.image.width(),
      composite.bbox[2] - composite.bbox[0],
    );
    recognize_text(&DynamicImage::ImageLuma8(composite.image), lang, ppi)
  });
  let separate = named.into_iter().map(|(name, img)| {
    let ppi = placements
      .iter()
      .find(|p| p.xobject_name == name)
      .map(|p| source_ppi(img.width(), p.x1 - p.x0))
      .unwrap_or(DEFAULT_SOURCE_PPI);
    recognize_text(&img, lang, ppi)
  });
  merged
    .chain(separate)
    .filter(|text| !text.is_empty())
    .collect::<Vec<_>>()
    .join("\n")
}

// The render modes a caller passes. With the `render` feature they are `pdfdown`'s; without it
// they are still accepted as parameters and ignored, so they are defined here.
#[cfg(feature = "render")]
pub use pdfdown::render::{RENDER_MODE_ALWAYS, RENDER_MODE_AUTO, RENDER_MODE_NEVER};

#[cfg(not(feature = "render"))]
pub const RENDER_MODE_AUTO: u8 = 0;
#[cfg(not(feature = "render"))]
pub const RENDER_MODE_NEVER: u8 = 1;
#[cfg(not(feature = "render"))]
pub const RENDER_MODE_ALWAYS: u8 = 2;

/// Extract text with OCR fallback, with optional render tier.
///
/// `render_dpi` and `render_mode` are always accepted as params.
/// When the `render` feature is disabled they are silently ignored.
pub fn extract_text_with_ocr(
  doc: &Document,
  pdf_bytes: &[u8],
  lang: &str,
  min_len: u32,
  max_threads: u32,
  render_dpi: u32,
  render_mode: u8,
) -> Vec<OcrPageText> {
  let pages = doc.get_pages();
  let page_count_str = pages.len().to_string();
  let page_entries: Vec<(u32, ObjectId)> = pages.iter().map(|(&k, &v)| (k, v)).collect();

  let pool = pool(max_threads as usize);

  #[cfg(feature = "render")]
  let pdf_arc: Arc<Vec<u8>> = Arc::new(pdf_bytes.to_vec());
  #[cfg(not(feature = "render"))]
  let _ = (pdf_bytes, render_dpi, render_mode);
  #[cfg(not(feature = "render"))]
  let render_available = false;
  #[cfg(feature = "render")]
  let render_available = render_mode != RENDER_MODE_NEVER;

  #[cfg(feature = "render")]
  let token_count = |s: &str| s.split_whitespace().count();

  let mut results: Vec<OcrPageText> = pool.install(|| {
    page_entries
      .par_iter()
      .map(|&(page_num, page_id)| {
        // Tier 1: the page's own text, as its layout has it; the routing decision is made on
        // the page's geometry.
        let layout = page_layout(doc, page_num, page_id);
        let native = strip_footer_artifacts(&text_of_layout(&layout), &page_count_str);
        let path_ops = page_path_op_count(doc, page_id);
        #[cfg(feature = "render")]
        let outline = path_ops >= OUTLINE_PATH_OPS;
        let geo = page_geometry(doc, page_id);
        let decision = page_needs_ocr(&layout, min_len, path_ops, geo.bbox);

        // Forced rendering replaces the page wholesale, as in the layout cascade; a render that
        // read nothing falls through to the cascade and is not repeated by tier 3.
        #[cfg(feature = "render")]
        let mut rendered_already = false;
        #[cfg(feature = "render")]
        if render_mode == RENDER_MODE_ALWAYS
          && let Some(rendered_text) = try_render_ocr_page(&pdf_arc, page_num, render_dpi, lang)
        {
          rendered_already = true;
          if !rendered_text.is_empty() {
            return OcrPageText {
              page: page_num,
              text: rendered_text,
              source: TextSource::Rendered,
            };
          }
        }

        if decision == OcrDecision::Native {
          return OcrPageText {
            page: page_num,
            text: native,
            source: TextSource::Native,
          };
        }

        // Tier 2: OCR extracted raster images
        #[cfg_attr(not(feature = "render"), allow(unused_mut))]
        let mut text = if decision == OcrDecision::Ocr {
          ocr_page_images(doc, page_id, lang, &layout.images, geo.bbox)
        } else {
          String::new()
        };
        #[cfg_attr(not(feature = "render"), allow(unused_mut))]
        let mut source = TextSource::Ocr;

        // Tier 3: render + OCR (render feature only). The text path carries no word boxes or
        // confidences, so unlike the layout cascade a weak tier 2 is a token count, the render
        // wins by token count rather than `ocr_score`, and the native stamp below is concatenated
        // in front of the OCR text rather than deduplicated against it.
        #[cfg(feature = "render")]
        if render_available && !rendered_already {
          let escalate = decision == OcrDecision::RenderOnly
            || (outline && token_count(&text) < WEAK_TIER2_WORDS)
            || (text.is_empty()
              && (path_ops > 10
                || page_has_form_xobjects(doc, page_id)
                || page_has_image_xobjects(doc, page_id)));
          if escalate
            && let Some(rendered_text) = try_render_ocr_page(&pdf_arc, page_num, render_dpi, lang)
            && token_count(&rendered_text) > token_count(&text)
          {
            text = rendered_text;
            source = TextSource::Rendered;
          }
        }
        let _ = render_available;

        // Hybrid page: the native stamp stays in front of the OCR text.
        let combined = match (native.trim().is_empty(), text.is_empty()) {
          (true, _) => text,
          (false, true) => native,
          (false, false) => format!("{native}\n{text}"),
        };
        OcrPageText {
          page: page_num,
          text: combined,
          source,
        }
      })
      .collect()
  });
  results.sort_unstable_by_key(|r| r.page);
  results
}

/// Render a page to an image and OCR the result.
#[cfg(feature = "render")]
fn try_render_ocr_page(pdf_bytes: &[u8], page_num: u32, dpi: u32, lang: &str) -> Option<String> {
  if !render::is_pdfium_available() {
    return None;
  }
  // page_num is 1-based, PDFium page index is 0-based
  let page_index = (page_num - 1) as u16;
  let img = render::render_page_to_image_from_bytes(pdf_bytes, page_index, dpi)?;
  let text = recognize_text(&img, lang, dpi as i32);
  Some(text)
}

pub fn detect_headers_footers_ocr(pages: &[OcrPageText]) -> Vec<OcrStructuredPageText> {
  let as_page_text: Vec<PageText> = pages
    .iter()
    .map(|p| PageText {
      page: p.page,
      text: p.text.clone(),
    })
    .collect();
  let structured = detect_headers_footers(&as_page_text);
  structured
    .into_iter()
    .zip(pages.iter())
    .map(|(s, ocr)| OcrStructuredPageText {
      page: s.page,
      header: s.header,
      body: s.body,
      footer: s.footer,
      source: ocr.source,
    })
    .collect()
}

// ── Positioned OCR (phase III) ──────────────────────────────────

/// A user-space box re-expressed in the page frame turned `degrees` clockwise about the page
/// box `bbox`: the box origin is kept and, for quarter turns, width and height swap.
fn turn_box(b: [f64; 4], degrees: u32, bbox: [f64; 4]) -> [f64; 4] {
  let [bx0, by0, bx1, by1] = bbox;
  let (w, h) = (bx1 - bx0, by1 - by0);
  let (x0, y0, x1, y1) = (b[0] - bx0, b[1] - by0, b[2] - bx0, b[3] - by0);
  match degrees {
    90 => [bx0 + y0, by0 + (w - x1), bx0 + y1, by0 + (w - x0)],
    270 => [bx0 + (h - y1), by0 + x0, bx0 + (h - y0), by0 + x1],
    180 => [
      bx0 + (w - x1),
      by0 + (h - y1),
      bx0 + (w - x0),
      by0 + (h - y0),
    ],
    _ => b,
  }
}

/// The page's turn is the one carrying the most confident OCR text, summed over the images that
/// read upright at it (`ocr_score` per image), so the tiles of a tiled scan vote together and one
/// tile a retry turned cannot outvote them. Ties keep the turn seen first.
fn dominant_rotation(per_image: &[(u32, f64)]) -> u32 {
  let mut totals: Vec<(u32, f64)> = Vec::new();
  for &(degrees, score) in per_image {
    match totals.iter_mut().find(|(d, _)| *d == degrees) {
      Some((_, total)) => *total += score,
      None => totals.push((degrees, score)),
    }
  }
  totals
    .iter()
    .fold(None::<(u32, f64)>, |best, &(degrees, total)| match best {
      Some((_, best_total)) if best_total >= total => best,
      _ => Some((degrees, total)),
    })
    .map(|(degrees, _)| degrees)
    .unwrap_or(0)
}

/// Express every word and image placement in the page frame turned `degrees` clockwise.
/// Baselines follow the turned box's bottom edge: a turned frame has no font baseline to keep.
fn turn_layout(words: &mut [Word], images: &mut [PlacedImage], degrees: u32, bbox: [f64; 4]) {
  if degrees == 0 {
    return;
  }
  for w in words.iter_mut() {
    let b = turn_box([w.x0, w.y0, w.x1, w.y1], degrees, bbox);
    w.x0 = b[0];
    w.y0 = b[1];
    w.x1 = b[2];
    w.y1 = b[3];
    w.baseline = b[1];
  }
  for i in images.iter_mut() {
    let b = turn_box([i.x0, i.y0, i.x1, i.y1], degrees, bbox);
    i.x0 = b[0];
    i.y0 = b[1];
    i.x1 = b[2];
    i.y1 = b[3];
  }
}

/// Word boxes from a turned raster, mapped back onto the raster as painted.
fn words_in_painted_raster(
  words: Vec<OcrWord>,
  degrees: u32,
  rotated_w: u32,
  rotated_h: u32,
) -> Vec<OcrWord> {
  words
    .into_iter()
    .map(|w| {
      let [left, top, right, bottom] = unrotate_pixel_box(
        [w.left, w.top, w.right, w.bottom],
        degrees,
        rotated_w as i32,
        rotated_h as i32,
      );
      OcrWord {
        left,
        top,
        right,
        bottom,
        ..w
      }
    })
    .collect()
}

/// What one of a page's rasters read as.
struct RasterRead {
  /// What the raster is, as a warning names it: `image Im3`, `14 one-bit images`.
  subject: String,
  /// The words, placed in user space in the frame as painted.
  words: Vec<Word>,
  /// The clockwise turn the raster read upright at.
  degrees: u32,
  score: f64,
}

/// OCR one raster at its best orientation and place its words on `dest`, the user-space
/// rectangle `[x0, y0, x1, y1]` the raster covers as painted. `page_turn` is the clockwise
/// turn the page says it is shown at: a scanner that keeps a sheet as it was fed, on its side,
/// says so there, so the raster is read at that turn first.
fn read_placed_raster(
  subject: String,
  img: &DynamicImage,
  dest: [f64; 4],
  lang: &str,
  page_turn: u32,
  warnings: &mut Vec<String>,
) -> RasterRead {
  let ppi = source_ppi(img.width(), dest[2] - dest[0]);
  let Upright {
    words,
    degrees,
    size: (rw, rh),
    decided,
  } = recognize_words_upright_expecting(img, lang, Some(ppi), page_turn);
  let painted = words_in_painted_raster(words, degrees, rw, rh);
  let words = place_words(painted, img.width(), img.height(), dest);
  // A logo or rule that OCR read as a word or two (fewer than WEAK_TIER2_WORDS) has no
  // orientation worth reporting.
  if degrees != 0 && words.len() >= WEAK_TIER2_WORDS {
    warnings.push(format!(
      "{subject}: {} words read upright after a {degrees}° turn ({})",
      words.len(),
      decided.label()
    ));
  }
  let score = ocr_score(&words);
  RasterRead {
    subject,
    words,
    degrees,
    score,
  }
}

/// The words of a raster set on the page through the placement that painted it: each box is
/// the box, square to the page, that holds the word where it landed.
fn place_through(words: Vec<OcrWord>, width: u32, height: u32, placement: &Placement) -> Vec<Word> {
  // Across the raster and up it, each from 0 to 1, which is what a placement carries.
  let mut placed = place_words(words, width, height, [0.0, 0.0, 1.0, 1.0]);
  for word in placed.iter_mut() {
    let corners = [
      placement.onto_page(word.x0, word.y0),
      placement.onto_page(word.x1, word.y0),
      placement.onto_page(word.x0, word.y1),
      placement.onto_page(word.x1, word.y1),
    ];
    let least =
      |pick: fn(&(f64, f64)) -> f64| corners.iter().map(pick).fold(f64::INFINITY, f64::min);
    let most =
      |pick: fn(&(f64, f64)) -> f64| corners.iter().map(pick).fold(f64::NEG_INFINITY, f64::max);
    word.x0 = least(|corner| corner.0);
    word.y0 = least(|corner| corner.1);
    word.x1 = most(|corner| corner.0);
    word.y1 = most(|corner| corner.1);
    word.baseline = word.y0;
  }
  placed
}

/// OCR one raster that does not lie on the page the way up it is kept, and set its words on
/// the page through the placement that painted it.
///
/// `painted` is the clockwise turn the placement gives the raster, when it is a quarter turn
/// or a half turn. It counts twice. The turn the page says it is shown at is counted from the
/// page as painted, so the raster as it is kept is expected to need that turn and the
/// placement's together. And the turn the raster read at, less the placement's, is the turn
/// the page as painted needs, which is what the raster's vote for the page's turn is. A
/// placement that is askew says nothing of either: nothing is expected, and the raster votes
/// with the turn it read at.
fn read_through_placement(
  subject: String,
  img: &DynamicImage,
  placement: &Placement,
  painted: Option<u32>,
  lang: &str,
  page_turn: u32,
  warnings: &mut Vec<String>,
) -> RasterRead {
  let (from, to) = (placement.onto_page(0.0, 0.0), placement.onto_page(1.0, 0.0));
  let across = (to.0 - from.0).hypot(to.1 - from.1);
  let ppi = source_ppi(img.width(), across);
  let expected = painted.map_or(0, |turn| (page_turn + turn) % 360);
  let Upright {
    words,
    degrees,
    size: (rw, rh),
    decided,
  } = recognize_words_upright_expecting(img, lang, Some(ppi), expected);
  let kept = words_in_painted_raster(words, degrees, rw, rh);
  let words = place_through(kept, img.width(), img.height(), placement);
  if degrees != 0 && words.len() >= WEAK_TIER2_WORDS {
    warnings.push(format!(
      "{subject}: {} words read upright after a {degrees}° turn ({})",
      words.len(),
      decided.label()
    ));
  }
  let score = ocr_score(&words);
  RasterRead {
    subject,
    words,
    degrees: painted.map_or(degrees, |turn| (degrees + 360 - turn % 360) % 360),
    score,
  }
}

/// How a warning names a raster of merged one-bit images.
fn composite_subject(composite: &OneBitComposite) -> String {
  let images = if composite.pieces == 1 {
    "one-bit image"
  } else {
    "one-bit images"
  };
  match composite.tone {
    InkTone::Dark => format!("{} {images}", composite.pieces),
    InkTone::Light => format!("{} {images} painted light", composite.pieces),
  }
}

/// Tier 2: OCR the page's embedded raster images and place each word in the frame as painted.
/// `page_box` is the page's box in user space, `page_turn` the turn the page says it is shown
/// at, and `painted` how each of `placements` was painted. Returns the words and the page's
/// dominant turn; the caller turns the whole page frame by it.
#[allow(clippy::too_many_arguments)]
fn ocr_page_image_words(
  doc: &Document,
  page_id: ObjectId,
  lang: &str,
  placements: &[PlacedImage],
  painted: &[Placement],
  page_box: [f64; 4],
  page_turn: u32,
  warnings: &mut Vec<String>,
) -> (Vec<Word>, u32) {
  let images = collect_page_ocr_images(doc, page_id, page_box);
  read_page_rasters(images, lang, placements, painted, page_turn, warnings)
}

/// Read a page's rasters: its one-bit images together, merged onto one raster for each tone,
/// with their words placed on the area that raster covers; then every other image by itself,
/// placed through its `Do` placement from the native pass. Returns the words in the frame as
/// painted and the page's dominant turn.
///
/// `page_turn`, the turn the page says it is shown at, is counted from the page as painted. A
/// merged raster is the page as painted, so it is read at that turn first. An image is read as
/// it is kept, which is the page as painted only when the painting does not turn it. `painted`
/// says how each of `placements` was painted, in the same order: an image painted turned is
/// read with that turn counted in, and its words are set on the page through its placement;
/// one painted mirrored is flipped first, since no turn makes mirrored text read. A placement
/// with nothing said of it is taken to keep its image as it is.
fn read_page_rasters(
  images: OcrImages,
  lang: &str,
  placements: &[PlacedImage],
  painted: &[Placement],
  page_turn: u32,
  warnings: &mut Vec<String>,
) -> (Vec<Word>, u32) {
  let OcrImages { named, composites } = images;
  let mut reads = Vec::new();
  for composite in composites {
    let subject = composite_subject(&composite);
    let dest = composite.bbox;
    let img = DynamicImage::ImageLuma8(composite.image);
    reads.push(read_placed_raster(
      subject, &img, dest, lang, page_turn, warnings,
    ));
  }
  for (name, img) in named {
    let placed: Vec<(usize, &PlacedImage)> = placements
      .iter()
      .enumerate()
      .filter(|(_, p)| p.xobject_name == name)
      .collect();
    let Some(&(at, first)) = placed.first() else {
      warnings.push(format!(
        "image {name}: no placement found; its OCR words were skipped"
      ));
      continue;
    };
    if placed.len() > 1 {
      warnings.push(format!(
        "image {name}: painted {} times; OCR words use the first placement",
        placed.len()
      ));
    }
    let subject = format!("image {name}");
    let placement = painted.get(at);
    let read = match placement.map(|placement| (placement, placement.lie())) {
      None | Some((_, Lie::AsKept)) => {
        let dest = [first.x0, first.y0, first.x1, first.y1];
        read_placed_raster(subject, &img, dest, lang, page_turn, warnings)
      }
      Some((placement, Lie::Turned(turn))) => read_through_placement(
        subject,
        &img,
        placement,
        Some(turn),
        lang,
        page_turn,
        warnings,
      ),
      Some((placement, Lie::Mirrored(turn))) => read_through_placement(
        subject,
        &img.fliph(),
        &placement.of_the_flipped_image(),
        Some(turn),
        lang,
        page_turn,
        warnings,
      ),
      Some((placement, Lie::Askew)) => {
        read_through_placement(subject, &img, placement, None, lang, page_turn, warnings)
      }
    };
    reads.push(read);
  }
  // A raster that read as a word or two is no evidence of the page's turn; it does not vote.
  let votes: Vec<(u32, f64)> = reads
    .iter()
    .filter(|read| read.words.len() >= WEAK_TIER2_WORDS)
    .map(|read| (read.degrees, read.score))
    .collect();
  let dominant = dominant_rotation(&votes);
  let mut out = Vec::new();
  for read in reads {
    if read.degrees != dominant && read.words.len() >= WEAK_TIER2_WORDS {
      warnings.push(format!(
        "{}: {} words read upright at {}° but the page frame is turned {dominant}°; they read sideways in it",
        read.subject,
        read.words.len(),
        read.degrees
      ));
    }
    out.extend(read.words);
  }
  (out, dominant)
}

/// Tier-3 words from the rendered raster (the page as displayed, `/Rotate` applied, turned
/// `degrees` more to read upright), mapped back onto the page box in the page's own frame, the
/// frame native words use. The content's turn from that frame is `/Rotate` plus `degrees`.
#[cfg_attr(not(feature = "render"), allow(dead_code))]
fn place_rendered_words(
  words: Vec<OcrWord>,
  degrees: u32,
  turned_dims: (u32, u32),
  geo: &PageGeometry,
) -> (Vec<Word>, u32) {
  let quarter = |d: u32| d == 90 || d == 270;
  let (tw, th) = turned_dims;
  // back to the raster as rendered …
  let displayed = words_in_painted_raster(words, degrees, tw, th);
  let (dw, dh) = if quarter(degrees) { (th, tw) } else { (tw, th) };
  // … then to the page as painted, undoing /Rotate
  let painted = words_in_painted_raster(displayed, geo.rotate, dw, dh);
  let (pw, ph) = if quarter(geo.rotate) {
    (dh, dw)
  } else {
    (dw, dh)
  };
  (
    place_words(painted, pw, ph, geo.bbox),
    (geo.rotate + degrees) % 360,
  )
}

/// Tier 3: rasterize the page as displayed and place each OCR word on the page box in the page's
/// own frame; the caller turns the page frame by `rotation`, which includes `/Rotate`.
#[cfg(feature = "render")]
fn render_page_words(
  pdf_bytes: &[u8],
  page_num: u32,
  dpi: u32,
  lang: &str,
  geo: &PageGeometry,
  rotation: &mut u32,
  warnings: &mut Vec<String>,
) -> Option<Vec<Word>> {
  if !render::is_pdfium_available() {
    return None;
  }
  let page_index = (page_num - 1) as u16;
  let img = render::render_page_to_image_from_bytes(pdf_bytes, page_index, dpi)?;
  let Upright {
    words,
    degrees,
    size: turned_dims,
    decided,
  } = recognize_words_upright(&img, lang, Some(dpi as i32));
  if degrees != 0 {
    warnings.push(format!(
      "page {page_num}: text reads upright after a {degrees}° turn ({})",
      decided.label()
    ));
  }
  let (placed, content_rotation) = place_rendered_words(words, degrees, turned_dims, geo);
  *rotation = content_rotation;
  Some(placed)
}

fn finish_ocr_layout(
  mut layout: PageLayout,
  words: Vec<Word>,
  fonts: &[FontMeta],
  source: TextSource,
  rotation: u32,
) -> PageLayout {
  let (words, lines) = words_to_lines(words, fonts);
  layout.words = words;
  layout.lines = lines;
  layout.source = source;
  layout.content_rotation = rotation;
  if rotation == 90 || rotation == 270 {
    std::mem::swap(&mut layout.width, &mut layout.height);
  }
  layout
}

/// Where a page goes in the OCR cascade.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OcrDecision {
  /// Native glyphs are the page: no OCR.
  Native,
  /// OCR the embedded rasters (tier 2), escalating to the render tier when weak.
  Ocr,
  /// No glyphs and no rasters but painted paths ("print as paths"): only the render tier can read it.
  RenderOnly,
}

/// Native words covering less than this share of the page do not prove the page readable…
const OCR_WORD_COVERAGE_MAX: f64 = 0.02;
/// …when image placements cover more than this share: that is a scan with a stamp on it.
const OCR_IMAGE_COVERAGE_MIN: f64 = 0.5;
/// Pages carrying at least this many path operators are text printed as outlines (or dense
/// drawings): not readable from glyphs, and only the render tier can read the paths.
const OUTLINE_PATH_OPS: usize = 200;
/// A tier-2 result this thin on a page with painted paths is escalated to the render tier.
const WEAK_TIER2_WORDS: usize = 3;
const WEAK_TIER2_CONFIDENCE: f64 = 0.3;

fn clipped_area(b: [f64; 4], bbox: [f64; 4]) -> f64 {
  let w = b[2].min(bbox[2]) - b[0].max(bbox[0]);
  let h = b[3].min(bbox[3]) - b[1].max(bbox[1]);
  if w <= 0.0 || h <= 0.0 { 0.0 } else { w * h }
}

/// Share of the page box `bbox` covered by `boxes` (clipped to it; overlaps count twice). Boxes are
/// raw user space, so a CropBox away from the origin must be clipped against, not an origin box.
fn coverage(boxes: impl Iterator<Item = [f64; 4]>, bbox: [f64; 4]) -> f64 {
  let page = ((bbox[2] - bbox[0]) * (bbox[3] - bbox[1])).max(1.0);
  boxes.map(|b| clipped_area(b, bbox)).sum::<f64>() / page
}

/// Decide by coverage, not by character count alone: a scanned page with a few stamped native
/// characters is still a scan, an outline-text page with a logo and a stamp is not readable
/// natively, and a sparse cover sheet without rasters or heavy paths is still native.
pub(crate) fn page_needs_ocr(
  native: &PageLayout,
  min_len: u32,
  path_ops: usize,
  bbox: [f64; 4],
) -> OcrDecision {
  let outline = path_ops >= OUTLINE_PATH_OPS;
  if native.words.is_empty() && native.images.is_empty() {
    return if outline {
      OcrDecision::RenderOnly
    } else {
      OcrDecision::Ocr
    };
  }
  if non_ws_chars(native) < min_len as usize {
    return OcrDecision::Ocr;
  }
  let words = coverage(native.words.iter().map(|w| [w.x0, w.y0, w.x1, w.y1]), bbox);
  let images = coverage(native.images.iter().map(|i| [i.x0, i.y0, i.x1, i.y1]), bbox).min(1.0);
  if words < OCR_WORD_COVERAGE_MAX && (images > OCR_IMAGE_COVERAGE_MIN || outline) {
    OcrDecision::Ocr
  } else {
    OcrDecision::Native
  }
}

fn intersection_over_union(a: &Word, b: &Word) -> f64 {
  let ix = (a.x1.min(b.x1) - a.x0.max(b.x0)).max(0.0);
  let iy = (a.y1.min(b.y1) - a.y0.max(b.y0)).max(0.0);
  let inter = ix * iy;
  let union = (a.x1 - a.x0) * (a.y1 - a.y0) + (b.x1 - b.x0) * (b.y1 - b.y0) - inter;
  if union <= 0.0 { 0.0 } else { inter / union }
}

/// Native words first, then every OCR word that does not sit on a native word (IoU > 0.5 is the
/// same word read twice). Returns the merged words and how many OCR words were dropped.
pub(crate) fn merge_native_and_ocr(native: Vec<Word>, ocr: Vec<Word>) -> (Vec<Word>, usize) {
  let mut out = native;
  let native_count = out.len();
  let mut dropped = 0usize;
  for w in ocr {
    if out[..native_count]
      .iter()
      .any(|n| intersection_over_union(n, &w) > 0.5)
    {
      dropped += 1;
    } else {
      out.push(w);
    }
  }
  (out, dropped)
}

fn ocr_score(words: &[Word]) -> f64 {
  if words.is_empty() {
    return 0.0;
  }
  let mean = words
    .iter()
    .map(|w| w.confidence.unwrap_or(0.0))
    .sum::<f64>()
    / words.len() as f64;
  words.len() as f64 * mean
}

fn tier2_is_weak(words: &[Word]) -> bool {
  if words.len() < WEAK_TIER2_WORDS {
    return true;
  }
  let mean = words
    .iter()
    .map(|w| w.confidence.unwrap_or(0.0))
    .sum::<f64>()
    / words.len() as f64;
  mean < WEAK_TIER2_CONFIDENCE
}

fn non_ws_chars(layout: &PageLayout) -> usize {
  layout
    .words
    .iter()
    .map(|w| w.text.chars().filter(|c| !c.is_whitespace()).count())
    .sum()
}

/// Positioned text with the same three-tier cascade as [`extract_text_with_ocr`]:
/// native glyph placement, then OCR of embedded images mapped through their
/// placements, then (with `render`) OCR of the rasterized page.
/// `only_pages` lists the 1-based pages the cascade may OCR; pages off the list come back as
/// their native layout untouched, and `None` means every page.
#[allow(clippy::too_many_arguments)]
pub fn extract_layout_with_ocr(
  doc: &Document,
  pdf_bytes: &[u8],
  lang: &str,
  min_len: u32,
  max_threads: u32,
  render_dpi: u32,
  render_mode: u8,
  only_pages: Option<&[u32]>,
) -> Vec<PageLayout> {
  let pages = doc.get_pages();
  let page_entries: Vec<(u32, ObjectId)> = pages.iter().map(|(&k, &v)| (k, v)).collect();
  let pool = pool(max_threads as usize);

  #[cfg(feature = "render")]
  let pdf_arc: Arc<Vec<u8>> = Arc::new(pdf_bytes.to_vec());
  #[cfg(not(feature = "render"))]
  let _ = (pdf_bytes, render_dpi, render_mode);
  #[cfg(not(feature = "render"))]
  let render_available = false;
  #[cfg(feature = "render")]
  let render_available = render_mode != RENDER_MODE_NEVER;

  // One page of the document. `handed` is the turn the page before it read at, when the pages
  // are read in order and that page had something to say: a page the file says nothing of is
  // expected to need the same turn, as the next sheet of a scan fed in sideways does.
  let read_page = |page_num: u32, page_id: ObjectId, handed: u32| -> PageLayout {
    if let Some(list) = only_pages
      && !list.contains(&page_num)
    {
      return native_layout(doc, page_num, page_id).layout;
    }
    // Tier 1: native glyph placement (also yields image placements for tier 2).
    let NativeLayout {
      layout: native,
      words: native_words,
      fonts,
      placements,
    } = native_layout(doc, page_num, page_id);
    let mut warnings = Vec::new();
    let mut rotation = 0u32;
    let path_ops = page_path_op_count(doc, page_id);
    let outline = path_ops >= OUTLINE_PATH_OPS;

    let geo = page_geometry(doc, page_id);
    let mut native = native;

    // Forced rendering replaces the page wholesale; the hybrid merge below is for the cascade.
    // A render that read nothing leaves no trace and is not repeated by tier 3.
    #[cfg(feature = "render")]
    let mut rendered_already = false;
    #[cfg(feature = "render")]
    if render_mode == RENDER_MODE_ALWAYS {
      let mut forced_rotation = 0u32;
      let mut forced_warnings = Vec::new();
      if let Some(mut words) = render_page_words(
        &pdf_arc,
        page_num,
        render_dpi,
        lang,
        &geo,
        &mut forced_rotation,
        &mut forced_warnings,
      ) {
        rendered_already = true;
        if !words.is_empty() {
          warnings.extend(forced_warnings);
          turn_layout(&mut words, &mut native.images, forced_rotation, geo.bbox);
          let mut out =
            finish_ocr_layout(native, words, &[], TextSource::Rendered, forced_rotation);
          out.warnings.extend(warnings);
          return out;
        }
        warnings.push(format!(
          "page {page_num}: forced render read no words; the cascade decided the page"
        ));
      }
    }

    let decision = page_needs_ocr(&native, min_len, path_ops, geo.bbox);
    if decision == OcrDecision::Native {
      return native;
    }

    // Tier 2: OCR embedded raster images.
    #[cfg_attr(not(feature = "render"), allow(unused_mut))]
    let mut ocr_words = if decision == OcrDecision::Ocr {
      let (words, dominant) = ocr_page_image_words(
        doc,
        page_id,
        lang,
        &native.images,
        &placements,
        geo.bbox,
        // What the page says of itself comes before what the page before it suggests.
        match geo.rotate {
          0 => handed,
          said => said,
        },
        &mut warnings,
      );
      rotation = dominant;
      words
    } else {
      Vec::new()
    };
    #[cfg_attr(not(feature = "render"), allow(unused_mut))]
    let mut source = TextSource::Ocr;

    // Tier 3: render + OCR for outline text, weak tier-2 results on path-heavy pages, and
    // rasters tier 2 could not decode.
    #[cfg(feature = "render")]
    if render_available {
      let escalate = decision == OcrDecision::RenderOnly
        || (outline && tier2_is_weak(&ocr_words))
        || (ocr_words.is_empty()
          && (path_ops > 10
            || page_has_form_xobjects(doc, page_id)
            || page_has_image_xobjects(doc, page_id)));
      if escalate && !rendered_already {
        let mut render_rotation = 0u32;
        let mut render_warnings = Vec::new();
        if let Some(rendered) = render_page_words(
          &pdf_arc,
          page_num,
          render_dpi,
          lang,
          &geo,
          &mut render_rotation,
          &mut render_warnings,
        ) && ocr_score(&rendered) > ocr_score(&ocr_words)
        {
          if !ocr_words.is_empty() {
            warnings.push(format!(
              "page {page_num}: rendered page replaced {} weak tier-2 words",
              ocr_words.len()
            ));
          }
          ocr_words = rendered;
          rotation = render_rotation;
          warnings.extend(render_warnings);
          source = TextSource::Rendered;
        }
      }
    }
    if outline
      && !render_available
      && (decision == OcrDecision::RenderOnly || tier2_is_weak(&ocr_words))
    {
      warnings.push(format!(
            "page {page_num}: {path_ops} path operators paint text as outlines; only the render tier can read them"
          ));
    }

    // Hybrid page: native words are real evidence and stay; OCR adds what they do not cover.
    let native_count = native_words.len();
    let (merged, dropped) = merge_native_and_ocr(native_words, ocr_words);
    if merged.is_empty() {
      warnings.push(format!("page {page_num}: OCR found no words"));
    }
    if native_count > 0 {
      if merged.len() > native_count {
        warnings.push(format!(
              "page {page_num}: {native_count} native words kept alongside OCR ({dropped} duplicates dropped)"
            ));
      } else {
        warnings.push(format!(
          "page {page_num}: OCR found no new words; {native_count} native words kept"
        ));
      }
    }
    let mut merged = merged;
    turn_layout(&mut merged, &mut native.images, rotation, geo.bbox);
    let mut out = finish_ocr_layout(native, merged, &fonts, source, rotation);
    out.warnings.extend(warnings);
    out
  };
  let mut results: Vec<PageLayout> = match max_threads {
    // One page at a time is pages in their order, so each can hand its turn to the next. This
    // is how a batch reads: a process to a core, each with one thread.
    1 => {
      let mut handed = 0;
      page_entries
        .iter()
        .map(|&(page_num, page_id)| {
          let page = read_page(page_num, page_id, handed);
          handed = turn_to_hand_on(&page);
          page
        })
        .collect()
    }
    // Several at once finish in no fixed order, and a page that waited on the one before it
    // would not be read at once: nothing is handed on, so that what comes out is the same
    // however the pages were scheduled.
    _ => pool.install(|| {
      page_entries
        .par_iter()
        .map(|&(page_num, page_id)| read_page(page_num, page_id, 0))
        .collect()
    }),
  };
  results.sort_unstable_by_key(|r| r.page);
  results
}

/// A page says how the sheets were fed when this many words of it read at this mean
/// confidence: the bar a reading has to clear to stand without a search.
const HAND_ON_WORDS: usize = 10;
const HAND_ON_CONFIDENCE: f64 = 0.8;

/// The turn a page hands to the page after it: the turn it read at, when it was read from the
/// pictures kept in it and read well; and none otherwise. A page that read well the way it is
/// hands on that nothing is expected, which ends a run of turned pages where it ends.
fn turn_to_hand_on(page: &PageLayout) -> u32 {
  let sure: Vec<f64> = page
    .words
    .iter()
    .filter_map(|word| word.confidence)
    .collect();
  let read_well = page.source == TextSource::Ocr
    && sure.len() >= HAND_ON_WORDS
    && sure.iter().sum::<f64>() / sure.len() as f64 >= HAND_ON_CONFIDENCE;
  match read_well {
    true => page.content_rotation,
    false => 0,
  }
}

#[cfg(test)]
mod cascade_tests;

#[cfg(test)]
mod composite_tests;

#[cfg(test)]
mod page_filter_tests;

#[cfg(test)]
mod frame_tests;

#[cfg(test)]
mod placement_tests;
