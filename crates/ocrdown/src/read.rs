//! Reading a picture: its text, or its words with where each one is.

use contentdown::Word;
use image::DynamicImage;

use crate::engine::tessdata_prefix;
use crate::resolution::{DEFAULT_SOURCE_PPI, set_resolution};
use crate::turn::{
  OrientationSource, ROTATION_RETRY_CONFIDENCE, ROTATIONS, osd_rotation, pick_first_rotation,
  rotate_image, upright_is_convincing,
};

/// One Tesseract word in image pixel space (origin top-left).
#[derive(Debug, Clone, PartialEq)]
pub struct OcrWord {
  pub text: String,
  pub left: i32,
  pub top: i32,
  pub right: i32,
  pub bottom: i32,
  /// How sure Tesseract is of the word, from 0 to 100.
  pub confidence: f32,
}

fn mean_confidence(words: &[OcrWord]) -> f32 {
  if words.is_empty() {
    0.0
  } else {
    words.iter().map(|w| w.confidence).sum::<f32>() / words.len() as f32
  }
}

/// One OCR pass over the raster at a given turn.
struct OrientedPass {
  words: Vec<OcrWord>,
  degrees: u32,
  dims: (u32, u32),
  confidence: f32,
  source: OrientationSource,
}

impl OrientedPass {
  fn upright(self) -> Upright {
    Upright {
      words: self.words,
      degrees: self.degrees,
      size: self.dims,
      decided: self.source,
    }
  }
}

/// What a picture reads as, the way up its text reads.
#[derive(Debug, Clone, PartialEq)]
pub struct Upright {
  /// The words, with their boxes in pixels of the picture once it is turned.
  pub words: Vec<OcrWord>,
  /// How far the picture was turned, clockwise: 0, 90, 180 or 270.
  pub degrees: u32,
  /// The width and height of the picture once it is turned.
  pub size: (u32, u32),
  /// Who decided the turn.
  pub decided: OrientationSource,
}

/// Plain-text OCR of one image plus Tesseract's mean confidence (0..100).
fn image_text(img: &DynamicImage, lang: &str, ppi: i32) -> (String, f32) {
  let datapath = tessdata_prefix().unwrap_or("");
  let rgb = img.to_rgb8();
  let (w, h) = rgb.dimensions();
  let pixels = rgb.as_raw();

  let tess = tesseract_rs::TesseractAPI::new();
  if tess.init(datapath, lang).is_err()
    || tess
      .set_image(pixels, w as i32, h as i32, 3, (w * 3) as i32)
      .is_err()
  {
    return (String::new(), 0.0);
  }
  set_resolution(&tess, ppi);
  let text = tess
    .get_utf8_text()
    .map(|t| t.trim().to_string())
    .unwrap_or_default();
  let conf = tess.mean_text_conf().unwrap_or(0) as f32;
  (text, conf)
}

/// OCR a single DynamicImage and return extracted text. The OSD detector names the turn first when
/// confident; otherwise the upright pass and the retry over the other turns decide.
pub fn recognize_text(img: &DynamicImage, lang: &str, ppi: i32) -> String {
  let datapath = tessdata_prefix().unwrap_or("");
  let mut tried: Vec<u32> = Vec::new();
  let mut best: Option<(String, f32)> = None;
  let mut consider = |degrees: u32, tried: &mut Vec<u32>| -> (bool, bool) {
    tried.push(degrees);
    let candidate = if degrees == 0 {
      image_text(img, lang, ppi)
    } else {
      image_text(&rotate_image(img, degrees), lang, ppi)
    };
    if best.as_ref().is_none_or(|b| candidate.1 > b.1) {
      best = Some(candidate);
    }
    let b = best.as_ref().expect("a pass was recorded");
    (
      !b.0.is_empty() && b.1 >= ROTATION_RETRY_CONFIDENCE,
      upright_is_convincing(b.0.split_whitespace().count(), b.1),
    )
  };
  let (upright_ok, convincing) = consider(0, &mut tried);
  if convincing {
    return best.expect("a pass was recorded").0;
  }
  if let Some(degrees) = pick_first_rotation(osd_rotation(img, datapath, Some(ppi)))
    && degrees != 0
    && consider(degrees, &mut tried).0
  {
    return best.expect("a pass was recorded").0;
  }
  if upright_ok {
    return best.expect("a pass was recorded").0;
  }
  for degrees in ROTATIONS {
    if !tried.contains(&degrees) && consider(degrees, &mut tried).0 {
      break;
    }
  }
  best.map(|b| b.0).unwrap_or_default()
}

