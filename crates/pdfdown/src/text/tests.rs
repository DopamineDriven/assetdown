use super::*;

#[test]
fn strip_basic_footer_artifact() {
  let text = "Some content\n/\n38\nMore content";
  let result = strip_footer_artifacts(text, "38");
  assert_eq!(result, "Some content\nMore content");
}

#[test]
fn strip_footer_artifact_with_whitespace() {
  let text = "Some content\n  /  \n  38  \nMore content";
  let result = strip_footer_artifacts(text, "38");
  assert_eq!(result, "Some content\nMore content");
}

#[test]
fn no_match_passthrough() {
  let text = "Some content\nNo footer here\nMore content";
  let result = strip_footer_artifacts(text, "38");
  assert_eq!(result, text);
}

#[test]
fn multiple_occurrences() {
  let text = "Page one\n/\n38\nPage two\n/\n38\nPage three";
  let result = strip_footer_artifacts(text, "38");
  assert_eq!(result, "Page one\nPage two\nPage three");
}

#[test]
fn at_start_of_text() {
  let text = "/\n38\nContent after";
  let result = strip_footer_artifacts(text, "38");
  assert_eq!(result, "Content after");
}

#[test]
fn at_end_of_text() {
  let text = "Content before\n/\n38";
  let result = strip_footer_artifacts(text, "38");
  assert_eq!(result, "Content before");
}

#[test]
fn empty_input() {
  let result = strip_footer_artifacts("", "38");
  assert_eq!(result, "");
}

#[test]
fn single_line_input() {
  let result = strip_footer_artifacts("just one line", "38");
  assert_eq!(result, "just one line");
}

#[test]
fn consecutive_pairs() {
  // Two pairs back to back: `/\n38\n/\n38`
  let text = "start\n/\n38\n/\n38\nend";
  let result = strip_footer_artifacts(text, "38");
  assert_eq!(result, "start\nend");
}

#[test]
fn slash_not_followed_by_count() {
  let text = "Some content\n/\n99\nMore content";
  let result = strip_footer_artifacts(text, "38");
  assert_eq!(result, text);
}

#[test]
fn slash_with_extra_text_not_stripped() {
  let text = "Some content\n/ extra\n38\nMore content";
  let result = strip_footer_artifacts(text, "38");
  assert_eq!(result, text);
}

#[test]
fn count_with_extra_text_not_stripped() {
  let text = "Some content\n/\n38 pages\nMore content";
  let result = strip_footer_artifacts(text, "38");
  assert_eq!(result, text);
}

// ── A page's text, read from where its glyphs are drawn ───────────

mod read_from_the_page {
  use lopdf::content::{Content, Operation};
  use lopdf::{Dictionary, Document, Object, ObjectId, Stream, dictionary};

  use crate::text::extract_text;

  fn op(name: &str, operands: Vec<Object>) -> Operation {
    Operation::new(name, operands)
  }

  fn helvetica() -> Dictionary {
    dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica" }
  }

