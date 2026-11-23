use super::*;
use crate::TextWord;
use crate::document::{DownFrom, Placing, Run};

fn paragraph(text: &str) -> Paragraph {
  Paragraph {
    inlines: vec![Inline::Text(Run {
      text: text.to_string(),
      size: 12.0,
      bold: false,
      italic: false,
      font: None,
    })],
    ..Paragraph::default()
  }
}

/// A paragraph given a place of its own: 200 points in from the left margin, 100 wide, and
/// `y` points down from what `down_from` names.
fn placed(text: &str, y: f64, down_from: DownFrom) -> Paragraph {
  Paragraph {
    placing: Some(Placing {
      x: 200.0,
      y,
      width: Some(100.0),
      from_page_side: false,
      down_from,
    }),
    ..paragraph(text)
  }
}

fn set(paragraphs: Vec<Paragraph>) -> PageLayout {
  let body = Body {
    blocks: paragraphs.into_iter().map(Block::Paragraph).collect(),
    paper: Paper {
      width: 612.0,
      height: 792.0,
      left: 72.0,
      right: 72.0,
      top: 72.0,
      bottom: 72.0,
    },
    running: Running::default(),
  };
  let mut pages = pages(&body, "estimated");
  assert_eq!(pages.len(), 1);
  pages.remove(0)
}

/// A word of the page. Words set level with one another are one line, so the words are what
/// is looked at.
fn word<'a>(page: &'a PageLayout, text: &str) -> &'a TextWord {
  page
    .words
    .iter()
    .find(|word| word.text == text)
    .unwrap_or_else(|| panic!("no word {text:?}"))
}

/// How far down the page a word is: both of its edges.
fn down(page: &PageLayout, text: &str) -> (f64, f64) {
  let word = word(page, text);
  (word.y0, word.y1)
}

#[test]
fn a_paragraph_placed_from_the_margin_or_the_page_is_set_there_whatever_comes_before_it() {
  let page = set(vec![
    paragraph("First"),
    paragraph("Second"),
    placed("Placed", 0.0, DownFrom::TopMargin),
  ]);
  // Level with the first paragraph, which starts at the top margin.
  assert_eq!(down(&page, "Placed"), down(&page, "First"));
  assert_eq!(word(&page, "Placed").x0, 272.0);

  let higher = set(vec![
    paragraph("First"),
    placed("Placed", 0.0, DownFrom::PageTop),
  ]);
  let (top_of_first, top_of_placed) = (down(&higher, "First"), down(&higher, "Placed"));
  assert_ne!(top_of_first, top_of_placed);
  // The top margin is 72 points, and that is how far apart the two are.
  assert!(((top_of_first.0 - top_of_placed.0).abs() - 72.0).abs() < 1e-6);
}

#[test]
fn a_paragraph_placed_from_where_the_text_has_reached_is_set_beside_what_follows_it() {
  let page = set(vec![
    paragraph("First"),
    placed("Placed", 0.0, DownFrom::Text),
    paragraph("Next"),
  ]);
  // It starts where the text stands, and the text goes on as though it were not there.
  assert_eq!(down(&page, "Placed"), down(&page, "Next"));
  assert_ne!(down(&page, "Placed"), down(&page, "First"));

  let lower = set(vec![
    paragraph("First"),
    placed("Placed", 30.0, DownFrom::Text),
    paragraph("Next"),
  ]);
  let apart = (down(&lower, "Placed").0 - down(&lower, "Next").0).abs();
  assert!((apart - 30.0).abs() < 1e-6, "{apart}");
}
