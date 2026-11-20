use super::*;

fn read_body(body: &str) -> Body {
  read(format!("{{\\rtf1\\ansi\\deff0{{\\fonttbl{{\\f0\\fswiss Arial;}}{{\\f1\\froman\\fcharset204 Times New Roman;}}}}{body}}}").as_bytes())
}

fn paragraphs(body: &Body) -> Vec<&Paragraph> {
  body
    .blocks
    .iter()
    .filter_map(|block| match block {
      Block::Paragraph(paragraph) => Some(paragraph),
      _ => None,
    })
    .collect()
}

/// The text of a paragraph, with a tab for a tab and a line end for a line break.
fn plain(paragraph: &Paragraph) -> String {
  paragraph.text()
}

#[test]
fn text_is_read_in_paragraphs_and_runs_with_how_each_is_set() {
  let body =
    read_body("\\f0\\fs24 Hello \\b bold\\b0  and \\i slanted\\i0\\par\\f1\\fs20 Second\\par");
  let [first, second] = paragraphs(&body)[..] else {
    panic!("two paragraphs, and {} came out", body.blocks.len());
  };
  assert_eq!(plain(first), "Hello bold and slanted");
  let runs: Vec<(&str, f64, bool, bool, Option<&str>)> = first
    .inlines
    .iter()
    .filter_map(|inline| match inline {
      Inline::Text(run) => Some((
        run.text.as_str(),
        run.size,
        run.bold,
        run.italic,
        run.font.as_deref(),
      )),
      _ => None,
    })
    .collect();
  assert_eq!(
    runs,
    [
      ("Hello ", 12.0, false, false, Some("Arial")),
      ("bold", 12.0, true, false, Some("Arial")),
      (" and ", 12.0, false, false, Some("Arial")),
      ("slanted", 12.0, false, true, Some("Arial")),
    ]
  );
  let Inline::Text(run) = &second.inlines[0] else {
    panic!("a run");
  };
  assert_eq!(
    (run.text.as_str(), run.size, run.font.as_deref()),
    ("Second", 10.0, Some("Times New Roman"))
  );
}

#[test]
fn a_paragraph_with_no_end_mark_before_the_file_ends_is_still_a_paragraph() {
  assert_eq!(
    plain(paragraphs(&read_body("no end mark"))[0]),
    "no end mark"
  );
}

#[test]
fn escapes_and_named_characters_are_the_characters_they_stand_for() {
  // After a character given by its number comes what a reader that cannot show it is to show
  // in its place: as many characters as the last `\uc` said, one when none did.
  let body = read_body(
    "caf\\'e9 \\u8364? \\uc0\\u8364 x\\uc2\\u8364?? y \\uc1\\u-10179?\\u-8640? \\{a\\}\\\\ 1\\emdash 2\\endash 3 \\lquote q\\rquote  \\ldblquote d\\rdblquote  \\bullet  a\\~b\\_c\\-d\\par",
  );
  assert_eq!(
    plain(paragraphs(&body)[0]),
    "café € €x€ y 🙀 {a}\\ 1—2–3 ‘q’ “d” • a\u{a0}b-cd"
  );
}

#[test]
fn bytes_are_read_in_the_code_page_the_file_or_the_font_states() {
  // The file's own code page, Cyrillic.
  let cyrillic = read(b"{\\rtf1\\ansi\\ansicpg1251 \\'ef\\'f0\\'e8\\par}");
  assert_eq!(plain(paragraphs(&cyrillic)[0]), "при");
  // A font's character set wins over the file's code page: font 1 is Cyrillic here.
  let by_font = read_body("\\f1 \\'ef\\'f0\\'e8 \\f0 \\'e9\\par");
  assert_eq!(plain(paragraphs(&by_font)[0]), "при é");
  // Two bytes that make one character in a double-byte code page.
  let japanese = read(b"{\\rtf1\\ansi\\ansicpg932 \\'93\\'fa\\'96\\'7b\\par}");
  assert_eq!(plain(paragraphs(&japanese)[0]), "日本");
}

#[test]
fn what_is_not_the_documents_text_is_passed_over() {
  let body = read_body(
    "{\\colortbl;\\red0\\green0\\blue0;}{\\stylesheet{\\s0 Normal;}}{\\info{\\title A title}{\\author An author}}{\\*\\generator Some program;}{\\header In the header\\par}{\\footer In the footer\\par}Kept {\\field{\\*\\fldinst HYPERLINK \"http://example.com\"}{\\fldrslt shown}} {\\pict\\pngblip 89504e47} {\\*\\unknown not kept}{\\v hidden}end\\par",
  );
  assert_eq!(plain(paragraphs(&body)[0]), "Kept shown  end");
  assert_eq!(body.blocks.len(), 1);
}

