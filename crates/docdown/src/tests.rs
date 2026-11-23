use contentdown::document::{Align, Block, DownFrom, Inline, Paragraph, Run};

use super::*;
use crate::testing::*;

fn opened(spec: DocSpec) -> Doc {
  Doc::open(&spec.file()).unwrap()
}

fn paragraphs(doc: &Doc) -> Vec<&Paragraph> {
  doc
    .blocks()
    .iter()
    .filter_map(|block| match block {
      Block::Paragraph(paragraph) => Some(paragraph),
      _ => None,
    })
    .collect()
}

fn runs(paragraph: &Paragraph) -> Vec<&Run> {
  paragraph
    .inlines
    .iter()
    .filter_map(|inline| match inline {
      Inline::Text(run) => Some(run),
      _ => None,
    })
    .collect()
}

fn refusal(file: &[u8]) -> String {
  Doc::open(file)
    .err()
    .map(|error| error.to_string())
    .unwrap()
}

#[test]
fn the_text_is_read_through_its_pieces_each_as_it_is_kept() {
  let doc = opened(
    DocSpec::new()
      .text("Caf\u{e9} \u{201c}one\u{201d}\r")
      .wide("\u{65e5}\u{672c} two\r")
      .text("three\r"),
  );
  assert_eq!(
    doc.text(),
    "Caf\u{e9} \u{201c}one\u{201d}\n\u{65e5}\u{672c} two\nthree"
  );
  assert!(doc.warnings().is_empty(), "{:?}", doc.warnings());
}

#[test]
fn a_paragraph_ends_at_its_mark_and_keeps_its_tabs_and_line_breaks() {
  let doc = opened(DocSpec::new().text("A\rB\u{b}C\tD\r"));
  let found = paragraphs(&doc);
  assert_eq!(found.len(), 2);
  assert_eq!(found[0].text(), "A");
  assert_eq!(found[1].text(), "B\nC\tD");
  assert!(matches!(
    found[1].inlines[..],
    [
      Inline::Text(_),
      Inline::LineBreak,
      Inline::Text(_),
      Inline::Tab,
      Inline::Text(_)
    ]
  ));
}

#[test]
fn a_page_break_stands_between_the_paragraphs_it_parts() {
  let doc = opened(DocSpec::new().text("One\r\u{c}Two\r"));
  assert!(matches!(
    doc.blocks(),
    [Block::Paragraph(_), Block::PageBreak, Block::Paragraph(_)]
  ));
  assert_eq!(doc.text(), "One\nTwo");
}

#[test]
fn a_field_keeps_what_it_shows_and_drops_what_it_is_told() {
  let doc = opened(DocSpec::new().text(
    "Total: \u{13} =SUM(ABOVE) \u{14}42\u{15} due\r\u{13} IF \u{13} PAGE \u{14}2\u{15} = 2 \u{14}yes\u{15}\r\u{13} TC \"entry\" \u{15}x\r",
  ));
  assert_eq!(doc.text(), "Total: 42 due\nyes\nx");
}

#[test]
fn marks_that_stand_for_a_picture_a_drawing_or_a_note_are_not_text() {
  let doc = opened(DocSpec::new().text("a\u{1}b\u{8}c\u{2}d\u{1f}e\u{1e}f\r"));
  // An optional hyphen is not shown, and one that may not break is a hyphen.
  assert_eq!(doc.text(), "abcde\u{2011}f");
}

