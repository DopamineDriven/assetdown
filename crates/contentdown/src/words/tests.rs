use super::*;

/// A word whose box sits around `baseline` the way a glyph run of `size` points does.
fn word(text: &str, x0: f64, baseline: f64, width: f64, size: f64) -> Word {
  Word {
    text: text.into(),
    x0,
    y0: baseline - 0.2 * size,
    x1: x0 + width,
    y1: baseline + 0.7 * size,
    baseline,
    font_size: Some(size),
    font: Some(0),
    confidence: None,
  }
}

fn font(name: &str, weight_known: bool) -> FontMeta {
  FontMeta {
    name: name.into(),
    bold: true,
    italic: false,
    symbolic: false,
    weight_known,
  }
}

#[test]
fn lines_cluster_by_baseline_and_order_top_down_left_right() {
  let fonts = vec![FontMeta {
    name: "Helvetica".into(),
    bold: false,
    italic: false,
    symbolic: false,
    weight_known: true,
  }];
  let w = vec![
    word("low", 10.0, 600.0, 20.0, 10.0),
    word("right", 200.0, 700.5, 30.0, 10.0),
    word("left", 10.0, 700.0, 25.0, 10.0),
  ];
  let (words, lines) = words_to_lines(w, &fonts);
  assert_eq!(lines.len(), 2);
  assert_eq!(lines[0].text, "left right");
  assert_eq!(lines[1].text, "low");
  assert_eq!((lines[0].first_word, lines[0].word_count), (0, 2));
  assert_eq!((lines[1].first_word, lines[1].word_count), (2, 1));
  assert_eq!(words.iter().map(|w| w.line).collect::<Vec<_>>(), [0, 0, 1]);
  assert_eq!(words[0].font_name.as_deref(), Some("Helvetica"));
  assert_eq!(words[0].bold, Some(false));
}

#[test]
fn no_words_give_no_lines() {
  let (words, lines) = words_to_lines(Vec::new(), &[]);
  assert!(words.is_empty());
  assert!(lines.is_empty());
}

#[test]
fn a_line_box_spans_its_words() {
  let w = vec![
    word("tall", 100.0, 500.0, 40.0, 20.0),
    word("small", 10.0, 500.0, 30.0, 16.0),
  ];
  let (_, lines) = words_to_lines(w, &[]);
  assert_eq!(lines.len(), 1);
  assert_eq!(lines[0].text, "small tall");
  assert_eq!((lines[0].x0, lines[0].x1), (10.0, 140.0));
  assert_eq!((lines[0].y0, lines[0].y1), (496.0, 514.0));
}

#[test]
fn a_font_without_weight_evidence_reports_neither_bold_nor_italic() {
  let fonts = vec![font("CIDFont+F1", false), font("Arial-BoldMT", true)];
  let mut second = word("bold", 60.0, 700.0, 30.0, 10.0);
  second.font = Some(1);
  let w = vec![word("plain", 10.0, 700.0, 30.0, 10.0), second];
  let (words, _) = words_to_lines(w, &fonts);
  assert_eq!(words[0].font_name.as_deref(), Some("CIDFont+F1"));
  assert_eq!((words[0].bold, words[0].italic), (None, None));
  assert_eq!(words[0].symbolic, Some(false));
  assert_eq!((words[1].bold, words[1].italic), (Some(true), Some(false)));
}

#[test]
fn an_ocr_word_keeps_its_confidence_and_has_no_font() {
  let mut ocr = word("scan", 10.0, 700.0, 30.0, 10.0);
  ocr.font = None;
  ocr.font_size = None;
  ocr.confidence = Some(0.87);
  let (words, _) = words_to_lines(vec![ocr], &[font("Unused", true)]);
  assert_eq!(words[0].confidence, Some(0.87));
  assert_eq!(words[0].font_name, None);
  assert_eq!(
    (words[0].bold, words[0].italic, words[0].symbolic),
    (None, None, None)
  );
  assert_eq!(words[0].font_size, None);
}

#[test]
fn a_font_index_past_the_table_is_treated_as_no_font() {
  let mut stray = word("x", 10.0, 700.0, 5.0, 10.0);
  stray.font = Some(3);
  let (words, _) = words_to_lines(vec![stray], &[font("Only", true)]);
  assert_eq!(words[0].font_name, None);
  assert_eq!(words[0].bold, None);
}

#[test]
fn origins_name_the_input_word_behind_each_emitted_word() {
  let w = vec![
    word("low", 10.0, 600.0, 20.0, 10.0),
    word("right", 200.0, 700.0, 30.0, 10.0),
    word("left", 10.0, 700.0, 25.0, 10.0),
  ];
  let (words, _, origins) = words_to_lines_indexed(w, &[]);
  assert_eq!(
    words.iter().map(|w| w.text.as_str()).collect::<Vec<_>>(),
    ["left", "right", "low"]
  );
  assert_eq!(origins, [2, 1, 0]);
}
