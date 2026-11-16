use contentdown::{PageLayout, TextWord};

use super::ESTIMATED;
use crate::model::Document;
use crate::testing::{Setup, document, p};

/// Letter paper with an inch of margin on every side: text is set from 72 to 540 points across
/// and starts 72 points down. At the default size a character is 5.5 points wide and a line
/// 13.2 points tall.
const LETTER: &str = r#"<w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"/>"#;

fn letter(body: &str) -> Document {
  document(
    &format!("{body}<w:sectPr>{LETTER}</w:sectPr>"),
    Setup::default(),
  )
}

fn one_page(body: &str) -> PageLayout {
  let mut pages = letter(body).layout();
  assert_eq!(pages.len(), 1);
  pages.remove(0)
}

fn lines(page: &PageLayout) -> Vec<&str> {
  page.lines.iter().map(|line| line.text.as_str()).collect()
}

fn word<'a>(page: &'a PageLayout, text: &str) -> &'a TextWord {
  page
    .words
    .iter()
    .find(|word| word.text == text)
    .unwrap_or_else(|| panic!("no word {text}"))
}

#[track_caller]
fn close(found: f64, expected: f64) {
  assert!((found - expected).abs() < 1e-6, "{found} is not {expected}");
}

#[test]
fn words_are_set_left_to_right_and_wrapped_at_the_margin() {
  let text = ["aaaa"; 20].join(" ");
  let page = one_page(&p(&text));
  assert_eq!((page.width, page.height), (612.0, 792.0));
  assert_eq!(page.warnings, [ESTIMATED]);
  // A word is 22 points wide and the next starts 27.5 further on: seventeen fit in 468.
  assert_eq!(
    lines(&page),
    [["aaaa"; 17].join(" "), ["aaaa"; 3].join(" ")]
  );
  let (first, second) = (&page.lines[0], &page.lines[1]);
  close(first.x0, 72.0);
  close(first.x1, 72.0 + 16.0 * 27.5 + 22.0);
  close(first.y1, 792.0 - 72.0);
  close(first.y0, 792.0 - 72.0 - 13.2);
  close(second.x0, 72.0);
  close(second.y1, first.y0);
  let third = &page.words[2];
  close(third.x0, 72.0 + 2.0 * 27.5);
  assert_eq!((third.font_size, &third.font_name), (Some(11.0), &None));
}

#[test]
fn a_word_that_runs_across_a_change_of_formatting_is_one_word() {
  let page = one_page(
    r#"<w:p><w:r><w:rPr><w:b/><w:rFonts w:ascii="Arial"/><w:sz w:val="40"/></w:rPr><w:t>In</w:t></w:r><w:r><w:t xml:space="preserve">voice 4711</w:t></w:r></w:p>"#,
  );
  assert_eq!(lines(&page), ["Invoice 4711"]);
  let invoice = word(&page, "Invoice");
  // It is set in the formatting it starts in.
  assert_eq!(
    (
      invoice.bold,
      invoice.font_size,
      invoice.font_name.as_deref()
    ),
    (Some(true), Some(20.0), Some("Arial"))
  );
  close(invoice.x1 - invoice.x0, 7.0 * 10.0);
  let number = word(&page, "4711");
  assert_eq!((number.bold, number.font_size), (Some(false), Some(11.0)));
  // Words of two sizes on one line stand on one baseline.
  close(number.y0, invoice.y0);
  close(invoice.y1 - invoice.y0, 24.0);
}