#[test]
fn the_rows_of_a_table_end_at_their_marks_with_the_edges_the_file_gives() {
  let in_table = byte(0x2416, 1);
  let row_end = |edges: &[i16]| vec![byte(0x2416, 1), byte(0x2417, 1), row_edges(edges)];
  let doc = opened(
    DocSpec::new()
      .text("Item\u{7}Price\u{7}\u{7}Bolt\ra long one\u{7}5.00\u{7}\u{7}After\r")
      .paragraph(0, std::slice::from_ref(&in_table))
      .paragraph(1, std::slice::from_ref(&in_table))
      .paragraph(2, &row_end(&[-100, 2880, 5760]))
      .paragraph(3, std::slice::from_ref(&in_table))
      .paragraph(4, std::slice::from_ref(&in_table))
      .paragraph(5, std::slice::from_ref(&in_table))
      .paragraph(6, &row_end(&[0, 1440, 2880])),
  );
  let [
    Block::Row(first),
    Block::Row(second),
    Block::Paragraph(after),
  ] = doc.blocks()
  else {
    panic!("{:?}", doc.blocks());
  };
  assert_eq!((first.left, &first.edges[..]), (-5.0, &[144.0, 288.0][..]));
  assert_eq!(first.cells.len(), 2);
  assert_eq!(first.cells[0][0].text(), "Item");
  assert_eq!(first.cells[1][0].text(), "Price");
  assert_eq!((second.left, &second.edges[..]), (0.0, &[72.0, 144.0][..]));
  // A cell may hold more than one paragraph.
  let held: Vec<String> = second.cells[0].iter().map(Paragraph::text).collect();
  assert_eq!(held, ["Bolt", "a long one"]);
  assert_eq!(second.cells[1][0].text(), "5.00");
  assert_eq!(after.text(), "After");
  assert_eq!(doc.text(), "Item\tPrice\nBolt a long one\t5.00\nAfter");
}

#[test]
fn a_paragraph_is_lined_up_indented_and_spaced_as_the_file_states() {
  let doc = opened(
    DocSpec::new()
      .text("left\rcenter\rright\rboth\r")
      .paragraph(1, &[byte(0x2403, 1), word(0x840F, 720), word(0x840E, 360)])
      .paragraph(2, &[byte(0x2461, 2), word(0xA413, 240), word(0xA414, 120)])
      .paragraph(3, &[byte(0x2403, 3), word(0x845E, 1440), word(0x845D, 20)]),
  );
  let found = paragraphs(&doc);
  let stated: Vec<(Align, f64, f64, f64, f64)> = found
    .iter()
    .map(|p| {
      (
        p.align,
        p.indent_left,
        p.indent_right,
        p.space_before,
        p.space_after,
      )
    })
    .collect();
  assert_eq!(
    stated,
    [
      (Align::Left, 0.0, 0.0, 0.0, 0.0),
      (Align::Center, 36.0, 18.0, 0.0, 0.0),
      (Align::Right, 0.0, 0.0, 12.0, 6.0),
      (Align::Justified, 72.0, 1.0, 0.0, 0.0)
    ]
  );
}

#[test]
fn runs_are_set_as_their_properties_state() {
  let doc = opened(
    DocSpec::new()
      .text("plain bold slanted big\r")
      .fonts(&["Times New Roman", "Arial"])
      .run(6, 10, &[byte(0x0835, 1)])
      .run(11, 18, &[byte(0x0836, 1), word(0x4A4F, 1)])
      .run(19, 22, &[word(0x4A43, 48)]),
  );
  let found = paragraphs(&doc);
  let set: Vec<(&str, f64, bool, bool, Option<&str>)> = runs(found[0])
    .iter()
    .map(|run| {
      (
        run.text.as_str(),
        run.size,
        run.bold,
        run.italic,
        run.font.as_deref(),
      )
    })
    .collect();
  assert_eq!(
    set,
    [
      ("plain ", 10.0, false, false, Some("Times New Roman")),
      ("bold", 10.0, true, false, Some("Times New Roman")),
      (" ", 10.0, false, false, Some("Times New Roman")),
      ("slanted", 10.0, false, true, Some("Arial")),
      (" ", 10.0, false, false, Some("Times New Roman")),
      ("big", 24.0, false, false, Some("Times New Roman")),
      // The paragraph's mark is not text.
    ]
  );
}

#[test]
fn text_that_is_hidden_is_left_out() {
  let doc = opened(
    DocSpec::new()
      .text("shown hidden shown\r")
      .run(6, 13, &[byte(0x083C, 1)]),
  );
  assert_eq!(doc.text(), "shown shown");
}

