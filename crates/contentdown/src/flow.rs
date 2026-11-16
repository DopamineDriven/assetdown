//! Setting text by estimate.
//!
//! A flow document and a slide record their text, not where its lines break. The format crates
//! that lay such text out as a page all do it one way, which is kept here: a character is taken
//! to be half its font size wide, a line 1.2 times its size tall, and words are set left to
//! right and wrapped at the right edge.

use std::collections::HashMap;

use crate::words::FontMeta;

/// Estimated advance of one character as a fraction of the font size.
pub const CHAR_EM: f64 = 0.5;
/// Height of one line as a fraction of the font size.
pub const LINE_EM: f64 = 1.2;
/// Tab stops stand half an inch apart when nothing sets them.
pub const TAB_STOP: f64 = 36.0;
/// What a sum of estimates may be off by before it counts as not fitting.
pub const SLACK: f64 = 0.01;

/// The fonts words are set in, each once.
#[derive(Debug, Default)]
pub struct Fonts {
  metas: Vec<FontMeta>,
  index: HashMap<(Option<String>, bool, bool), usize>,
}

impl Fonts {
  /// The place of a font among the ones seen, adding it when it is new. Text whose font nothing
  /// names is set in a font with a blank name.
  pub fn intern(&mut self, name: Option<&str>, bold: bool, italic: bool) -> usize {
    let key = (name.map(str::to_owned), bold, italic);
    if let Some(&at) = self.index.get(&key) {
      return at;
    }
    let at = self.metas.len();
    self.metas.push(FontMeta {
      name: name.unwrap_or_default().to_owned(),
      bold,
      italic,
      symbolic: false,
      weight_known: true,
    });
    self.index.insert(key, at);
    at
  }

  pub fn metas(&self) -> &[FontMeta] {
    &self.metas
  }
}

/// One piece of a paragraph as it is set.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
  Word {
    text: String,
    size: f64,
    font: usize,
  },
  Space,
  Tab,
  Newline,
}

/// Tokens built from text as it comes, run by run. A word that runs across a change of
/// formatting stays one word, in the formatting it starts in.
#[derive(Debug, Default)]
pub struct Tokens {
  out: Vec<Token>,
  word: Option<(String, f64, usize)>,
}

impl Tokens {
  /// Ends the word being built, if one is.
  pub fn end_word(&mut self) {
    if let Some((text, size, font)) = self.word.take() {
      self.out.push(Token::Word { text, size, font });
    }
  }

  /// Adds text in one formatting. A tab in it is a tab, a line break a line break, and any other
  /// white space parts words.
  pub fn text(&mut self, text: &str, size: f64, font: usize) {
    for c in text.chars() {
      match c {
        '\t' => self.tab(),
        '\n' | '\u{b}' => self.newline(),
        c if c.is_whitespace() => {
          self.end_word();
          if !matches!(self.out.last(), Some(Token::Space)) {
            self.out.push(Token::Space);
          }
        }
        c => match &mut self.word {
          Some((word, _, _)) => word.push(c),
          None => self.word = Some((c.to_string(), size, font)),
        },
      }
    }
  }

  /// Adds a word that stands on its own, such as a list marker, with a space after it.
  pub fn word(&mut self, text: &str, size: f64, font: usize) {
    self.end_word();
    self.out.push(Token::Word {
      text: text.to_owned(),
      size,
      font,
    });
    self.out.push(Token::Space);
  }

  pub fn tab(&mut self) {
    self.end_word();
    self.out.push(Token::Tab);
  }

  pub fn newline(&mut self) {
    self.end_word();
    self.out.push(Token::Newline);
  }

  pub fn finish(mut self) -> Vec<Token> {
    self.end_word();
    self.out
  }
}

/// The size of the first word, which stands for the size of the paragraph.
pub fn base_size(tokens: &[Token]) -> Option<f64> {
  tokens.iter().find_map(|token| match token {
    Token::Word { size, .. } => Some(*size),
    _ => None,
  })
}

/// A word placed on a line, before the line has a height.
#[derive(Debug, Clone, PartialEq)]
pub struct Set {
  pub text: String,
  pub x0: f64,
  pub x1: f64,
  pub size: f64,
  pub font: usize,
}