#[test]
fn a_tab_moves_to_the_next_stop_and_alignment_moves_the_line() {
  let aligned = |how: &str, text: &str| {
    format!(r#"<w:p><w:pPr><w:jc w:val="{how}"/></w:pPr><w:r><w:t>{text}</w:t></w:r></w:p>"#)
  };
  let page = one_page(
    &[
      "<w:p><w:r><w:t>No:</w:t><w:tab/><w:t>4711</w:t><w:tab/><w:tab/><w:t>far</w:t></w:r></w:p>"
        .to_owned(),
      aligned("right", "Total 25.50"),
      aligned("center", "Mid"),
    ]
    .concat(),
  );
  assert_eq!(lines(&page), ["No: 4711 far", "Total 25.50", "Mid"]);
  // Stops stand 36 points apart, from the left margin.
  close(word(&page, "No:").x0, 72.0);
  close(word(&page, "4711").x0, 108.0);
  close(word(&page, "far").x0, 180.0);
  close(word(&page, "25.50").x1, 540.0);
  close(word(&page, "Total").x0, 540.0 - 27.5 - 5.5 - 27.5);
  let mid = word(&page, "Mid");
  close((mid.x0 + mid.x1) / 2.0, 306.0);
}

#[test]
fn a_row_is_set_on_one_line_with_each_cell_in_its_column() {
  let cell =
    |properties: &str, text: &str| format!("<w:tc><w:tcPr>{properties}</w:tcPr>{}</w:tc>", p(text));
  let wide = ["aaaa"; 6].join(" ");
  let wider = ["bbbb"; 8].join(" ");
  let page = one_page(&format!(
    r#"<w:tbl><w:tblGrid><w:gridCol w:w="2880"/><w:gridCol w:w="2880"/><w:gridCol w:w="2880"/></w:tblGrid>
       <w:tr>{}{}{}</w:tr><w:tr>{}{}{}</w:tr><w:tr>{}{}</w:tr></w:tbl>{}"#,
    cell("", "A"),
    cell("", "B"),
    cell("", "C"),
    cell("", &wide),
    cell("", "x"),
    cell("", "y"),
    cell(r#"<w:gridSpan w:val="2"/>"#, &wider),
    cell("", "z"),
    p("after"),
  ));
  // Columns are 144 points wide, and text stands 5 points inside its cell. Five words fit in
  // the first column; the sixth wraps, and makes its row two lines tall.
  assert_eq!(
    lines(&page),
    [
      "A B C".to_owned(),
      format!("{} x y", ["aaaa"; 5].join(" ")),
      "aaaa".to_owned(),
      format!("{wider} z"),
      "after".to_owned(),
    ]
  );
  for (text, x0) in [
    ("A", 77.0),
    ("B", 221.0),
    ("C", 365.0),
    ("x", 221.0),
    ("z", 365.0),
  ] {
    close(word(&page, text).x0, x0);
  }
  // A cell that spans two columns is set across both.
  let spanning = &page.lines[3];
  close(spanning.x0, 77.0);
  assert!(page.words[spanning.first_word as usize + 7].x1 > 216.0);
  // Each row starts under the tallest cell of the row above it.
  let tops: Vec<f64> = page.lines.iter().map(|line| 792.0 - line.y1).collect();
  close(tops[0], 72.0);
  close(tops[1], 72.0 + 13.2 + 7.7);
  close(tops[2], tops[1] + 13.2);
  close(tops[3], tops[1] + 2.0 * 13.2 + 7.7);
}

#[test]
fn a_table_whose_grid_is_a_placeholder_is_sized_by_what_its_cells_hold() {
  let page = one_page(&format!(
    r#"<w:tbl><w:tblGrid><w:gridCol w:w="81"/><w:gridCol w:w="171"/></w:tblGrid>
       <w:tr><w:tc>{}</w:tc><w:tc>{}</w:tc></w:tr><w:tr><w:tc>{}</w:tc><w:tc>{}</w:tc></w:tr></w:tbl>"#,
    p("Name"),
    p("A much longer value"),
    p("No"),
    p("7"),
  ));
  assert_eq!(lines(&page), ["Name A much longer value", "No 7"]);
  // The first column holds four characters and its padding; the second starts after it.
  close(word(&page, "A").x0, 72.0 + (10.0 + 4.0 * 5.5) + 5.0);
  close(word(&page, "7").x0, word(&page, "A").x0);
}

const PARTS: Setup<'static> = Setup {
  styles: None,
  numbering: None,
  rels: &[
    ("h1", "word/header1.xml", false),
    ("h2", "word/header2.xml", false),
    ("f1", "word/footer1.xml", false),
  ],
  view: crate::View::Final,
  headers: &[
    (
      "word/header1.xml",
      "<w:p><w:r><w:t>Running head</w:t></w:r></w:p>",
    ),
    (
      "word/header2.xml",
      "<w:p><w:r><w:t>Title head</w:t></w:r></w:p>",
    ),
  ],
  footers: &[(
    "word/footer1.xml",
    "<w:p><w:r><w:t>Page foot</w:t></w:r></w:p>",
  )],
  footnotes: &[],
  comments: &[],
};

const PAGE_BREAK: &str = r#"<w:p><w:r><w:br w:type="page"/></w:r></w:p>"#;

#[test]
fn a_page_has_its_header_above_and_its_footer_below_and_a_title_page_has_its_own() {
  let body = format!(
    r#"{}{PAGE_BREAK}{}<w:sectPr><w:headerReference w:type="default" r:id="h1"/><w:headerReference w:type="first" r:id="h2"/><w:footerReference w:type="default" r:id="f1"/><w:titlePg/>{LETTER}</w:sectPr>"#,
    p("first page"),
    p("second page"),
  );
  let pages = document(&body, PARTS).layout();
  assert_eq!(pages.len(), 2);
  // The title page has the header made for it, and no footer, since none was made for it.
  assert_eq!(lines(&pages[0]), ["Title head", "first page"]);
  assert_eq!(
    lines(&pages[1]),
    ["Running head", "second page", "Page foot"]
  );
  for page in &pages {
    assert_eq!(page.height, 792.0);
    assert_eq!(page.warnings, [ESTIMATED]);
    // The header starts half an inch down; the body starts at the margin whatever is above it.
    close(page.lines[0].y1, 792.0 - 36.0);
    close(page.lines[1].y1, 792.0 - 72.0);
  }
  // The footer ends half an inch above the bottom edge, the space after its paragraph included.
  let foot = &pages[1].lines[2];
  close(foot.y0, 36.0 + 7.7);
  close(foot.y1, 36.0 + 7.7 + 13.2);
}

#[test]
fn a_section_that_names_no_header_uses_the_one_before_it() {
  let body = format!(
    r#"{}<w:p><w:pPr><w:sectPr><w:headerReference w:type="default" r:id="h1"/>{LETTER}</w:sectPr></w:pPr></w:p>{}<w:sectPr>{LETTER}</w:sectPr>"#,
    p("one"),
    p("two"),
  );
  let document = document(&body, PARTS);
  let defaults: Vec<Option<u32>> = document
    .sections
    .iter()
    .map(|s| s.headers.default)
    .collect();
  assert_eq!(defaults, [Some(0), Some(0)]);
  assert_eq!(document.sections[1].footers.default, None);
  let pages = document.layout();
  assert_eq!(lines(&pages[0]), ["Running head", "one"]);
  assert_eq!(lines(&pages[1]), ["Running head", "two"]);
}

#[test]
fn a_page_is_as_tall_as_what_is_set_on_it_and_its_footer_goes_under_the_body() {
  let body = format!(
    r#"{}<w:sectPr><w:footerReference w:type="default" r:id="f1"/>{LETTER}</w:sectPr>"#,
    p("x").repeat(80),
  );
  let pages = document(&body, PARTS).layout();
  assert_eq!(pages.len(), 1);
  let page = &pages[0];
  // Eighty paragraphs of a line and the space after it, from 72 points down; then the footer,
  // and half an inch under it.
  let body_end = 72.0 + 80.0 * (13.2 + 7.7);
  close(page.height, body_end + 13.2 + 7.7 + 36.0);
  assert_eq!(page.warnings.len(), 2);
  assert!(
    page.warnings[1].contains("run past the paper"),
    "{:?}",
    page.warnings
  );
  assert_eq!(page.lines.len(), 81);
  let foot = page.lines.last().unwrap();
  assert_eq!(foot.text, "Page foot");
  close(page.height - foot.y1, body_end);
  assert!(
    page
      .words
      .iter()
      .all(|w| w.y0 >= 0.0 && w.y1 <= page.height)
  );
}

const LISTS: &str = r#"<w:abstractNum w:abstractNumId="0">
    <w:lvl w:ilvl="0"><w:numFmt w:val="decimal"/><w:lvlText w:val="%1."/></w:lvl>
    <w:lvl w:ilvl="1"><w:numFmt w:val="bullet"/><w:lvlText w:val="-"/></w:lvl>
  </w:abstractNum>
  <w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num>"#;

#[test]
fn a_list_item_starts_with_its_marker_and_is_set_further_in_with_each_level() {
  let item = |level: u8, text: &str| {
    format!(
      r#"<w:p><w:pPr><w:numPr><w:ilvl w:val="{level}"/><w:numId w:val="1"/></w:numPr></w:pPr><w:r><w:t>{text}</w:t></w:r></w:p>"#
    )
  };
  let body = format!(
    "{}{}<w:sectPr>{LETTER}</w:sectPr>",
    item(0, "first"),
    item(1, "inner")
  );
  let mut pages = document(
    &body,
    Setup {
      numbering: Some(LISTS),
      ..Setup::default()
    },
  )
  .layout();
  let page = pages.remove(0);
  assert_eq!(lines(&page), ["1. first", "- inner"]);
  close(word(&page, "1.").x0, 72.0);
  close(word(&page, "first").x0, 72.0 + 11.0 + 5.5);
  close(word(&page, "-").x0, 90.0);
}

#[test]
fn a_document_that_states_no_page_is_set_on_letter_with_inch_margins() {
  let pages = document(&p("alone"), Setup::default()).layout();
  assert_eq!(pages.len(), 1);
  assert_eq!((pages[0].width, pages[0].height), (612.0, 792.0));
  let alone = &pages[0].words[0];
  close(alone.x0, 72.0);
  close(alone.y1, 720.0);
  // A document with nothing in it is still one page.
  let empty = document("", Setup::default()).layout();
  assert_eq!(empty.len(), 1);
  assert!(empty[0].words.is_empty());
}

#[test]
fn pages_follow_the_breaks_and_a_table_is_split_between_its_rows() {
  let marked = r#"<w:p><w:r><w:lastRenderedPageBreak/><w:t>marked</w:t></w:r></w:p>"#;
  let body = format!(
    "{}<w:tbl><w:tr><w:tc>{}</w:tc></w:tr><w:tr><w:tc>{marked}</w:tc></w:tr></w:tbl>{PAGE_BREAK}{PAGE_BREAK}{}",
    p("top"),
    p("row one"),
    p("last"),
  );
  let document = letter(&body);
  let pages = document.layout();
  // The second break follows the first with no text between: a page with nothing on it.
  assert_eq!(document.page_count, 4);
  let texts: Vec<Vec<&str>> = pages.iter().map(lines).collect();
  assert_eq!(
    texts,
    [vec!["top", "row one"], vec!["marked"], vec![], vec!["last"]]
  );
  assert_eq!(
    pages.iter().map(|page| page.page).collect::<Vec<_>>(),
    [1, 2, 3, 4]
  );
  // What a page starts with starts at its top margin.
  close(pages[1].lines[0].y1, 720.0);
  close(pages[3].lines[0].y1, 720.0);
  // A paragraph that holds only a break is an empty line on the page the break ends.
  assert_eq!(
    document.text_per_page(),
    ["top\nrow one", "marked\n", "", "last"]
  );
}
