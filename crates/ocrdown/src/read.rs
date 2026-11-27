//! Reading a picture: its text, or its words with where each one is.

use contentdown::Word;
use image::DynamicImage;

use crate::engine::tessdata_prefix;
use crate::resolution::{DEFAULT_SOURCE_PPI, set_resolution};
use crate::turn::{
  OrientationSource, ROTATION_RETRY_CONFIDENCE, ROTATIONS, expected_is_convincing, osd_rotation,
  pick_first_rotation, rotate_image, upright_is_convincing,
};

/// How a picture of gray or of color is cut into black and white before it is read. A picture
/// that is already black and white reads the same either way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Thresholding {
  /// One cut for the whole picture: Tesseract's own default. Text in a light gray is taken
  /// for paper on a page that also has black text.
  #[default]
  Global,
  /// A cut for each part of the picture. Measured on six invoice pages rendered in gray at
  /// 150, 200, and 300 pixels to the inch, against the words of the PDFs they were rendered
  /// from: 93%, 91%, and 91% of the words were found this way, and 89%, 83%, and 83% with
  /// one cut.
  Adaptive,
}

impl Thresholding {
  fn set(self, tess: &tesseract_rs::TesseractAPI) {
    if self == Thresholding::Adaptive {
      let _ = tess.set_variable("thresholding_method", "1");
    }
  }
}

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
  recognize_words_upright_cut(img, lang, ppi, Thresholding::Global)
}

/// [`recognize_words_upright`], told the clockwise turn the picture is expected to need: the
/// one its page says it is shown at, say. That turn is read before the picture as it is, and
/// stands when a page's worth of words reads convincingly at it, so that a picture kept on its
/// side in a page shown turned is read once, and the detector is not asked. An expectation that does not read
/// convincingly costs the one pass and changes nothing else: the search ends where it does
/// with nothing expected. Anything but a quarter turn or a half turn is no expectation.
pub fn recognize_words_upright_expecting(
  img: &DynamicImage,
  lang: &str,
  ppi: Option<i32>,
  expected: u32,
) -> Upright {
  upright_search(img, lang, ppi, Thresholding::Global, expected)
}

/// [`recognize_words_upright`], with the way the picture is cut into black and white chosen.
pub fn recognize_words_upright_cut(
  img: &DynamicImage,
  lang: &str,
  ppi: Option<i32>,
  cut: Thresholding,
) -> Upright {
  upright_search(img, lang, ppi, cut, 0)
}

/// The search for the way up over a picture, read by Tesseract at each turn it tries.
fn upright_search(
  img: &DynamicImage,
  lang: &str,
  ppi: Option<i32>,
  cut: Thresholding,
  expected: u32,
) -> Upright {
  let datapath = tessdata_prefix().unwrap_or("");
  way_up(
    expected,
    |degrees| {
      if degrees == 0 {
        (words_cut(img, lang, ppi, cut), (img.width(), img.height()))
      } else {
        let rotated = rotate_image(img, degrees);
        (
          words_cut(&rotated, lang, ppi, cut),
          (rotated.width(), rotated.height()),
        )
      }
    },
    || osd_rotation(img, datapath, ppi),
  )
}

/// What one pass over a picture gives: its words, and the width and height of the picture as
/// it was turned for the pass.
pub(crate) type Pass = (Vec<OcrWord>, (u32, u32));

/// Which way up a picture reads, and its words that way up. `read` gives the words of the
/// picture turned clockwise by so many degrees, with the turned picture's size; `detect` asks
/// the orientation detector for the turn it would make and how sure it is.
///
/// The expected turn, when there is one, is read ahead of everything, and stands at once when
/// it reads convincingly and holds more than a word or two. When it does not, the search is the one made with nothing expected,
/// and ends where that one does: what was read ahead is only kept for when the search comes
/// to that turn itself, so that no turn is read twice. That search reads the picture as it
/// is, which stands when it is convincing; failing that the detector names a turn to read,
/// and the turns not yet read are the last resort. Of the turns the search reads, the one
/// whose words are surest is kept.
pub(crate) fn way_up(
  expected: u32,
  mut read: impl FnMut(u32) -> Pass,
  detect: impl FnOnce() -> Option<(u32, f32)>,
) -> Upright {
  let expected = expected % 360;
  let mut ahead: Option<(u32, Pass)> = None;
  if ROTATIONS.contains(&expected) {
    let (words, size) = read(expected);
    if expected_is_convincing(words.len(), mean_confidence(&words)) {
      return Upright {
        words,
        degrees: expected,
        size,
        decided: OrientationSource::Expected,
      };
    }
    ahead = Some((expected, (words, size)));
  }
  // From here on, the search as it is made with nothing expected.
  let mut read = |degrees: u32| match ahead.take_if(|(turn, _)| *turn == degrees) {
    Some((_, pass)) => pass,
    None => read(degrees),
  };
  let mut tried: Vec<u32> = Vec::new();
  let mut best: Option<OrientedPass> = None;

  let mut consider =
    |degrees: u32, source: OrientationSource, tried: &mut Vec<u32>| -> (bool, bool) {
      tried.push(degrees);
      let (words, dims) = read(degrees);
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
  if let Some(degrees) = pick_first_rotation(detect())
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
  words_cut(img, lang, ppi, Thresholding::Global)
}

/// [`recognize_words`], with the way the picture is cut into black and white chosen.
pub(crate) fn words_cut(
  img: &DynamicImage,
  lang: &str,
  ppi: Option<i32>,
  cut: Thresholding,
) -> Vec<OcrWord> {
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
  cut.set(&tess);
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
