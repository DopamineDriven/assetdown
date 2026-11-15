//! Glyphs → words. The words then go to `contentdown`, which groups them into lines.

use super::interpreter::Glyph;
use contentdown::Word;

/// A gap wider than this fraction of the font size separates words.
const WORD_GAP_EM: f64 = 0.25;
/// A pen moving backwards by more than this fraction of the font size starts a new word.
const BACKTRACK_EM: f64 = 0.5;

fn word_from_glyph(g: &Glyph) -> Word {
  Word {
    text: g.text.clone(),
    x0: g.x0,
    y0: g.y0,
    x1: g.x1,
    y1: g.y1,
    baseline: g.base_y,
    font_size: Some(g.font_size),
    font: Some(g.font),
    confidence: None,
  }
}

fn extend_word(word: &mut Word, g: &Glyph) {
  word.text.push_str(&g.text);
  word.x0 = word.x0.min(g.x0);
  word.y0 = word.y0.min(g.y0);
  word.x1 = word.x1.max(g.x1);
  word.y1 = word.y1.max(g.y1);
  if let Some(fs) = word.font_size {
    word.font_size = Some(fs.max(g.font_size));
  }
}

/// Group consecutive glyphs into words: split at space glyphs, explicit word
/// gaps, baseline changes, backwards pen moves, and horizontal gaps.
pub fn glyphs_to_words(glyphs: &[Glyph]) -> Vec<Word> {
  let mut words: Vec<Word> = Vec::new();
  let mut current: Option<Word> = None;
  for g in glyphs {
    if g.is_space {
      if let Some(w) = current.take() {
        words.push(w);
      }
      continue;
    }
    let starts_new = match &current {
      None => true,
      Some(w) => {
        let em = g.font_size.max(1e-6);
        g.break_before
          || (g.base_y - w.baseline).abs() > 0.5 * em
          || g.x0 - w.x1 > WORD_GAP_EM * em
          || g.x0 < w.x1 - BACKTRACK_EM * em
      }
    };
    if starts_new {
      if let Some(w) = current.take() {
        words.push(w);
      }
      current = Some(word_from_glyph(g));
    } else if let Some(w) = current.as_mut() {
      extend_word(w, g);
    }
  }
  if let Some(w) = current {
    words.push(w);
  }
  words.retain(|w| !w.text.trim().is_empty());
  words
}

#[cfg(test)]
mod tests;