#[test]
fn a_style_sets_what_a_paragraph_and_its_runs_do_not_state() {
  let normal = Style {
    name: "Normal",
    based_on: 0x0FFF,
    paragraph: Vec::new(),
    characters: word(0x4A43, 24),
  };
  let heading = Style {
    name: "Heading 1",
    based_on: 0,
    paragraph: [byte(0x2403, 1), word(0xA413, 240)].concat(),
    characters: [byte(0x0835, 1), word(0x4A43, 32)].concat(),
  };
  let doc = opened(
    DocSpec::new()
      .text("Title here\rbody text\rSmall\r")
      .style(normal)
      .style(heading)
      .styled(0, 1, &[])
      // In a bold style, one value is "as the style has it" and another "the other way".
      .run(0, 5, &[byte(0x0835, 0x80)])
      .run(6, 10, &[byte(0x0835, 0x81)])
      .styled(2, 1, &[byte(0x2403, 2)])
      .run(21, 26, &[word(0x4A43, 16), byte(0x0835, 0)]),
  );
  let found = paragraphs(&doc);
  assert_eq!(
    (found[0].align, found[0].space_before),
    (Align::Center, 12.0)
  );
  let title: Vec<(&str, f64, bool)> = runs(found[0])
    .iter()
    .map(|run| (run.text.as_str(), run.size, run.bold))
    .collect();
  // Text set one way is one run, however many stretches the file states it in.
  assert_eq!(title, [("Title ", 16.0, true), ("here", 16.0, false)]);
  let body = runs(found[1]);
  assert_eq!(
    (body[0].size, body[0].bold, found[1].align),
    (12.0, false, Align::Left)
  );
  // What the paragraph states wins over its style, and what the run states over both.
  let small = runs(found[2]);
  assert_eq!(
    (found[2].align, small[0].size, small[0].bold),
    (Align::Right, 8.0, false)
  );
}

#[test]
fn the_paper_and_its_margins_are_the_first_section_s() {
  let stated = opened(DocSpec::new().text("x\r").section(&[
    word(0xB01F, 11906),
    word(0xB020, 16838),
    word(0xB021, 1134),
    word(0xB022, 567),
    word(0x9023, 850),
    word(0x9024, 1417),
  ]));
  let paper = stated.paper();
  assert_eq!(
    (
      paper.width,
      paper.height,
      paper.left,
      paper.right,
      paper.top,
      paper.bottom
    ),
    (595.3, 841.9, 56.7, 28.35, 42.5, 70.85)
  );
  // A file that states none is on letter paper, with the margins Word gives a new document.
  let unstated = opened(DocSpec::new().text("x\r")).paper();
  assert_eq!(
    (
      unstated.width,
      unstated.height,
      unstated.left,
      unstated.right,
      unstated.top,
      unstated.bottom
    ),
    (612.0, 792.0, 90.0, 90.0, 72.0, 72.0)
  );
}

#[test]
fn the_headers_and_footers_are_read_apart_from_the_text() {
  // Six notes on how footnotes are parted come first, and then for the section its header
  // and footer for even pages, for odd pages, and for the first page.
  let story =
    "Even head\rOdd head\rEven foot\rPage \u{13} PAGE \u{14}1\u{15}\rFirst head\rFirst foot\r\r";
  let starts = [0, 0, 0, 0, 0, 0, 0, 10, 19, 29, 45, 56, 67, 68];
  let plain = opened(DocSpec::new().text("Body\r").running(story, &starts));
  assert_eq!(plain.text(), "Body");
  let running = plain.running();
  let text = |blocks: &[Block]| -> Vec<String> {
    blocks
      .iter()
      .filter_map(|block| match block {
        Block::Paragraph(paragraph) => Some(paragraph.text()),
        _ => None,
      })
      .collect()
  };
  assert_eq!(text(&running.header), ["Odd head"]);
  assert_eq!(text(&running.footer), ["Page 1"]);
  // The first page has its own only where the section says it does.
  assert_eq!(
    (&running.first_header, &running.first_footer),
    (&None, &None)
  );

  let titled = opened(
    DocSpec::new()
      .text("Body\r")
      .running(story, &starts)
      .section(&[byte(0x300A, 1)]),
  );
  let running = titled.running();
  assert_eq!(
    running.first_header.as_deref().map(text),
    Some(vec!["First head".to_string()])
  );
  assert_eq!(
    running.first_footer.as_deref().map(text),
    Some(vec!["First foot".to_string()])
  );
}

