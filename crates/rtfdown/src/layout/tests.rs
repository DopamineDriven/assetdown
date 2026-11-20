use contentdown::{PageLayout, TextSource};

use crate::Rtf;
use crate::testing::{document, object, object_data};

fn pages(body: &str) -> Vec<PageLayout> {
  Rtf::open(&document(body)).unwrap().layout()
}

fn lines(page: &PageLayout) -> Vec<&str> {
  page.lines.iter().map(|line| line.text.as_str()).collect()
}

fn close(a: f64, b: f64) -> bool {
  (a - b).abs() < 1e-6
}

// A file that states no paper is set on letter, 612 by 792 points, with margins of 90 at the
// sides and 72 at the top and the foot. Text that states no size is 12 points: a character is
// taken to be 6 points wide and a line 14.4 tall.

#[test]
fn a_paragraphs_words_are_set_from_the_left_margin_down_from_the_top_margin() {
  let [page] = &pages("Invoice 42\\par")[..] else {
    panic!("one page");
  };
  assert_eq!((page.page, page.width, page.height), (1, 612.0, 792.0));
  assert_eq!(page.source, TextSource::Native);
  assert_eq!(lines(page), ["Invoice 42"]);
  let [invoice, number] = &page.words[..] else {
    panic!("two words, and there are {}", page.words.len());
  };
  assert!(
    close(invoice.x0, 90.0) && close(invoice.x1, 132.0),
    "{invoice:?}"
  );
  assert!(
    close(invoice.y1, 720.0) && close(invoice.y0, 705.6),
    "{invoice:?}"
  );
  assert!(
    close(number.x0, 138.0) && close(number.x1, 150.0),
    "{number:?}"
  );
  assert_eq!(invoice.font_size, Some(12.0));
  assert_eq!(
    page.warnings,
    ["word positions are estimated from the flow of the text; an RTF file records no layout"]
  );
}

#[test]
fn a_paragraph_lined_up_on_the_right_ends_at_the_right_margin_and_an_indent_moves_its_start() {
  let found = pages("\\qr Total 5.00\\par\\pard\\li720 Indented\\par");
  let words = &found[0].words;
  assert!(close(words[1].x1, 522.0), "{:?}", words[1]);
  assert!(close(words[2].x0, 126.0), "{:?}", words[2]);
}

#[test]
fn the_cells_of_a_row_are_set_at_their_edges_on_one_line() {
  let found =
    pages("\\trowd\\cellx2880\\cellx5760\\pard\\intbl Item\\cell\\pard\\intbl 5.00\\cell\\row");
  let page = &found[0];
  assert_eq!(lines(page), ["Item 5.00"]);
  // A cell's text starts 5 points inside its edge: the first at the margin, the second 144
  // points along.
  assert!(close(page.words[0].x0, 95.0), "{:?}", page.words[0]);
  assert!(close(page.words[1].x0, 239.0), "{:?}", page.words[1]);
  assert!(close(page.words[0].y0, page.words[1].y0));
}

#[test]
fn a_page_break_starts_a_page() {
  let found = pages("One\\par\\page Two\\par");
  assert_eq!(
    found.iter().map(|page| page.page).collect::<Vec<_>>(),
    [1, 2]
  );
  assert_eq!(
    (lines(&found[0]), lines(&found[1])),
    (vec!["One"], vec!["Two"])
  );
  assert!(close(found[1].words[0].y1, 720.0));
}

#[test]
fn text_that_runs_past_the_foot_of_a_page_goes_on_to_the_next() {
  // A hundred paragraphs of a line each, with nothing stated between them. A line takes 14.4
  // points and the page has 648 between its margins, which holds forty-five.
  let body: String = (1..=100).map(|n| format!("Line {n}\\par ")).collect();
  let found = pages(&body);
  assert_eq!(found.len(), 3);
  assert_eq!(
    found
      .iter()
      .map(|page| page.lines.len())
      .collect::<Vec<_>>(),
    [45, 45, 10]
  );
  for page in &found {
    assert_eq!(page.height, 792.0);
    assert!(
      page
        .words
        .iter()
        .all(|word| word.y0 >= 72.0 - 1e-6 && word.y1 <= 720.0 + 1e-6)
    );
  }
  assert_eq!(lines(&found[1])[0], "Line 46");
}

#[test]
fn a_file_with_no_text_is_one_page_with_no_words() {
  let [page] = &pages("")[..] else {
    panic!("one page");
  };
  assert!(page.words.is_empty() && page.lines.is_empty());
}