/// Sets tokens in lines between `left` and `right`. A word that does not fit starts a new line
/// when `wrap` is on; a line break always does. There is always at least one line, which may be
/// empty.
pub fn set_lines(tokens: Vec<Token>, left: f64, right: f64, wrap: bool) -> Vec<Vec<Set>> {
  let mut lines = vec![Vec::new()];
  let mut x = left;
  let mut space = false;
  for token in tokens {
    match token {
      Token::Word { text, size, font } => {
        let width = text.chars().count() as f64 * CHAR_EM * size;
        let words = lines.last().map_or(0, Vec::len);
        let mut gap = if space && words > 0 {
          CHAR_EM * size
        } else {
          0.0
        };
        if wrap && words > 0 && x + gap + width > right + SLACK {
          lines.push(Vec::new());
          (x, gap) = (left, 0.0);
        }
        let x0 = x + gap;
        x = x0 + width;
        space = false;
        if let Some(line) = lines.last_mut() {
          line.push(Set {
            text,
            x0,
            x1: x,
            size,
            font,
          });
        }
      }
      Token::Space => space = true,
      Token::Tab => {
        x = left + (((x - left) / TAB_STOP).floor() + 1.0) * TAB_STOP;
        space = false;
        if wrap && x >= right {
          lines.push(Vec::new());
          x = left;
        }
      }
      Token::Newline => {
        lines.push(Vec::new());
        (x, space) = (left, false);
      }
    }
  }
  lines
}

/// How tall a line is: as tall as its largest word needs, or as a word of size `base` would
/// when the line is empty.
pub fn line_height(line: &[Set], base: f64) -> f64 {
  let largest = line.iter().map(|set| set.size).fold(0.0, f64::max);
  LINE_EM * if line.is_empty() { base } else { largest }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn texts(lines: &[Vec<Set>]) -> Vec<Vec<&str>> {
    lines
      .iter()
      .map(|line| line.iter().map(|set| set.text.as_str()).collect())
      .collect()
  }

  #[test]
  fn text_is_cut_into_words_and_a_word_across_two_runs_stays_one() {
    let mut tokens = Tokens::default();
    tokens.word("1.", 12.0, 0);
    tokens.text("In", 20.0, 1);
    tokens.text("voice  4711\tdue\nnow ", 10.0, 2);
    let word = |text: &str, size, font| Token::Word {
      text: text.to_owned(),
      size,
      font,
    };
    assert_eq!(
      tokens.finish(),
      [
        word("1.", 12.0, 0),
        Token::Space,
        word("Invoice", 20.0, 1),
        Token::Space,
        word("4711", 10.0, 2),
        Token::Tab,
        word("due", 10.0, 2),
        Token::Newline,
        word("now", 10.0, 2),
        Token::Space,
      ]
    );
  }

  #[test]
  fn words_wrap_at_the_right_edge_and_tabs_move_to_the_next_stop() {
    let mut tokens = Tokens::default();
    // Each word is 20 wide at size 10, and the next starts 25 further on.
    tokens.text("aaaa bbbb cccc dddd\teeee", 10.0, 0);
    let lines = set_lines(tokens.finish(), 100.0, 175.0, true);
    assert_eq!(
      texts(&lines),
      [vec!["aaaa", "bbbb", "cccc"], vec!["dddd", "eeee"]]
    );
    let starts: Vec<f64> = lines.iter().flatten().map(|set| set.x0).collect();
    assert_eq!(starts, [100.0, 125.0, 150.0, 100.0, 136.0]);
    assert_eq!(lines[0][2].x1, 170.0);

    let mut unwrapped = Tokens::default();
    unwrapped.text("aaaa bbbb cccc dddd\nlast", 10.0, 0);
    let lines = set_lines(unwrapped.finish(), 100.0, 175.0, false);
    assert_eq!(
      texts(&lines),
      [vec!["aaaa", "bbbb", "cccc", "dddd"], vec!["last"]]
    );
  }

  #[test]
  fn a_line_is_as_tall_as_its_largest_word_and_an_empty_one_as_the_paragraph() {
    let lines = set_lines(Vec::new(), 0.0, 100.0, true);
    assert_eq!(lines.len(), 1);
    assert_eq!(line_height(&lines[0], 11.0), 1.2 * 11.0);
    let mut tokens = Tokens::default();
    tokens.text("small", 8.0, 0);
    tokens.text(" BIG", 20.0, 1);
    let tokens = tokens.finish();
    assert_eq!(base_size(&tokens), Some(8.0));
    assert_eq!(
      line_height(&set_lines(tokens, 0.0, 500.0, true)[0], 11.0),
      24.0
    );
  }

  #[test]
  fn a_font_is_kept_once_and_an_unnamed_one_has_a_blank_name() {
    let mut fonts = Fonts::default();
    let plain = fonts.intern(Some("Arial"), false, false);
    let bold = fonts.intern(Some("Arial"), true, false);
    let unnamed = fonts.intern(None, false, false);
    assert_eq!((plain, bold, unnamed), (0, 1, 2));
    assert_eq!(fonts.intern(Some("Arial"), false, false), 0);
    assert_eq!(fonts.metas().len(), 3);
    assert_eq!(fonts.metas()[2].name, "");
    assert!(fonts.metas()[1].bold && fonts.metas()[1].weight_known);
  }
}