/// Word boxes in the frame where the text reads upright: the image as given, or rotated by the
/// returned clockwise angle. The upright pass runs first and is final when it is convincing
/// (`OSD_SKIP_CONFIDENCE`); otherwise the OSD detector names the turn to try next, and the blind
/// retry over the remaining turns is the last resort. Boxes are in pixels of the rotated image,
/// whose size is also returned, together with who decided.
pub fn recognize_words_upright(img: &DynamicImage, lang: &str, ppi: Option<i32>) -> Upright {
  let datapath = tessdata_prefix().unwrap_or("");
  let mut tried: Vec<u32> = Vec::new();
  let mut best: Option<OrientedPass> = None;

  let mut consider =
    |degrees: u32, source: OrientationSource, tried: &mut Vec<u32>| -> (bool, bool) {
      tried.push(degrees);
      let (words, dims) = if degrees == 0 {
        (recognize_words(img, lang, ppi), (img.width(), img.height()))
      } else {
        let rotated = rotate_image(img, degrees);
        (
          recognize_words(&rotated, lang, ppi),
          (rotated.width(), rotated.height()),
        )
      };
      let confidence = mean_confidence(&words);
      if best.as_ref().is_none_or(|b| confidence > b.confidence) {
        best = Some(OrientedPass {
          words,
          degrees,
          dims,
          confidence,
          source,
        });
      }
      let b = best.as_ref().expect("a pass was recorded");
      (
        !b.words.is_empty() && b.confidence >= ROTATION_RETRY_CONFIDENCE,
        upright_is_convincing(b.words.len(), b.confidence),
      )
    };

  // Upright first: a convincing read needs no detector.
  let (upright_ok, convincing) = consider(0, OrientationSource::Upright, &mut tried);
  if convincing {
    let b = best.expect("a pass was recorded");
    return b.upright();
  }
  // Not convincing: ask the detector which turn to try before the blind retry.
  if let Some(degrees) = pick_first_rotation(osd_rotation(img, datapath, ppi))
    && degrees != 0
    && consider(degrees, OrientationSource::Osd, &mut tried).0
  {
    let b = best.expect("a pass was recorded");
    return b.upright();
  }
  // An acceptable upright read that the detector did not overturn stands; the blind retry is only
  // for pages nothing has read well yet.
  if upright_ok {
    let b = best.expect("a pass was recorded");
    return b.upright();
  }
  for degrees in ROTATIONS {
    if tried.contains(&degrees) {
      continue;
    }
    if consider(degrees, OrientationSource::Retry, &mut tried).0 {
      break;
    }
  }
  let b = best.expect("at least the upright pass ran");
  b.upright()
}

/// OCR an image and return every recognized word with its pixel box.
pub fn recognize_words(img: &DynamicImage, lang: &str, ppi: Option<i32>) -> Vec<OcrWord> {
  use tesseract_rs::TessPageIteratorLevel;

  let datapath = tessdata_prefix().unwrap_or("");
  let rgb = img.to_rgb8();
  let (w, h) = rgb.dimensions();
  let pixels = rgb.as_raw();

  let tess = tesseract_rs::TesseractAPI::new();
  if tess.init(datapath, lang).is_err()
    || tess
      .set_image(pixels, w as i32, h as i32, 3, (w * 3) as i32)
      .is_err()
  {
    return Vec::new();
  }
  set_resolution(&tess, ppi.unwrap_or(DEFAULT_SOURCE_PPI));
  if tess.recognize().is_err() {
    return Vec::new();
  }
  let Ok(it) = tess.get_iterator() else {
    return Vec::new();
  };
  let mut out = Vec::new();
  loop {
    if let Ok((text, left, top, right, bottom, confidence)) = it.get_word_with_bounds()
      && !text.trim().is_empty()
    {
      out.push(OcrWord {
        text: text.trim().to_string(),
        left,
        top,
        right,
        bottom,
        confidence,
      });
    }
    match it.next(TessPageIteratorLevel::RIL_WORD) {
      Ok(true) => continue,
      _ => break,
    }
  }
  out
}

/// Map pixel boxes of an image of `img_w`×`img_h` pixels onto the user-space
/// rectangle `dest = [x0, y0, x1, y1]` the image covers (y up).
pub fn place_words(words: Vec<OcrWord>, img_w: u32, img_h: u32, dest: [f64; 4]) -> Vec<Word> {
  let (iw, ih) = (f64::from(img_w.max(1)), f64::from(img_h.max(1)));
  let (dw, dh) = (dest[2] - dest[0], dest[3] - dest[1]);
  words
    .into_iter()
    .map(|w| {
      let x0 = dest[0] + f64::from(w.left) / iw * dw;
      let x1 = dest[0] + f64::from(w.right) / iw * dw;
      let y1 = dest[1] + (1.0 - f64::from(w.top) / ih) * dh;
      let y0 = dest[1] + (1.0 - f64::from(w.bottom) / ih) * dh;
      Word {
        text: w.text,
        x0,
        y0,
        x1,
        y1,
        baseline: y0,
        font_size: None,
        font: None,
        confidence: Some((f64::from(w.confidence) / 100.0).clamp(0.0, 1.0)),
      }
    })
    .collect()
}

#[cfg(test)]
mod tests;