#[test]
fn the_invoice_is_read_from_the_pages() {
  let body = "\\b INVOICE\\b0\\par Invoice Number: INV-1001\\par Invoice Date: 03/14/2024\\par \\trowd\\cellx4320\\cellx7200\\pard\\intbl Description\\cell\\pard\\intbl Amount\\cell\\row\\trowd\\cellx4320\\cellx7200\\pard\\intbl Engine inspection\\cell\\pard\\intbl $1,234.56\\cell\\row\\pard Total: $1,234.56\\par";
  let rtf = Rtf::open(&document(body)).unwrap();
  assert_eq!(
    rtf.text(),
    "INVOICE\nInvoice Number: INV-1001\nInvoice Date: 03/14/2024\nDescription\tAmount\nEngine inspection\t$1,234.56\nTotal: $1,234.56"
  );
  let invoice = rtf.invoice(None);
  assert_eq!(invoice.source, contentdown::invoice::InvoiceSource::Native);
  assert_eq!(
    invoice
      .fields
      .invoice_id
      .as_ref()
      .map(|id| id.value.as_str()),
    Some("INV-1001")
  );
  assert_eq!(
    invoice
      .fields
      .invoice_date
      .as_ref()
      .map(|date| date.value.as_str()),
    Some("2024-03-14")
  );
  assert_eq!(
    invoice
      .fields
      .invoice_total
      .as_ref()
      .map(|total| total.value),
    Some(1234.56)
  );
}

#[test]
fn an_invoice_read_from_a_document_that_carries_a_file_says_so() {
  let carried = object("Paint.Picture", &object_data("Paint.Picture", b"some data"));
  let rtf = Rtf::open(&document(&format!("Please see attached.\\par {carried}"))).unwrap();
  assert_eq!(
    rtf.invoice(None).warnings.first().map(String::as_str),
    Some("the document carries 1 embedded file, which is not read here: the invoice may be in it")
  );
}

#[test]
fn a_paragraph_placed_on_the_page_is_set_where_the_file_puts_it_and_moves_nothing_else() {
  // Two placed on one line, as a report puts a label and its amount, and then a paragraph in
  // the flow, which starts at the top margin as though the placed ones were not there.
  let found = pages(
    "\\pard\\pvpg\\phpg\\posx1440\\posy2880\\absw2000 Total\\par\\pard\\pvpg\\phpg\\posx8000\\posy2880\\absw2000 5.00\\par\\pard In the flow\\par",
  );
  let [page] = &found[..] else {
    panic!("one page, and there are {}", found.len());
  };
  assert_eq!(lines(page), ["In the flow", "Total 5.00"]);
  let word = |text: &str| page.words.iter().find(|word| word.text == text).unwrap();
  // 1,440 twentieths of a point from the page's left side, and 2,880 down from its top.
  assert!(close(word("Total").x0, 72.0) && close(word("Total").y1, 792.0 - 144.0));
  assert!(close(word("5.00").x0, 400.0) && close(word("5.00").y1, 792.0 - 144.0));
  assert!(close(word("In").x0, 90.0) && close(word("In").y1, 720.0));
}

#[test]
fn a_page_of_placed_paragraphs_is_one_page() {
  // Forty placed down the page, a line apart: stacked in the flow they would run over.
  let body: String = (0..40)
    .map(|n| {
      format!(
        "\\pard\\pvpg\\phpg\\posx1440\\posy{}\\absw3000 Row {n}\\par ",
        1440 + n * 300
      )
    })
    .collect();
  let found = pages(&body);
  assert_eq!(found.len(), 1);
  assert_eq!(found[0].lines.len(), 40);
  assert_eq!(found[0].height, 792.0);
}

#[test]
fn paragraphs_are_as_far_apart_as_the_file_says_and_no_farther() {
  // Three of a line each: nothing stated between the first two, and 12 points after the
  // second with 6 before the third.
  let found = pages("One\\par\\pard\\sa240 Two\\par\\pard\\sb120 Three\\par");
  let word = |text: &str| {
    found[0]
      .words
      .iter()
      .find(|word| word.text == text)
      .unwrap()
  };
  // A line of 12-point text is 14.4 points tall, and its words 12 of that.
  assert!(close(word("One").y1, 720.0));
  assert!(close(word("Two").y1, 720.0 - 14.4));
  assert!(close(word("Three").y1, 720.0 - 14.4 - 14.4 - 12.0 - 6.0));
}