#[test]
fn the_document_is_set_on_pages_and_an_invoice_is_read_from_them() {
  let doc = opened(DocSpec::new().text(
    "Acme Tooling Ltd\rInvoice Number: INV-2026-0042\rInvoice Date: 2026-11-23\rTotal Due: 1,250.00\r\u{c}Terms\r",
  ));
  let pages = doc.layout();
  assert_eq!(pages.len(), 2);
  assert_eq!(pages[0].lines[0].text, "Acme Tooling Ltd");
  assert_eq!(
    pages[0].warnings,
    [
      "word positions are estimated from the flow of the text; a Word file of this kind records no layout"
    ]
  );
  let invoice = doc.invoice(None);
  assert_eq!(
    invoice
      .fields
      .invoice_id
      .as_ref()
      .map(|field| field.value.as_str()),
    Some("INV-2026-0042")
  );
}

#[test]
fn a_template_and_a_document_with_macros_say_what_they_are() {
  let plain = opened(DocSpec::new().text("x\r"));
  assert!(!plain.is_template() && !plain.has_macros());
  assert!(opened(DocSpec::new().text("x\r").flags(0x0001)).is_template());
  let (main, table) = DocSpec::new().text("x\r").streams();
  let with = cfbdown::compound(&[
    ("WordDocument", &main),
    ("1Table", &table),
    ("Macros/VBA/dir", b"d"),
  ])
  .unwrap();
  assert!(Doc::open(&with).unwrap().has_macros());
}

#[test]
fn what_is_not_a_document_of_word_97_to_2003_is_refused_and_named() {
  assert_eq!(
    refusal(b"{\\rtf1 not this}"),
    "malformed document: not a compound file"
  );
  let workbook = cfbdown::compound(&[("Workbook", b"records")]).unwrap();
  assert_eq!(
    refusal(&workbook),
    "unsupported format: a compound file that holds no Word document"
  );
  assert_eq!(
    refusal(&DocSpec::new().text("x\r").version(0xA5DC, 0x0068).file()),
    "unsupported format: a Word 6 or 95 document"
  );
  assert_eq!(
    refusal(&DocSpec::new().text("x\r").flags(0x0100).file()),
    "unsupported format: an encrypted document"
  );
  let (main, _) = DocSpec::new().text("x\r").streams();
  let no_table = cfbdown::compound(&[("WordDocument", &main)]).unwrap();
  assert_eq!(
    refusal(&no_table),
    "malformed document: the stream that describes the text is missing"
  );
  let short = cfbdown::compound(&[("WordDocument", &main[..40]), ("1Table", b"t")]).unwrap();
  assert_eq!(
    refusal(&short),
    "malformed document: the document's opening block cannot be read"
  );
  let (main, table) = DocSpec::new().text("x\r").streams();
  let no_pieces = cfbdown::compound(&[("WordDocument", &main), ("1Table", &table[..3])]).unwrap();
  assert_eq!(
    refusal(&no_pieces),
    "malformed document: the table of the text's pieces cannot be read"
  );
}

#[test]
fn a_piece_that_lies_outside_the_file_is_left_out_and_said_to_be() {
  let (main, mut table) = DocSpec::new().text("kept\r").text("lost\r").streams();
  // The table of pieces opens the table stream: a mark, a length, three places, and then for
  // each piece two bytes and where it is kept. The second is sent past the end of the file.
  let second = 1 + 4 + 12 + 8 + 2;
  table[second..second + 4].copy_from_slice(&0x7FFF_0000u32.to_le_bytes());
  let file = cfbdown::compound(&[("WordDocument", &main), ("1Table", &table)]).unwrap();
  let doc = Doc::open(&file).unwrap();
  assert_eq!(doc.text(), "kept");
  assert_eq!(
    doc.warnings(),
    ["a piece of the text lies outside the file and is left out"]
  );
}

