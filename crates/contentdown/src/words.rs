//! Words → lines. Purely geometric: works for native glyph boxes and for OCR
//! word boxes alike.

use crate::layout::{TextLine, TextWord};

/// Font identity shared by every glyph set in that font.
#[derive(Debug, Clone, PartialEq)]
pub struct FontMeta {
  pub name: String,
  pub bold: bool,
  pub italic: bool,
  pub symbolic: bool,
  /// False when the font gives no weight evidence; `bold` and `italic` are then not reported.
  pub weight_known: bool,
}

/// Intermediate word used by both the native and OCR paths.
#[derive(Debug, Clone)]
pub struct Word {
  pub text: String,
  pub x0: f64,
  pub y0: f64,
  pub x1: f64,
  pub y1: f64,
  /// Baseline y in user space; words are grouped into lines by it.
  pub baseline: f64,
  pub font_size: Option<f64>,
  /// Native words: index into the `fonts` slice given to [`words_to_lines`].
  pub font: Option<usize>,
  /// OCR words: recognition confidence, 0..1.
  pub confidence: Option<f64>,
}

struct LineAcc {
  /// Each word with its position in the caller's input.
  words: Vec<(usize, Word)>,
  baseline_sum: f64,
  y0: f64,
  y1: f64,
}

impl LineAcc {
  fn baseline(&self) -> f64 {
    self.baseline_sum / self.words.len() as f64
  }
}

fn vertical_overlap(a0: f64, a1: f64, b0: f64, b1: f64) -> f64 {
  let inter = (a1.min(b1) - a0.max(b0)).max(0.0);
  let shortest = (a1 - a0).min(b1 - b0).max(1e-6);
  inter / shortest
}

/// Cluster words into baseline bands, order bands top to bottom and words
/// left to right, and emit the public `TextWord` / `TextLine` arrays.
pub fn words_to_lines(words: Vec<Word>, fonts: &[FontMeta]) -> (Vec<TextWord>, Vec<TextLine>) {
  let (words, lines, _) = words_to_lines_indexed(words, fonts);
  (words, lines)
}

/// [`words_to_lines`], plus where each emitted word came from: `origins[i]` is the position in
/// the input of the `Word` that became `words[i]`. A caller that keeps facts about its words
/// beside them, such as the cell a spreadsheet word was laid out from, carries them over with it.
pub fn words_to_lines_indexed(
  words: Vec<Word>,
  fonts: &[FontMeta],
) -> (Vec<TextWord>, Vec<TextLine>, Vec<usize>) {
  if words.is_empty() {
    return (Vec::new(), Vec::new(), Vec::new());
  }
  let mut heights: Vec<f64> = words.iter().map(|w| w.y1 - w.y0).collect();
  heights.sort_by(f64::total_cmp);
  let median_height = heights[heights.len() / 2].max(1e-6);
  let tolerance = 0.5 * median_height;

  let mut sorted: Vec<(usize, Word)> = words.into_iter().enumerate().collect();
  sorted.sort_by(|(_, a), (_, b)| {
    b.baseline
      .total_cmp(&a.baseline)
      .then(a.x0.total_cmp(&b.x0))
  });

  let mut lines: Vec<LineAcc> = Vec::new();
  for (origin, w) in sorted {
    let fit = lines.iter_mut().find(|l| {
      (l.baseline() - w.baseline).abs() <= tolerance
        && vertical_overlap(l.y0, l.y1, w.y0, w.y1) > 0.5
    });
    match fit {
      Some(l) => {
        l.baseline_sum += w.baseline;
        l.y0 = l.y0.min(w.y0);
        l.y1 = l.y1.max(w.y1);
        l.words.push((origin, w));
      }
      None => lines.push(LineAcc {
        baseline_sum: w.baseline,
        y0: w.y0,
        y1: w.y1,
        words: vec![(origin, w)],
      }),
    }
  }

  lines.sort_by(|a, b| {
    b.y1
      .total_cmp(&a.y1)
      .then(a.words[0].1.x0.total_cmp(&b.words[0].1.x0))
  });

  let mut out_words: Vec<TextWord> = Vec::new();
  let mut out_lines: Vec<TextLine> = Vec::new();
  let mut origins: Vec<usize> = Vec::new();
  for (li, mut line) in lines.into_iter().enumerate() {
    line.words.sort_by(|(_, a), (_, b)| a.x0.total_cmp(&b.x0));
    let first_word = out_words.len() as u32;
    let x0 = line
      .words
      .iter()
      .map(|(_, w)| w.x0)
      .fold(f64::MAX, f64::min);
    let x1 = line
      .words
      .iter()
      .map(|(_, w)| w.x1)
      .fold(f64::MIN, f64::max);
    let text = line
      .words
      .iter()
      .map(|(_, w)| w.text.as_str())
      .collect::<Vec<_>>()
      .join(" ");
    let word_count = line.words.len() as u32;
    for (origin, w) in line.words {
      origins.push(origin);
      let meta = w.font.and_then(|i| fonts.get(i));
      out_words.push(TextWord {
        text: w.text,
        x0: w.x0,
        y0: w.y0,
        x1: w.x1,
        y1: w.y1,
        line: li as u32,
        font_size: w.font_size,
        font_name: meta.map(|m| m.name.clone()),
        bold: meta.and_then(|m| m.weight_known.then_some(m.bold)),
        italic: meta.and_then(|m| m.weight_known.then_some(m.italic)),
        symbolic: meta.map(|m| m.symbolic),
        confidence: w.confidence,
      });
    }
    out_lines.push(TextLine {
      text,
      x0,
      y0: line.y0,
      x1,
      y1: line.y1,
      first_word,
      word_count,
    });
  }
  (out_words, out_lines, origins)
}

#[cfg(test)]
mod tests;