#[test]
fn paragraphs_that_go_on_in_one_frame_are_set_one_under_the_other() {
  // As a report writes an address: a place stated once, and a paragraph to a line after it.
  // The paragraph placed next has a frame of its own, though it is the same size.
  let found = pages(
    "\\pard\\pvpg\\phpg\\posx1440\\posy2880\\absw4000 First\\par Second\\par Third\\par\\pard\\pvpg\\phpg\\posx1440\\posy5760\\absw4000 Apart\\par Under\\par",
  );
  let [page] = &found[..] else {
    panic!("one page, and there are {}", found.len());
  };
  assert_eq!(lines(page), ["First", "Second", "Third", "Apart", "Under"]);
  let word = |text: &str| page.words.iter().find(|word| word.text == text).unwrap();
  for text in ["First", "Second", "Third", "Apart", "Under"] {
    assert!(
      close(word(text).x0, 72.0),
      "{text} starts at the frame's side"
    );
  }
  assert!(close(word("First").y1, 792.0 - 144.0));
  assert!(close(word("Second").y1, 792.0 - 144.0 - 14.4));
  assert!(close(word("Third").y1, 792.0 - 144.0 - 28.8));
  assert!(close(word("Apart").y1, 792.0 - 288.0));
  assert!(close(word("Under").y1, 792.0 - 288.0 - 14.4));
}

#[test]
fn a_frame_stated_again_in_the_same_words_goes_on_and_one_stated_otherwise_is_another() {
  // The format's own rule: paragraphs one after another with the same frame are one frame,
  // though the file starts over between them. A frame that differs in anything is another,
  // and here it is in the same spot, as a program that prints one thing over another writes.
  let found = pages(
    "\\pard\\pvpg\\phpg\\posx1440\\posy2880\\absw4000 First\\par\\pard\\pvpg\\phpg\\posx1440\\posy2880\\absw4000 Second\\par\\pard\\pvpg\\phpg\\posx1440\\posy2880\\absw3000 Over\\par",
  );
  let word = |text: &str| {
    found[0]
      .words
      .iter()
      .find(|word| word.text == text)
      .unwrap()
  };
  assert!(close(word("First").y1, 792.0 - 144.0));
  assert!(close(word("Second").y1, 792.0 - 144.0 - 14.4));
  assert!(close(word("Over").y1, word("First").y1));
}

#[test]
fn a_header_is_set_above_the_text_and_a_footer_below_it_on_every_page() {
  let found = pages("{\\header Head\\par}{\\footer Foot\\par}One\\par\\page Two\\par");
  assert_eq!(found.len(), 2);
  assert_eq!(lines(&found[0]), ["Head", "One", "Foot"]);
  assert_eq!(lines(&found[1]), ["Head", "Two", "Foot"]);
  for page in &found {
    let word = |text: &str| page.words.iter().find(|word| word.text == text).unwrap();
    // Half an inch from the top edge, and ending half an inch from the bottom one. The text
    // starts at the top margin as it would with no header.
    assert!(close(word("Head").y1, 792.0 - 36.0));
    assert!(close(word("Foot").y0, 36.0));
    assert_eq!(page.height, 792.0);
  }
  assert!(close(
    found[0].words.iter().find(|w| w.text == "One").unwrap().y1,
    720.0
  ));
  assert!(close(
    found[1].words.iter().find(|w| w.text == "Two").unwrap().y1,
    720.0
  ));
}

#[test]
fn the_first_page_takes_its_own_header_where_the_file_says_it_has_one() {
  let found = pages("\\titlepg{\\headerf Cover\\par}{\\header Rest\\par}One\\par\\page Two\\par");
  assert_eq!(lines(&found[0]), ["Cover", "One"]);
  assert_eq!(lines(&found[1]), ["Rest", "Two"]);
}

#[test]
fn a_header_taller_than_the_top_margin_moves_the_text_down() {
  // Six lines of 14.4 points from 36 down end 122.4 down, past the margin of 72.
  let head: String = (1..=6).map(|n| format!("Head {n}\\par ")).collect();
  let found = pages(&format!("{{\\header {head}}}Body\\par"));
  let body = found[0]
    .words
    .iter()
    .find(|word| word.text == "Body")
    .unwrap();
  assert!(close(body.y1, 792.0 - 122.4));
}

#[test]
fn text_that_turns_a_page_by_itself_starts_under_the_next_pages_header() {
  // Forty-five lines fill a page; the forty-sixth is the first of the second, under its header.
  let body: String = (1..=46).map(|n| format!("Line {n}\\par ")).collect();
  let found = pages(&format!("{{\\header Head\\par}}{body}"));
  assert_eq!(found.len(), 2);
  assert_eq!(lines(&found[1]), ["Head", "Line 46"]);
  let word = |text: &str| {
    found[1]
      .words
      .iter()
      .find(|word| word.text == text)
      .unwrap()
  };
  assert!(close(word("Head").y1, 792.0 - 36.0));
  assert!(close(word("46").y1, 720.0));
}