#[test]
fn text_in_a_text_box_is_placed_where_its_shape_is_anchored() {
  let doc = opened(
    DocSpec::new()
      .text("Before\r\u{8}Anchor\r\u{8}After\r")
      .boxes("Boxed one\rBoxed two\rOn the page\r\r", &[0, 20, 32, 33])
      // Measured across from the column and down from the paragraph it is anchored in.
      .shape(Shape {
        at: 7,
        id: 1025,
        edges: (1440, 720, 4320, 2160),
        flags: 0x0014,
        text_box: Some(1),
      })
      // Measured from the page, both ways.
      .shape(Shape {
        at: 15,
        id: 1026,
        edges: (720, 1440, 2160, 2880),
        flags: 0x000A,
        text_box: Some(2),
      }),
  );
  assert_eq!(
    doc.text(),
    "Before\nBoxed one\nBoxed two\nAnchor\nOn the page\nAfter"
  );
  let found = paragraphs(&doc);
  assert_eq!(found.len(), 6);
  let close = |a: f64, b: f64| (a - b).abs() < 1e-9;
  // A box's text stands in from its edges: a tenth of an inch at the sides, and half that
  // above.
  let first = found[1]
    .placing
    .expect("the box's first paragraph is placed");
  assert!(close(first.x, 79.2) && close(first.y, 39.6), "{first:?}");
  assert!(close(first.width.unwrap(), 129.6), "{first:?}");
  assert_eq!(
    (first.from_page_side, first.down_from),
    (false, DownFrom::Text)
  );
  assert!(!found[1].continues_frame);
  // The paragraphs after the first go on in the same box.
  assert_eq!(found[2].placing, Some(first));
  assert!(found[2].continues_frame);
  assert_eq!(
    (found[0].placing, found[3].placing, found[5].placing),
    (None, None, None)
  );
  let second = found[4].placing.expect("the second box is placed");
  assert_eq!(
    (second.from_page_side, second.down_from),
    (true, DownFrom::PageTop)
  );
  assert!(close(second.x, 43.2) && close(second.y, 75.6), "{second:?}");
}

#[test]
fn a_shape_with_no_text_puts_nothing_in_the_text() {
  let doc = opened(DocSpec::new().text("A\u{8}B\r").shape(Shape {
    at: 1,
    id: 1025,
    edges: (0, 0, 1440, 1440),
    flags: 0x0014,
    text_box: None,
  }));
  assert_eq!(doc.text(), "AB");
  assert_eq!(doc.blocks().len(), 1);
}

#[test]
fn a_text_box_that_no_shape_leads_to_is_read_after_the_text() {
  let doc = opened(
    DocSpec::new()
      .text("Body\r")
      .boxes("Loose one\rLoose two\r\r", &[0, 10, 20, 21]),
  );
  assert_eq!(doc.text(), "Body\nLoose one\nLoose two");
  assert!(
    paragraphs(&doc)
      .iter()
      .all(|paragraph| paragraph.placing.is_none())
  );
}

/// A document with these streams beside its own.
fn carrying(extra: &[(&str, &[u8])]) -> Doc {
  let (main, table) = DocSpec::new().text("See the attached.\r").streams();
  let mut streams: Vec<(&str, &[u8])> = vec![("WordDocument", &main), ("1Table", &table)];
  streams.extend(extra);
  Doc::open(&cfbdown::compound(&streams).unwrap()).unwrap()
}

/// What an object says of itself: the name a person sees, and then the name programs use.
fn says(shown: &str, class: &str) -> Vec<u8> {
  let counted = |text: &str| {
    let mut out = u32::try_from(text.len() + 1)
      .unwrap()
      .to_le_bytes()
      .to_vec();
    out.extend(text.as_bytes());
    out.push(0);
    out
  };
  let mut out = vec![0u8; 28];
  out.extend(counted(shown));
  out.extend(0u32.to_le_bytes());
  out.extend(counted(class));
  out
}