#[test]
fn tabs_and_line_breaks_are_kept_as_what_they_are() {
  let body = read_body("Total\\tab 42.00\\line next line\\par");
  let kinds: Vec<&str> = paragraphs(&body)[0]
    .inlines
    .iter()
    .map(|inline| match inline {
      Inline::Text(_) => "text",
      Inline::Tab => "tab",
      Inline::LineBreak => "break",
    })
    .collect();
  assert_eq!(kinds, ["text", "tab", "text", "break", "text"]);
  assert_eq!(plain(paragraphs(&body)[0]), "Total\t42.00\nnext line");
}

#[test]
fn a_paragraph_keeps_how_it_is_lined_up_and_indented_until_it_is_reset() {
  let body =
    read_body("\\qr\\li720\\ri360 right\\par still right\\par\\pard plain\\par\\qc centered\\par");
  let found: Vec<(String, Align, f64, f64)> = paragraphs(&body)
    .iter()
    .map(|p| (plain(p), p.align, p.indent_left, p.indent_right))
    .collect();
  assert_eq!(
    found,
    [
      ("right".to_string(), Align::Right, 36.0, 18.0),
      ("still right".to_string(), Align::Right, 36.0, 18.0),
      ("plain".to_string(), Align::Left, 0.0, 0.0),
      ("centered".to_string(), Align::Center, 0.0, 0.0),
    ]
  );
}

#[test]
fn plain_puts_the_characters_back_as_they_start() {
  let body = read_body("\\b\\i\\fs40\\f1 loud\\plain  quiet\\par");
  let runs: Vec<(&str, f64, bool, bool)> = paragraphs(&body)[0]
    .inlines
    .iter()
    .filter_map(|inline| match inline {
      Inline::Text(run) => Some((run.text.as_str(), run.size, run.bold, run.italic)),
      _ => None,
    })
    .collect();
  assert_eq!(
    runs,
    [("loud", 20.0, true, true), (" quiet", 12.0, false, false)]
  );
}

#[test]
fn a_group_keeps_what_it_changes_to_itself() {
  let body = read_body("plain {\\b bold} plain again\\par");
  let bold: Vec<bool> = paragraphs(&body)[0]
    .inlines
    .iter()
    .filter_map(|inline| match inline {
      Inline::Text(run) => Some(run.bold),
      _ => None,
    })
    .collect();
  assert_eq!(bold, [false, true, false]);
}

#[test]
fn a_table_row_is_its_cells_with_the_edge_each_ends_at() {
  let body = read_body(
    "Before\\par\\trowd\\trleft-108\\cellx2880\\cellx5760\\pard\\intbl Item\\cell\\pard\\intbl\\qr 5.00\\par 6.00\\cell\\row\\pard After\\par",
  );
  let [
    Block::Paragraph(before),
    Block::Row(row),
    Block::Paragraph(after),
  ] = &body.blocks[..]
  else {
    panic!(
      "a paragraph, a row, and a paragraph, and {} blocks came out",
      body.blocks.len()
    );
  };
  assert_eq!(
    (plain(before), plain(after)),
    ("Before".to_string(), "After".to_string())
  );
  // Twentieths of a point: 2,880 is 144 points.
  assert_eq!(
    (row.left, row.edges.as_slice()),
    (-5.4, &[144.0, 288.0][..])
  );
  let cells: Vec<Vec<String>> = row
    .cells
    .iter()
    .map(|cell| cell.iter().map(plain).collect())
    .collect();
  assert_eq!(
    cells,
    [
      vec!["Item".to_string()],
      vec!["5.00".to_string(), "6.00".to_string()]
    ]
  );
  assert_eq!(row.cells[1][0].align, Align::Right);
}

#[test]
fn the_paper_and_its_margins_are_what_the_file_states_and_letter_when_it_states_none() {
  let stated =
    read(b"{\\rtf1\\paperw11906\\paperh16838\\margl1440\\margr1440\\margt720\\margb720 x\\par}");
  assert_eq!(
    stated.paper,
    Paper {
      width: 595.3,
      height: 841.9,
      left: 72.0,
      right: 72.0,
      top: 36.0,
      bottom: 36.0
    }
  );
  assert_eq!(
    read(b"{\\rtf1 x\\par}").paper,
    Paper {
      width: 612.0,
      height: 792.0,
      left: 90.0,
      right: 90.0,
      top: 72.0,
      bottom: 72.0
    }
  );
}

#[test]
fn a_page_break_is_a_block_of_its_own() {
  let body = read_body("One\\par\\page Two\\par");
  assert!(matches!(
    &body.blocks[..],
    [Block::Paragraph(_), Block::PageBreak, Block::Paragraph(_)]
  ));
}

