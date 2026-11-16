use roxmltree::Node;

use super::*;

const FIXTURES: [&str; 13] = [
  "Catullus-and-Lucan-on-Pompey-and-Caesar.docx",
  "Harvati-2003.docx",
  "Lucans-Pharsalia-1.129-157.docx",
  "Muscles-of-mastication.docx",
  "Muscles-of-the-Skull.docx",
  "Ongoing-Research-Notes.docx",
  "Poem-29.docx",
  "Ramal-Morphology-Articles-Summary.docx",
  "Summary-of-Mandibular-Ontogenetic-Patterns.docx",
  "andrew-ross-oct-2026.docx",
  "fonts-sample-dos-word.docx",
  "lo-pandoc.docx",
  "pandoc.docx",
];

fn fixture(name: &str) -> Vec<u8> {
  std::fs::read(format!(
    "{}/../docxdown-node/__test__/fixtures/{name}",
    env!("CARGO_MANIFEST_DIR")
  ))
  .unwrap_or_else(|e| panic!("fixture {name}: {e}"))
}

fn open(name: &str) -> Document {
  Document::open(fixture(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// Characters that are not white space.
fn visible(text: &str) -> usize {
  text.chars().filter(|c| !c.is_whitespace()).count()
}

/// What a reader of the document sees in its main part, counted straight from the XML: the
/// visible characters of every `w:t`, less the second copy of a text box and what a pronunciation
/// guide or a moved-away passage holds, plus the characters that are elements of their own.
fn raw_count(bytes: Vec<u8>) -> usize {
  let mut package = ooxmldown::Package::open(bytes).unwrap();
  let main = package.officedocument().unwrap();
  let part = package.part(&main).unwrap();
  let doc = ooxmldown::parse_xml(&part).unwrap();
  let named = |n: &Node<'_, '_>, name: &str| n.is_element() && n.tag_name().name() == name;
  let unseen = |n: &Node<'_, '_>| {
    n.ancestors().any(|a| {
      named(&a, "moveFrom")
        || named(&a, "rt")
        || (named(&a, "Fallback")
          && a
            .parent()
            .is_some_and(|p| p.children().any(|c| named(&c, "Choice"))))
    })
  };
  doc
    .descendants()
    .filter(|n| !unseen(n))
    .map(|n| match xml::wml(n) {
      Some("t") => visible(n.text().unwrap_or("")),
      Some("noBreakHyphen" | "sym") => 1,
      _ => 0,
    })
    .sum()
}

fn blocks_count(blocks: &[Block]) -> usize {
  blocks
    .iter()
    .map(|block| match block {
      Block::Paragraph(paragraph) => visible(&paragraph.text()),
      Block::Table(table) => table
        .rows
        .iter()
        .flat_map(|row| &row.cells)
        .map(|cell| blocks_count(&cell.blocks))
        .sum(),
    })
    .sum()
}

#[test]
fn every_character_a_reader_sees_is_read() {
  for name in FIXTURES {
    let document = open(name);
    assert_eq!(
      blocks_count(&document.blocks),
      raw_count(fixture(name)),
      "{name}"
    );
    assert_eq!(document.warnings, Vec::<String>::new(), "{name}");
  }
}

/// What each fixture's own properties record, and what the file holds when its XML is counted
/// by hand: pages as the application last saved them, tables, hyperlinks and pictures.
struct Known {
  name: &'static str,
  pages_recorded: u32,
  pages_counted: u32,
  tables: u32,
  links: u32,
  pictures: u32,
}

const KNOWN: [Known; 13] = [
  // Word left no marker for one of this document's pages, so the count falls one short.
  known(
    "Catullus-and-Lucan-on-Pompey-and-Caesar.docx",
    6,
    5,
    0,
    0,
    0,
  ),
  known("Harvati-2003.docx", 8, 8, 1, 0, 3),
  known("Lucans-Pharsalia-1.129-157.docx", 1, 1, 0, 0, 0),
  known("Muscles-of-mastication.docx", 5, 5, 2, 2, 7),
  known("Muscles-of-the-Skull.docx", 4, 4, 0, 0, 4),
  known("Ongoing-Research-Notes.docx", 9, 9, 2, 0, 0),
  known("Poem-29.docx", 1, 1, 0, 0, 0),
  known("Ramal-Morphology-Articles-Summary.docx", 3, 2, 0, 0, 0),
  known(
    "Summary-of-Mandibular-Ontogenetic-Patterns.docx",
    3,
    3,
    0,
    0,
    0,
  ),
  known("andrew-ross-oct-2026.docx", 3, 3, 0, 3, 0),
  known("fonts-sample-dos-word.docx", 1, 1, 0, 0, 0),
  known("lo-pandoc.docx", 1, 1, 1, 1, 0),
  known("pandoc.docx", 1, 1, 1, 1, 0),
];

const fn known(
  name: &'static str,
  pages_recorded: u32,
  pages_counted: u32,
  tables: u32,
  links: u32,
  pictures: u32,
) -> Known {
  Known {
    name,
    pages_recorded,
    pages_counted,
    tables,
    links,
    pictures,
  }
}

#[test]
fn the_counts_the_application_recorded_come_through_as_it_wrote_them() {
  // The essay's `docProps/app.xml` says: Pages 6, Words 1887, Characters 9665, Lines 182,
  // Paragraphs 96.
  let metadata = open("Catullus-and-Lucan-on-Pompey-and-Caesar.docx").metadata();
  assert_eq!(
    (
      metadata.pages_recorded,
      metadata.words_recorded,
      metadata.characters_recorded,
      metadata.lines_recorded,
      metadata.paragraphs_recorded
    ),
    (Some(6), Some(1887), Some(9665), Some(182), Some(96))
  );
}

#[test]
fn metadata_counts_what_each_document_holds() {
  for expected in &KNOWN {
    let name = expected.name;
    let document = open(name);
    let metadata = document.metadata();
    assert_eq!(
      metadata.pages_recorded,
      Some(expected.pages_recorded),
      "{name}"
    );
    assert_eq!(metadata.pages_counted, expected.pages_counted, "{name}");
    // The application's own count is the one reported as the page count.
    assert_eq!(metadata.page_count, expected.pages_recorded, "{name}");
    assert_eq!(metadata.table_count, expected.tables, "{name}");
    assert_eq!(metadata.link_count, expected.links, "{name}");
    assert_eq!(metadata.image_count, expected.pictures, "{name}");
    assert_eq!(
      metadata.annotation_count,
      expected.links + metadata.comment_count,
      "{name}"
    );
    assert_eq!(
      document.annotations().len() as u32,
      metadata.annotation_count,
      "{name}"
    );
    assert_eq!(
      document.placements().len() as u32,
      metadata.image_count,
      "{name}"
    );
    assert_eq!(
      document.tables().len() as u32,
      metadata.table_count,
      "{name}"
    );
    assert_eq!(document.language.as_deref(), Some("en-US"), "{name}");
    assert!(!document.has_macros && !document.is_template, "{name}");
    assert!(!document.has_tracked_changes, "{name}");
  }
}

#[test]
fn words_counted_in_plain_prose_are_within_two_percent_of_the_count_word_recorded() {
  // Documents of running text, with no text boxes and no header: where the application's own
  // rules for what a word is have the least room to differ.
  for (name, recorded) in [
    ("Harvati-2003.docx", 1920),
    ("Muscles-of-mastication.docx", 1712),
    ("Ongoing-Research-Notes.docx", 2198),
    ("Ramal-Morphology-Articles-Summary.docx", 654),
    ("Summary-of-Mandibular-Ontogenetic-Patterns.docx", 1066),
    ("andrew-ross-oct-2026.docx", 1520),
  ] {
    let metadata = open(name).metadata();
    assert_eq!(metadata.words_recorded, Some(recorded), "{name}");
    let off = f64::from(metadata.word_count.abs_diff(recorded)) / f64::from(recorded);
    assert!(
      off < 0.02,
      "{name}: {} counted, {recorded} recorded",
      metadata.word_count
    );
  }
}

#[test]
fn every_section_is_letter_paper() {
  for name in FIXTURES {
    let metadata = open(name).metadata();
    assert!(metadata.page_size_uniform, "{name}");
    for section in &metadata.sections {
      assert_eq!(
        (section.width, section.height),
        (Some(612.0), Some(792.0)),
        "{name}"
      );
      assert_eq!(section.paper_size, contentdown::PaperSize::Letter, "{name}");
      assert!(!section.landscape, "{name}");
    }
  }
  // One document has two sections; the second starts where its first block is.
  let sections = open("Harvati-2003.docx").metadata().sections;
  assert_eq!(
    sections.iter().map(|s| s.first_page).collect::<Vec<_>>(),
    [1, 6]
  );
}

#[test]
fn the_pages_of_a_document_join_back_into_its_text() {
  for name in FIXTURES {
    let document = open(name);
    let pages = document.text_per_page();
    assert_eq!(pages.len() as u32, document.page_count, "{name}");
    assert_eq!(pages.join("\n"), document.text(), "{name}");
    // Pages only go forward.
    let mut last = 1;
    for block in &document.blocks {
      assert!(block.page() >= last, "{name}");
      last = block.page();
    }
    assert!(last <= document.page_count, "{name}");
  }
  // A document Word broke into pages has text on each of them.
  let notes = open("Ongoing-Research-Notes.docx").text_per_page();
  assert!(notes.iter().all(|page| visible(page) > 0));
  // Unless a page holds nothing but a picture.
  let figures = open("Harvati-2003.docx");
  let lengths: Vec<usize> = figures.text_per_page().iter().map(|p| visible(p)).collect();
  assert_eq!(lengths[3], 0);
  assert!(lengths.iter().enumerate().all(|(at, n)| at == 3 || *n > 0));
  assert!(figures.placements().iter().any(|p| p.page == Some(4)));
}

#[test]
fn pictures_are_listed_where_they_are_placed_with_the_bytes_as_stored() {
  let document = open("Muscles-of-mastication.docx");
  let mimes: Vec<&str> = document.images.iter().filter_map(|i| i.mime).collect();
  assert_eq!(mimes.iter().filter(|m| **m == "image/jpeg").count(), 6);
  assert_eq!(mimes.iter().filter(|m| **m == "image/gif").count(), 1);
  for image in &document.images {
    assert!(image.name.starts_with("word/media/"), "{}", image.name);
    assert!(!image.linked);
    let magic: &[u8] = match image.mime {
      Some("image/jpeg") => &[0xFF, 0xD8, 0xFF],
      Some("image/gif") => b"GIF8",
      other => panic!("{other:?}"),
    };
    assert!(image.data.starts_with(magic), "{}", image.name);
    assert!(image.width.is_some_and(|w| w > 0) && image.height.is_some_and(|h| h > 0));
  }
  let placements = document.placements();
  assert_eq!(placements.len(), 7);
  let mut on_page = std::collections::HashMap::new();
  let mut last = 1;
  for placement in &placements {
    assert_eq!(placement.part, PartKind::Body);
    let page = placement.page.unwrap();
    assert!((last..=document.page_count).contains(&page));
    last = page;
    // The index counts the pictures of one page.
    let next = on_page.entry(page).or_insert(0);
    assert_eq!(placement.image_index, *next);
    *next += 1;
    // Every picture here is shown at a size, in points.
    assert!(placement.display_width.is_some_and(|w| w > 0.0));
    assert!(placement.display_height.is_some_and(|h| h > 0.0));
    assert!(document.images.get(placement.image as usize).is_some());
  }

  // A vector picture has no pixel size. The first is in a table cell on the first page; each
  // of the other two is alone in a paragraph that Word marked as starting a page.
  let vector = open("Harvati-2003.docx");
  assert!(
    vector
      .images
      .iter()
      .all(|i| i.mime == Some("image/emf") && i.width.is_none())
  );
  let pages: Vec<u32> = vector.placements().iter().filter_map(|p| p.page).collect();
  assert_eq!(pages, [1, 4, 5]);
}

#[test]
fn links_are_annotations_with_the_text_they_cover() {
  for name in ["Muscles-of-mastication.docx", "andrew-ross-oct-2026.docx"] {
    let document = open(name);
    let annotations = document.annotations();
    assert!(!annotations.is_empty(), "{name}");
    for annotation in &annotations {
      assert_eq!(annotation.subtype, AnnotationKind::Link, "{name}");
      assert_eq!(annotation.part, PartKind::Body, "{name}");
      assert!(
        annotation
          .uri
          .as_deref()
          .is_some_and(|uri| uri.contains(':')),
        "{name}"
      );
      assert!(!annotation.text.trim().is_empty(), "{name}");
      assert!(
        annotation
          .page
          .is_some_and(|page| (1..=document.page_count).contains(&page)),
        "{name}"
      );
      assert_eq!(
        (&annotation.content, &annotation.author),
        (&None, &None),
        "{name}"
      );
    }
  }
}

/// The document `pandoc` wrote from a page of Markdown and LibreOffice saved again.
const SAMPLE: &str = "lo-pandoc.docx";

#[test]
fn the_sample_comes_back_as_the_markdown_it_was_written_from() {
  let document = open(SAMPLE);
  assert_eq!(
    document.markdown(),
    r"**Invoice 4711**

Trial

# Invoice 4711

Bill to **Acme Corp**, see the [terms](<https://example.com/terms>).[^1]

| Item | Qty | Unit price | Total |
| --- | --- | --- | --- |
| Widget | 2 | 10.00 | 20.00 |
| Gadget | 1 | 5.50 | 5.50 |

1. First step
2. Second step
    - nested bullet

Total due: 25.50 USD

[^1]: Net thirty days."
  );
  assert_eq!(document.props.title.as_deref(), Some("Invoice 4711"));
  assert_eq!(document.props.creator.as_deref(), Some("Trial"));
}

#[test]
fn the_sample_has_its_heading_its_list_its_table_and_its_note() {
  let document = open(SAMPLE);
  let mut headings = Vec::new();
  let mut markers = Vec::new();
  walk::each_paragraph(&document.blocks, &mut |paragraph| {
    if let Some(level) = paragraph.heading {
      headings.push((level, paragraph.text()));
    }
    if let Some(item) = &paragraph.list {
      markers.push((item.marker.clone(), item.level, item.ordered));
    }
  });
  assert_eq!(headings, [(1, "Invoice 4711".to_owned())]);
  assert_eq!(
    markers,
    [
      ("1.".to_owned(), 0, true),
      ("2.".to_owned(), 0, true),
      ("•".to_owned(), 1, false)
    ]
  );

  let tables = document.tables();
  let table = &tables[0];
  assert_eq!((table.row_count, table.column_count), (3, 4));
  assert_eq!((table.index, table.parent, table.page), (0, None, 1));
  assert!(table.merges.is_empty());
  let cell = |row: u32, column: u32| {
    table
      .cells
      .iter()
      .find(|cell| (cell.row, cell.column) == (row, column))
      .map(|cell| cell.text.as_str())
  };
  assert_eq!(cell(0, 2), Some("Unit price"));
  assert_eq!(cell(1, 3), Some("20.00"));
  assert_eq!(cell(2, 0), Some("Gadget"));
  assert_eq!(cell(3, 0), None);
  assert_eq!(cell_ref(1, 3), "D2");

  assert_eq!(document.notes.len(), 1);
  let note = &document.notes[0];
  assert_eq!((note.kind, note.page), (NoteKind::Footnote, 1));
  assert_eq!(text::blocks_text(&note.blocks).trim(), "Net thirty days.");

  assert_eq!(
    document.annotations(),
    [Annotation {
      page: Some(1),
      part: PartKind::Body,
      subtype: AnnotationKind::Link,
      uri: Some("https://example.com/terms".to_owned()),
      dest: None,
      content: None,
      text: "terms".to_owned(),
      author: None,
      date: None,
    }]
  );
}

#[test]
fn the_sample_is_laid_out_with_a_row_on_a_line_and_read_as_an_invoice() {
  let document = open(SAMPLE);
  let pages = document.layout();
  assert_eq!(pages.len(), 1);
  let page = &pages[0];
  assert_eq!((page.page, page.width, page.height), (1, 612.0, 792.0));
  let lines: Vec<&str> = page.lines.iter().map(|line| line.text.as_str()).collect();
  assert_eq!(
    lines,
    [
      "Invoice 4711",
      "Trial",
      "Invoice 4711",
      "Bill to Acme Corp, see the terms.",
      "Item Qty Unit price Total",
      "Widget 2 10.00 20.00",
      "Gadget 1 5.50 5.50",
      "1. First step",
      "2. Second step",
      "• nested bullet",
      "Total due: 25.50 USD",
    ]
  );
  // The columns line up from row to row: the first on its left edge, and the numbers, which
  // the table sets flush right, on their right edges.
  let edges = |line: usize| -> (f64, Vec<f64>) {
    let line = &page.lines[line];
    let words = &page.words[line.first_word as usize..][..line.word_count as usize];
    (words[0].x0, words[1..].iter().map(|word| word.x1).collect())
  };
  assert_eq!(edges(5), edges(6));
  assert_eq!(edges(5).1.len(), 3);
  let bold: Vec<&str> = page
    .words
    .iter()
    .filter(|word| word.bold == Some(true))
    .map(|word| word.text.as_str())
    .collect();
  assert!(bold.ends_with(&["Acme", "Corp,"]), "{bold:?}");

  let invoice = document.invoice(None);
  let fields = &invoice.fields;
  assert_eq!(
    fields.invoice_id.as_ref().map(|f| f.value.as_str()),
    Some("4711")
  );
  assert_eq!(fields.amount_due.as_ref().map(|f| f.value), Some(25.5));
}

#[test]
fn a_layout_has_a_page_for_each_one_counted_and_every_word_on_it() {
  for name in FIXTURES {
    let document = open(name);
    let pages = document.layout();
    assert_eq!(pages.len() as u32, document.page_count, "{name}");
    let mut set = 0;
    for (at, page) in pages.iter().enumerate() {
      assert_eq!(page.page as usize, at + 1, "{name}");
      assert_eq!(page.width, 612.0, "{name}");
      assert!(page.height >= 792.0, "{name}");
      for word in &page.words {
        assert!(
          0.0 <= word.x0 && word.x0 < word.x1 && word.x1 <= page.width,
          "{name}: {word:?}"
        );
        assert!(
          0.0 <= word.y0 && word.y0 < word.y1 && word.y1 <= page.height,
          "{name}: {word:?}"
        );
        set += visible(&word.text);
      }
    }
    // Every character of the body is set; a header or a footer adds its own on each page.
    let body = visible(&document.text());
    if document.headers.is_empty() && document.footers.is_empty() {
      assert_eq!(set, body, "{name}");
    } else {
      assert!(set >= body, "{name}");
    }
  }
}

#[test]
fn a_header_is_set_at_the_top_of_each_page_of_its_section() {
  let document = open("Catullus-and-Lucan-on-Pompey-and-Caesar.docx");
  let header = text::blocks_text(&document.headers[0].blocks);
  assert!(visible(&header) > 0);
  assert_eq!(document.sections[0].headers.default, Some(0));
  let first = header.split_whitespace().next().unwrap();
  for page in document.layout() {
    // Its first word is the first word of the page.
    assert_eq!(page.words[0].text, first);
    assert!(page.words[0].y1 > page.height - 72.0);
  }
}

#[test]
fn what_is_not_a_word_document_is_refused_by_name() {
  let refusal = |bytes: Vec<u8>| Document::open(bytes).unwrap_err().to_string();
  let mut legacy = OLE_MAGIC.to_vec();
  legacy.extend_from_slice(&[0; 64]);
  assert_eq!(
    refusal(legacy),
    "unsupported format: OLE compound file: a legacy .doc, or a document protected with a password"
  );
  let workbook = std::fs::read(format!(
    "{}/../xlsxdown-node/__test__/fixtures/minimal.xlsx",
    env!("CARGO_MANIFEST_DIR")
  ))
  .unwrap();
  assert_eq!(refusal(workbook), "unsupported format: workbook");
  assert!(refusal(b"not a zip at all".to_vec()).starts_with("malformed document: "));
  assert!(refusal(Vec::new()).starts_with("malformed document: "));
}

#[test]
fn the_original_view_of_a_document_with_no_changes_is_the_same_document() {
  let last = open(SAMPLE);
  let original = Document::open_with(
    fixture(SAMPLE),
    Options {
      view: View::Original,
    },
  )
  .unwrap();
  assert_eq!(last, original);
}