/// A file as the packager keeps it, with the length that goes before it in its stream.
fn packed(name: &str, bytes: &[u8]) -> Vec<u8> {
  let mut data = vec![2, 0];
  data.extend(name.as_bytes());
  data.extend(b"\0C:\\from\\it\0");
  data.extend(3u32.to_le_bytes());
  data.extend(9u32.to_le_bytes());
  data.extend(b"C:\\to\\it\0");
  data.extend(u32::try_from(bytes.len()).unwrap().to_le_bytes());
  data.extend(bytes);
  let mut out = u32::try_from(data.len()).unwrap().to_le_bytes().to_vec();
  out.extend(data);
  out
}

#[test]
fn a_file_dropped_into_a_document_is_given_as_it_was_with_its_name() {
  let doc = carrying(&[
    ("ObjectPool/_1000/\u{1}CompObj", &says("Package", "Package")),
    (
      "ObjectPool/_1000/\u{1}Ole10Native",
      &packed("invoice.pdf", b"%PDF-1.7 the file"),
    ),
    ("ObjectPool/_1000/\u{3}ObjInfo", &[0; 6]),
  ]);
  let [file] = doc.embedded() else {
    panic!("{:?}", doc.embedded());
  };
  assert_eq!(file.class.as_deref(), Some("Package"));
  assert_eq!(file.name.as_deref(), Some("invoice.pdf"));
  assert_eq!(file.bytes, b"%PDF-1.7 the file");
}

#[test]
fn a_file_held_whole_in_a_stream_is_given_from_it() {
  let doc = carrying(&[
    (
      "ObjectPool/_2/\u{1}CompObj",
      &says("Acrobat Document", "AcroExch.Document.7"),
    ),
    ("ObjectPool/_2/CONTENTS", b"%PDF-1.4 held"),
    (
      "ObjectPool/_1/\u{1}CompObj",
      &says("Microsoft Word Document", "Word.Document.12"),
    ),
    ("ObjectPool/_1/Package", b"PK\x03\x04 a document of 2007"),
  ]);
  // In the order of their names, which is the order they were put in.
  let found: Vec<(Option<&str>, &[u8])> = doc
    .embedded()
    .iter()
    .map(|file| (file.class.as_deref(), &file.bytes[..]))
    .collect();
  assert_eq!(
    found,
    [
      (
        Some("Word.Document.12"),
        &b"PK\x03\x04 a document of 2007"[..]
      ),
      (Some("AcroExch.Document.7"), &b"%PDF-1.4 held"[..])
    ]
  );
  assert!(doc.embedded().iter().all(|file| file.name.is_none()));
}

#[test]
fn an_embedded_workbook_is_given_as_a_file_of_its_own() {
  let records = vec![9u8; 5000];
  let doc = carrying(&[
    (
      "ObjectPool/_7/\u{1}CompObj",
      &says("Microsoft Excel Worksheet", "Excel.Sheet.8"),
    ),
    ("ObjectPool/_7/Workbook", &records),
    ("ObjectPool/_7/\u{5}SummaryInformation", b"about it"),
  ]);
  let [file] = doc.embedded() else {
    panic!("{:?}", doc.embedded());
  };
  assert_eq!(file.class.as_deref(), Some("Excel.Sheet.8"));
  // The storage it was kept in is a compound file again, with the workbook in it.
  let workbook = cfbdown::CompoundFile::open(&file.bytes).unwrap();
  assert_eq!(workbook.stream("Workbook"), Some(records));
  assert_eq!(
    workbook.stream("\u{5}SummaryInformation").as_deref(),
    Some(&b"about it"[..])
  );
}