  /// A document of one page that draws `ops`, with `font` as /F1 and these forms to draw.
  fn page_of(
    ops: Vec<Operation>,
    font: Dictionary,
    forms: Vec<(&str, ObjectId)>,
    mut doc: Document,
  ) -> Document {
    let pages_id = doc.new_object_id();
    let font_id = doc.add_object(font);
    let content = Content { operations: ops }.encode().unwrap();
    let content_id = doc.add_object(Stream::new(dictionary! {}, content));
    let mut xobjects = Dictionary::new();
    for (name, id) in forms {
      xobjects.set(name, Object::Reference(id));
    }
    let resources = dictionary! {
      "Font" => Object::Dictionary(dictionary! { "F1" => Object::Reference(font_id) }),
      "XObject" => Object::Dictionary(xobjects),
    };
    let page_id = doc.add_object(dictionary! {
      "Type" => "Page", "Parent" => Object::Reference(pages_id),
      "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
      "Contents" => Object::Reference(content_id), "Resources" => Object::Dictionary(resources),
    });
    doc.objects.insert(
      pages_id,
      Object::Dictionary(
        dictionary! { "Type" => "Pages", "Kids" => vec![Object::Reference(page_id)], "Count" => 1 },
      ),
    );
    let catalog =
      doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => Object::Reference(pages_id) });
    doc.trailer.set("Root", Object::Reference(catalog));
    doc
  }

  /// One string set at a place on the page, in /F1 at 10 points.
  fn shown(x: f64, y: f64, text: &str) -> Vec<Operation> {
    vec![
      op("BT", vec![]),
      op("Tf", vec!["F1".into(), 10.into()]),
      op("Td", vec![x.into(), y.into()]),
      op("Tj", vec![Object::string_literal(text)]),
      op("ET", vec![]),
    ]
  }

  fn text_of(doc: &Document) -> String {
    let [page] = extract_text(doc)
      .try_into()
      .unwrap_or_else(|pages: Vec<_>| {
        panic!("one page was made, and {} came back", pages.len());
      });
    page.text
  }

  #[test]
  fn text_a_page_draws_inside_a_form_is_the_pages_text() {
    // The page's own content has no text at all: it draws a form, and the words are in there,
    // set in a font the form brings with it.
    let mut doc = Document::with_version("1.7");
    let form_font = doc.add_object(helvetica());
    let mut form_ops = shown(0.0, 0.0, "Invoice 42");
    form_ops[1] = op("Tf", vec!["F9".into(), 10.into()]);
    let form = doc.add_object(Stream::new(
      dictionary! {
        "Type" => "XObject", "Subtype" => "Form",
        "BBox" => vec![0.into(), 0.into(), 200.into(), 20.into()],
        "Resources" => Object::Dictionary(dictionary! {
          "Font" => Object::Dictionary(dictionary! { "F9" => Object::Reference(form_font) }),
        }),
      },
      Content {
        operations: form_ops,
      }
      .encode()
      .unwrap(),
    ));
    let draws = vec![
      op("q", vec![]),
      op("Do", vec!["Fx".into()]),
      op("Q", vec![]),
    ];
    let doc = page_of(draws, helvetica(), vec![("Fx", form)], doc);
    assert_eq!(text_of(&doc), "Invoice 42");
  }

  #[test]
  fn a_code_the_fonts_map_gives_outside_the_range_it_declares_is_still_read() {
    // As one writer of PDFs makes them: the map declares that its codes run from A to Z, and
    // then gives the space and a digit as well. What the map gives is what the page means.
    let map = b"/CIDInit /ProcSet findresource begin 12 dict begin begincmap /CIDSystemInfo <<\n\
/Registry (AAAAAA+Arial) /Ordering (UCS) /Supplement 0 >> def\n\
/CMapName /AAAAAA+Arial def\n\
1 begincodespacerange <41> <5a> endcodespacerange\n\
9 beginbfrange\n\
<54> <54> <0054>\n<4f> <4f> <004f>\n<41> <41> <0041>\n<4c> <4c> <004c>\n<20> <20> <0020>\n\
<44> <44> <0044>\n<55> <55> <0055>\n<45> <45> <0045>\n<35> <35> <0035>\n\
endbfrange\nendcmap CMapName currentdict /CMap defineresource pop end end\n";
    let mut doc = Document::with_version("1.7");
    let map_id = doc.add_object(Stream::new(dictionary! {}, map.to_vec()));
    let font = dictionary! {
      "Type" => "Font", "Subtype" => "TrueType", "BaseFont" => "AAAAAA+Arial",
      "FirstChar" => 32, "LastChar" => 90,
      "ToUnicode" => Object::Reference(map_id),
    };
    let doc = page_of(shown(100.0, 700.0, "TOTAL DUE 5"), font, Vec::new(), doc);
    assert_eq!(text_of(&doc), "TOTAL DUE 5");
  }

  #[test]
  fn letters_drawn_one_at_a_time_are_read_as_the_words_they_spell() {
    // Each letter is set on its own, moved along by the width of the one before: Helvetica's
    // T, o, t, a, l at 10 points. Then a gap, and "Due" the same way.
    let letters = [
      ("T", 100.0),
      ("o", 6.11),
      ("t", 5.56),
      ("a", 2.78),
      ("l", 5.56),
      ("D", 2.22 + 5.0),
      ("u", 7.22),
      ("e", 5.56),
    ];
    let mut ops = vec![
      op("BT", vec![]),
      op("Tf", vec!["F1".into(), 10.into()]),
      op("Td", vec![0.into(), 700.into()]),
    ];
    for (letter, along) in letters {
      ops.push(op("Td", vec![along.into(), 0.into()]));
      ops.push(op("Tj", vec![Object::string_literal(letter)]));
    }
    ops.push(op("ET", vec![]));
    let doc = page_of(ops, helvetica(), Vec::new(), Document::with_version("1.7"));
    assert_eq!(text_of(&doc), "Total Due");
  }

  #[test]
  fn lines_are_read_from_the_top_of_the_page_down_whatever_order_they_were_drawn_in() {
    let mut ops = shown(100.0, 600.0, "Lower line");
    ops.extend(shown(100.0, 700.0, "Upper line"));
    let doc = page_of(ops, helvetica(), Vec::new(), Document::with_version("1.7"));
    assert_eq!(text_of(&doc), "Upper line\nLower line");
  }
}