#[test]
fn the_text_is_a_line_to_a_paragraph_and_to_a_row_with_tabs_between_its_cells() {
  let body = read_body(
    "Invoice 42\\par\\trowd\\cellx2880\\cellx5760\\intbl Item\\cell 5.00\\cell\\row\\pard Total\\tab 5.00\\par",
  );
  assert_eq!(body.text(), "Invoice 42\nItem\t5.00\nTotal\t5.00");
}

#[test]
fn a_paragraph_placed_on_the_page_keeps_the_place_the_file_gives_it() {
  // As a program that writes a report does it: every paragraph with a place and a width of
  // its own, measured from the page's corner, in twentieths of a point.
  let body = read_body(
    "\\pard\\pvpg\\phpg\\posx1440\\posy2880\\absw4320\\absh300 Placed\\par\\pard\\phmrg\\pvmrg\\posx200\\posy400 From the margins\\par\\pard Not placed\\par",
  );
  let frames: Vec<Option<Placing>> = paragraphs(&body).iter().map(|p| p.placing).collect();
  assert_eq!(
    frames,
    [
      Some(Placing {
        x: 72.0,
        y: 144.0,
        width: Some(216.0),
        from_page_side: true,
        from_page_top: true
      }),
      Some(Placing {
        x: 10.0,
        y: 20.0,
        width: None,
        from_page_side: false,
        from_page_top: false
      }),
      None,
    ]
  );
}

#[test]
fn the_space_a_file_states_before_and_after_a_paragraph_is_kept() {
  // Twentieths of a point, as every length in the file is. A paragraph that states none has
  // none, and the next keeps what the last stated until the file starts over.
  let body = read_body("\\pard\\sb120\\sa240 One\\par Two\\par\\pard Three\\par");
  let spaces: Vec<(f64, f64)> = paragraphs(&body)
    .iter()
    .map(|p| (p.space_before, p.space_after))
    .collect();
  assert_eq!(spaces, [(6.0, 12.0), (6.0, 12.0), (0.0, 0.0)]);
}

fn texts(blocks: &[Block]) -> Vec<String> {
  blocks
    .iter()
    .filter_map(|block| match block {
      Block::Paragraph(paragraph) => Some(paragraph.text()),
      _ => None,
    })
    .collect()
}

#[test]
fn a_header_and_a_footer_are_kept_apart_from_the_text() {
  let body = read_body(
    "{\\header\\pard\\qc Letterhead\\par Second line\\par}{\\footer\\pard\\qr Page one}Body\\par",
  );
  assert_eq!(texts(&body.blocks), ["Body"]);
  assert_eq!(texts(&body.running.header), ["Letterhead", "Second line"]);
  // A footer whose last paragraph is not ended is still read whole.
  assert_eq!(texts(&body.running.footer), ["Page one"]);
  assert_eq!(body.running.first_header, None);
  assert_eq!(body.running.first_footer, None);
  // How a header's paragraphs are lined up is the header's own, and not the text's after it.
  let aligned = |blocks: &[Block]| match blocks.first() {
    Some(Block::Paragraph(paragraph)) => paragraph.align,
    _ => panic!("a paragraph"),
  };
  assert_eq!(aligned(&body.running.header), Align::Center);
  assert_eq!(aligned(&body.blocks), Align::Left);
}

#[test]
fn a_first_page_has_a_header_of_its_own_only_where_the_file_says_it_has() {
  let stated = read_body("\\titlepg{\\headerf Cover\\par}{\\header Rest\\par}Body\\par");
  assert_eq!(texts(&stated.running.header), ["Rest"]);
  assert_eq!(
    stated.running.first_header.as_deref().map(texts),
    Some(vec!["Cover".to_string()])
  );
  // It says its first page is its own and gives no footer for it: the first page has none.
  assert_eq!(
    stated.running.first_footer.as_deref().map(texts),
    Some(vec![])
  );
  let unstated = read_body("{\\headerf Cover\\par}{\\header Rest\\par}Body\\par");
  assert_eq!(unstated.running.first_header, None);
  assert_eq!(texts(&unstated.running.header), ["Rest"]);
}

#[test]
fn the_header_stated_first_is_the_documents_and_one_for_left_pages_is_passed_over() {
  let body = read_body(
    "{\\headerl Left\\par}{\\headerr Right\\par}One\\sect\\sectd{\\header Later\\par}Two\\par",
  );
  assert_eq!(texts(&body.running.header), ["Right"]);
  assert_eq!(texts(&body.blocks), ["One", "Two"]);
}