#[test]
fn a_document_that_carries_files_says_so_with_its_invoice() {
  let none = carrying(&[]);
  assert!(none.embedded().is_empty());
  assert!(
    none
      .invoice(None)
      .warnings
      .iter()
      .all(|warning| !warning.contains("embedded"))
  );
  let one = carrying(&[("ObjectPool/_1/CONTENTS", b"%PDF-1.4")]);
  assert_eq!(
    one.invoice(None).warnings.first().map(String::as_str),
    Some("the document carries 1 embedded file, which is not read here: the invoice may be in it")
  );
  // A storage that holds nothing is no file.
  let (main, table) = DocSpec::new().text("x\r").streams();
  let empty = cfbdown::compound(&[
    ("WordDocument", &main),
    ("1Table", &table),
    ("ObjectPool/Loose", b"not in a storage of its own"),
  ])
  .unwrap();
  assert!(Doc::open(&empty).unwrap().embedded().is_empty());
}

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n the picture";

/// A picture as the store keeps it: who it is, a byte, and its bytes.
fn bitmap(instance: u16, kind: u16, bytes: &[u8]) -> Vec<u8> {
  let ids = if instance & 1 == 1 { 32 } else { 16 };
  let mut data = vec![0xAB; ids];
  data.push(0xFF);
  data.extend(bytes);
  drawn(instance << 4, kind, &data)
}

/// The entry of the store that goes before a picture, with the picture after it.
fn entry(picture: &[u8]) -> Vec<u8> {
  let mut data = vec![0u8; 36];
  data.extend(picture);
  drawn(0x0002, 0xF007, &data)
}

/// A picture set in the text, as the `Data` stream keeps it: its length, a header with the
/// size it is to be shown at and how far it is scaled, a shape, and the picture.
fn inline(goal: (i16, i16), scale: (u16, u16), picture: &[u8]) -> Vec<u8> {
  let mut out = vec![0u8; 0x44];
  out[4..6].copy_from_slice(&0x44u16.to_le_bytes());
  out[6..8].copy_from_slice(&0x64i16.to_le_bytes());
  out[0x1C..0x1E].copy_from_slice(&goal.0.to_le_bytes());
  out[0x1E..0x20].copy_from_slice(&goal.1.to_le_bytes());
  out[0x20..0x22].copy_from_slice(&scale.0.to_le_bytes());
  out[0x22..0x24].copy_from_slice(&scale.1.to_le_bytes());
  out.extend(drawn(0x000F, 0xF004, &drawn(0x0002, 0xF00A, &[0; 8])));
  out.extend(entry(picture));
  let length = u32::try_from(out.len()).unwrap();
  out[0..4].copy_from_slice(&length.to_le_bytes());
  out
}

fn with_data(spec: DocSpec, data: &[u8]) -> Doc {
  let (main, table) = spec.streams();
  let file = cfbdown::compound(&[("WordDocument", &main), ("1Table", &table), ("Data", data)]);
  Doc::open(&file.unwrap()).unwrap()
}

#[test]
fn a_picture_set_in_the_text_is_given_as_its_bytes_with_the_size_it_is_shown_at() {
  let first = inline((1440, 720), (500, 1000), &bitmap(0x6E0, 0xF01E, PNG));
  let second = inline(
    (0, 0),
    (1000, 1000),
    &bitmap(0x46B, 0xF01D, b"\xFF\xD8\xFF a photograph"),
  );
  let data = [first.clone(), second].concat();
  let at = u32::try_from(first.len()).unwrap();
  let doc = with_data(
    DocSpec::new()
      .text("a\u{1}b\u{1}c\u{1}\r")
      .run(1, 2, &[long(0x6A03, 0), byte(0x0855, 1)])
      .run(3, 4, &[long(0x6A03, at), byte(0x0855, 1)])
      // The same picture again is the same picture.
      .run(5, 6, &[long(0x6A03, 0), byte(0x0855, 1)]),
    &data,
  );
  assert_eq!(doc.text(), "abc");
  let [png, jpeg] = doc.pictures() else {
    panic!("{} pictures", doc.pictures().len());
  };
  // Shown at half its width: 1,440 twentieths of a point, scaled to 500 in the thousand.
  assert_eq!(
    (png.format, &png.bytes[..], png.goal),
    (PictureFormat::Png, PNG, Some((720, 720)))
  );
  assert_eq!((jpeg.format, jpeg.goal), (PictureFormat::Jpeg, None));
  assert_eq!(jpeg.bytes, b"\xFF\xD8\xFF a photograph");
}

#[test]
fn a_metafile_kept_compressed_is_given_as_it_was() {
  use std::io::Write;
  let drawing = b"\x01\x00\x00\x00 an enhanced metafile".repeat(20);
  let mut packer = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
  packer.write_all(&drawing).unwrap();
  let packed = packer.finish().unwrap();
  // Who it is, its length when whole, its bounds and its size, its length as kept, and how
  // it is kept: nought for compressed.
  let mut data = vec![0xAB; 16];
  data.extend(u32::try_from(drawing.len()).unwrap().to_le_bytes());
  data.extend([0u8; 24]);
  data.extend(u32::try_from(packed.len()).unwrap().to_le_bytes());
  data.extend([0, 0xFE]);
  data.extend(&packed);
  let kept = inline((100, 100), (1000, 1000), &drawn(0x3D4 << 4, 0xF01A, &data));
  let doc = with_data(
    DocSpec::new().text("\u{1}\r").run(0, 1, &[long(0x6A03, 0)]),
    &kept,
  );
  let [picture] = doc.pictures() else {
    panic!("{} pictures", doc.pictures().len());
  };
  assert_eq!(picture.format, PictureFormat::Emf);
  assert_eq!(picture.bytes, drawing);
}

#[test]
fn the_picture_that_stands_for_an_embedded_object_is_not_one_of_the_pictures() {
  let data = inline((100, 100), (1000, 1000), &bitmap(0x6E0, 0xF01E, PNG));
  let doc = with_data(
    DocSpec::new()
      .text("\u{1}\r")
      .run(0, 1, &[long(0x6A03, 0), byte(0x080A, 1)]),
    &data,
  );
  assert!(doc.pictures().is_empty());
}

#[test]
fn the_pictures_of_floating_shapes_are_read_from_the_drawing_s_store() {
  let (mut main, mut table) = DocSpec::new().text("x\r").streams();
  // The store lists each picture and where it is kept in the main stream.
  let kept = u32::try_from(main.len()).unwrap();
  main.extend(bitmap(0x6E0, 0xF01E, PNG));
  let mut listed = vec![0u8; 36];
  listed[28..32].copy_from_slice(&kept.to_le_bytes());
  let store = drawn(0x001F, 0xF001, &drawn(0x0002, 0xF007, &listed));
  let drawings = drawn(0x000F, 0xF000, &store);
  let slot = 0x9A + 50 * 8;
  let at = u32::try_from(table.len()).unwrap();
  main[slot..slot + 4].copy_from_slice(&at.to_le_bytes());
  main[slot + 4..slot + 8].copy_from_slice(&u32::try_from(drawings.len()).unwrap().to_le_bytes());
  table.extend(drawings);
  let file = cfbdown::compound(&[("WordDocument", &main), ("1Table", &table)]).unwrap();
  let doc = Doc::open(&file).unwrap();
  let [picture] = doc.pictures() else {
    panic!("{} pictures", doc.pictures().len());
  };
  assert_eq!(
    (picture.format, &picture.bytes[..], picture.goal),
    (PictureFormat::Png, PNG, None)
  );
}

#[test]
fn a_document_that_states_more_characters_than_any_holds_is_refused_before_it_is_read() {
  let (main, mut table) = DocSpec::new().text("x\r").streams();
  // The table of pieces opens the table stream: a mark, a length, and then where the one
  // piece starts and where it ends. It is made to end forty million characters on.
  table[9..13].copy_from_slice(&40_000_000u32.to_le_bytes());
  let file = cfbdown::compound(&[("WordDocument", &main), ("1Table", &table)]).unwrap();
  assert_eq!(
    refusal(&file),
    "limit exceeded: the document has more than 33554432 characters"
  );
}
