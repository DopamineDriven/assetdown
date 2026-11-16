use super::*;
use crate::lists::{AnnotationKind, PartKind};

/// The decks in the binding's public fixtures folder, both written by PowerPoint. Decks kept
/// out of the repository are read by a spec of the binding that is kept out of it too.
const FIXTURES: [&str; 2] = [
  "Activity-7.pptx",
  "HMEM-Horizon-Mediated-Episodic-Memory.pptx",
];

fn fixture(name: &str) -> Vec<u8> {
  std::fs::read(format!(
    "{}/../pptxdown-node/__test__/fixtures/{name}",
    env!("CARGO_MANIFEST_DIR")
  ))
  .unwrap_or_else(|e| panic!("fixture {name}: {e}"))
}

fn open(name: &str) -> Presentation {
  Presentation::open(fixture(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// Characters that are not white space.
fn visible(text: &str) -> usize {
  text.chars().filter(|c| !c.is_whitespace()).count()
}

/// What each slide shows, counted straight from the XML: the visible characters of every `a:t`
/// in the slide's part, less the second copy of a shape kept for older applications.
fn raw_counts(bytes: Vec<u8>) -> Vec<usize> {
  let mut package = ooxmldown::Package::open(bytes).unwrap();
  let main = package.officedocument().unwrap();
  let rels = relationships_of(&mut package, &main).unwrap();
  let part = package.part(&main).unwrap();
  let doc = parse_xml(&part).unwrap();
  let named = |n: &Node<'_, '_>, name: &str| n.is_element() && n.tag_name().name() == name;
  let slides: Vec<String> = doc
    .descendants()
    .filter(|n| named(n, "sldId") && n.parent().is_some_and(|p| named(&p, "sldIdLst")))
    .filter_map(|n| Some(rels.get(rel_attr(n, "id")?)?.target.clone()))
    .collect();
  slides
    .iter()
    .map(|slide| {
      let part = package.part(slide).unwrap();
      let doc = parse_xml(&part).unwrap();
      let second_copy = |n: &Node<'_, '_>| {
        n.ancestors().any(|a| {
          named(&a, "Fallback")
            && a
              .parent()
              .is_some_and(|p| p.children().any(|c| named(&c, "Choice")))
        })
      };
      doc
        .descendants()
        .filter(|n| named(n, "t") && !second_copy(n))
        .map(|n| visible(n.text().unwrap_or("")))
        .sum()
    })
    .collect()
}

/// The visible characters a shape holds, whatever it is.
fn shape_count(shape: &Shape) -> usize {
  let paragraphs =
    |paragraphs: &[Paragraph]| -> usize { paragraphs.iter().map(|p| visible(&p.text())).sum() };
  match shape {
    Shape::Text(shape) => paragraphs(&shape.paragraphs),
    Shape::Table(table) => table
      .rows
      .iter()
      .flat_map(|row| &row.cells)
      .map(|cell| paragraphs(&cell.paragraphs))
      .sum(),
    Shape::Group(group) => group.shapes.iter().map(shape_count).sum(),
    Shape::Diagram(diagram) => diagram.shapes.iter().map(shape_count).sum(),
    Shape::Picture(_) | Shape::Chart(_) | Shape::Media(_) => 0,
  }
}

#[test]
fn every_character_on_a_slide_is_read() {
  for name in FIXTURES {
    let deck = open(name);
    let read: Vec<usize> = deck
      .slides
      .iter()
      .map(|slide| slide.shapes.iter().map(shape_count).sum())
      .collect();
    assert_eq!(read, raw_counts(fixture(name)), "{name}");
    assert_eq!(deck.warnings, Vec::<String>::new(), "{name}");
  }
}

use crate::testing::{Deck, para, placeholder, shape, text_box, xfrm};

/// What each fixture holds when its XML is counted by hand: its slides, the size of a slide in
/// points, and the pictures, tables and links on its slides.
struct Known {
  name: &'static str,
  slides: u32,
  size: (f64, f64),
  pictures: u32,
  tables: u32,
  links: u32,
}

const fn known(
  name: &'static str,
  slides: u32,
  size: (f64, f64),
  pictures: u32,
  tables: u32,
  links: u32,
) -> Known {
  Known {
    name,
    slides,
    size,
    pictures,
    tables,
    links,
  }
}

const KNOWN: [Known; 2] = [
  known("Activity-7.pptx", 4, (720.0, 540.0), 9, 0, 0),
  known(
    "HMEM-Horizon-Mediated-Episodic-Memory.pptx",
    13,
    (960.0, 540.0),
    5,
    0,
    0,
  ),
];

#[test]
fn metadata_counts_what_each_deck_holds() {
  for Known {
    name,
    slides,
    size,
    pictures,
    tables,
    links,
  } in KNOWN
  {
    let deck = open(name);
    let metadata = deck.metadata();
    assert_eq!(metadata.slide_count, slides, "{name}");
    assert_eq!(metadata.slides_recorded, Some(slides), "{name}");
    assert_eq!((metadata.width, metadata.height), size, "{name}");
    assert_eq!(metadata.image_count, pictures, "{name}");
    assert_eq!(metadata.table_count, tables, "{name}");
    assert_eq!(metadata.link_count, links, "{name}");
    assert_eq!(
      (
        metadata.hidden_slide_count,
        metadata.comment_count,
        metadata.media_count
      ),
      (0, 0, 0),
      "{name}"
    );
    assert_eq!(deck.placements().len() as u32, pictures, "{name}");
    assert_eq!(deck.tables().len() as u32, tables, "{name}");
    assert_eq!(deck.annotations().len() as u32, links, "{name}");
    // Slides are numbered from one in the order of the deck.
    let numbers: Vec<u32> = deck.slides.iter().map(|slide| slide.number).collect();
    assert_eq!(numbers, (1..=slides).collect::<Vec<_>>(), "{name}");
    assert!(
      !deck.has_macros && !deck.is_template && !deck.is_show,
      "{name}"
    );
  }
}

#[test]
fn a_slide_sized_for_a_screen_is_no_paper_size() {
  let wide = open("HMEM-Horizon-Mediated-Episodic-Memory.pptx");
  assert_eq!(wide.metadata().aspect_ratio, "16:9");
  assert_eq!(wide.metadata().paper_size, contentdown::PaperSize::Unknown);
  // A size the file gives no word for.
  assert_eq!(wide.size_kind, None);
  let narrow = open("Activity-7.pptx");
  assert_eq!(narrow.metadata().aspect_ratio, "4:3");
  assert_eq!(narrow.size_kind.as_deref(), Some("screen4x3"));
  // The other usual size for a 16:9 screen: ten inches across.
  let small = Deck::new()
    .size(r#"<p:sldSz cx="9144000" cy="5143500" type="screen16x9"/>"#)
    .slide("")
    .open();
  assert_eq!((small.width, small.height), (720.0, 405.0));
  assert_eq!(small.metadata().aspect_ratio, "16:9");
  assert_eq!(small.size_kind.as_deref(), Some("screen16x9"));
  // A deck sized for paper is that paper.
  let letter = Deck::new()
    .size(r#"<p:sldSz cx="10058400" cy="7772400" type="letter"/>"#)
    .slide("")
    .open();
  assert_eq!((letter.width, letter.height), (792.0, 612.0));
  assert_eq!(letter.metadata().paper_size, contentdown::PaperSize::Letter);
  // And one that states no size is ten inches by seven and a half.
  let bare = Deck::new().size("").slide("").open();
  assert_eq!(
    (bare.width, bare.height, bare.size_kind),
    (720.0, 540.0, None)
  );
}

#[test]
fn every_picture_of_the_fixtures_is_found_with_its_bytes_and_its_box() {
  for Known { name, .. } in KNOWN {
    let deck = open(name);
    for image in &deck.images {
      assert!(
        image.name.starts_with("ppt/media/"),
        "{name}: {}",
        image.name
      );
      assert!(
        !image.linked && !image.data.is_empty(),
        "{name}: {}",
        image.name
      );
      assert!(image.mime.is_some(), "{name}: {}", image.name);
    }
    let mut on_slide = std::collections::HashMap::new();
    for placement in deck.placements() {
      assert!(placement.frame.is_some(), "{name}");
      assert!(
        deck.images.get(placement.image as usize).is_some(),
        "{name}"
      );
      let next = on_slide.entry(placement.page).or_insert(0);
      assert_eq!(placement.image_index, *next, "{name}");
      *next += 1;
    }
  }
  let formats = |name: &str| {
    let mut found: Vec<&str> = open(name).images.iter().filter_map(|i| i.mime).collect();
    found.sort_unstable();
    found.dedup();
    found.join(" ")
  };
  assert_eq!(formats("Activity-7.pptx"), "image/jpeg image/tiff");
  assert_eq!(
    formats("HMEM-Horizon-Mediated-Episodic-Memory.pptx"),
    "image/png"
  );
}

#[test]
fn the_title_of_a_slide_is_its_title_placeholder_and_its_layout_is_named() {
  let deck = open("HMEM-Horizon-Mediated-Episodic-Memory.pptx");
  let titles: Vec<Option<&str>> = deck
    .slides
    .iter()
    .map(|slide| slide.title.as_deref())
    .collect();
  assert_eq!(
    titles,
    [
      Some("Horizon-Mediated Episodic Memory"),
      Some("The cost of long conversations"),
      // The third slide has no title placeholder: its large figure is a text box.
      None,
      Some("Three horizons"),
      Some("One write path, three read paths"),
      Some("The write path"),
      Some("The watermark"),
      Some("The summarizer roster"),
      Some("No fold editor"),
      Some("Substitution assembly"),
      Some("Deliberate retrieval"),
      Some("Bounded and self-healing"),
      Some("The case for HMEM"),
    ]
  );
  assert_eq!(deck.metadata().layouts, ["Title and body"]);
  assert_eq!(deck.language.as_deref(), Some("en-US"));
  for slide in &deck.slides {
    assert_eq!(slide.layout.as_deref(), Some("Title and body"));
    // The title is read first wherever it sits, and every shape has a box.
    if let Some(title) = &slide.title {
      assert_eq!(&slide.shapes[0].text(), title, "slide {}", slide.number);
    }
    for shape in &slide.shapes {
      assert!(shape.info().frame.is_some(), "slide {}", slide.number);
    }
  }

  // An older deck: three slides whose heading is typed into a text box, and one with a title.
  let older = open("Activity-7.pptx");
  let found: Vec<(Option<&str>, Option<&str>)> = older
    .slides
    .iter()
    .map(|slide| (slide.title.as_deref(), slide.layout.as_deref()))
    .collect();
  assert_eq!(
    found,
    [
      (None, Some("Title Slide")),
      (None, Some("Title and Content")),
      (None, Some("Title and Content")),
      (Some("Point Group Table"), Some("Title and Content")),
    ]
  );
  assert_eq!(
    older.metadata().layouts,
    ["Title Slide", "Title and Content"]
  );
}

#[test]
fn a_list_goes_on_from_where_it_says_and_an_empty_paragraph_takes_no_number() {
  // The second slide types its first step as text ("1.  Using the scissors…") and numbers the
  // rest: five numbered paragraphs, two of them empty, which say they start at 2, 2, 3, 3
  // and 3. Read as PowerPoint shows them, the steps run 1 to 4.
  let deck = open("Activity-7.pptx");
  let Shape::Text(steps) = &deck.slides[1].shapes[0] else {
    panic!("a text shape was expected first");
  };
  let numbered: Vec<(&str, bool)> = steps
    .paragraphs
    .iter()
    .filter(|paragraph| paragraph.ordered)
    .map(|paragraph| {
      (
        paragraph.bullet.as_deref().unwrap_or(""),
        paragraph.text().starts_with("Using the chart"),
      )
    })
    .collect();
  assert_eq!(numbered, [("2.", false), ("3.", false), ("4.", true)]);
  assert_eq!(steps.paragraphs.len(), 9);
  assert!(
    steps.paragraphs[2]
      .text()
      .trim_start()
      .starts_with("1.  Using")
  );
}

#[test]
fn slides_are_in_the_order_the_deck_lists_and_a_hidden_one_keeps_its_number() {
  let deck = || {
    Deck::new()
      .slide(&text_box(0.0, 0.0, 9.0, 9.0, &para("one")))
      .slide_with(r#"show="0""#, "", &text_box(0.0, 0.0, 9.0, 9.0, &para("two")))
      .slide(&text_box(0.0, 0.0, 9.0, 9.0, &para("three")))
      .extensions(
        r#"<p:extLst><p:ext uri="{521415D9}"><p14:sectionLst><p14:section name="Opening" id="{A}"><p14:sldIdLst><p14:sldId id="256"/></p14:sldIdLst></p14:section><p14:section name="Detail" id="{B}"><p14:sldIdLst><p14:sldId id="257"/><p14:sldId id="258"/></p14:sldIdLst></p14:section></p14:sectionLst></p:ext></p:extLst>"#,
      )
  };
  let all = deck().open();
  let read: Vec<(u32, bool, Option<&str>, String)> = all
    .slides
    .iter()
    .map(|slide| {
      (
        slide.number,
        slide.hidden,
        slide.section.as_deref(),
        slide.text(),
      )
    })
    .collect();
  assert_eq!(
    read,
    [
      (1, false, Some("Opening"), "one".to_owned()),
      (2, true, Some("Detail"), "two".to_owned()),
      (3, false, Some("Detail"), "three".to_owned()),
    ]
  );
  assert_eq!(all.slides[1].name, "ppt/slides/slide2.xml");
  let metadata = all.metadata();
  assert_eq!((metadata.slide_count, metadata.hidden_slide_count), (3, 1));
  assert_eq!(metadata.sections, ["Opening", "Detail"]);
  assert_eq!(all.text(), "one\n\ntwo\n\nthree");

  // Left out, the slide after it is still slide three, and the deck still has three slides.
  let shown = deck()
    .options(Options {
      hidden_slides: false,
    })
    .open();
  let numbers: Vec<u32> = shown.slides.iter().map(|slide| slide.number).collect();
  assert_eq!(numbers, [1, 3]);
  assert_eq!(shown.slide_count, 3);
  assert_eq!(
    shown.text_per_page(),
    [(1, "one".to_owned()), (3, "three".to_owned())]
  );
}

#[test]
fn notes_are_the_notes_and_not_the_rest_of_the_notes_page() {
  let deck = Deck::new()
    .slide(&text_box(0.0, 0.0, 9.0, 9.0, &para("slide")))
    .notes(
      &[
        para("Mention the two accounts."),
        "<a:p/>".to_owned(),
        para("Pause."),
      ]
      .concat(),
    )
    .slide(&text_box(0.0, 0.0, 9.0, 9.0, &para("no notes")))
    .slide(&text_box(0.0, 0.0, 9.0, 9.0, &para("empty notes")))
    .notes("<a:p/>")
    .open();
  assert_eq!(
    deck.slides[0].notes_text(),
    "Mention the two accounts.\nPause."
  );
  assert_eq!(
    deck.notes(),
    [(1, "Mention the two accounts.\nPause.".to_owned())]
  );
  // The notes are not on the slide, so they are not in its text.
  assert_eq!(deck.slides[0].text(), "slide");
  assert_eq!(deck.metadata().notes_count, 1);
  // PowerPoint keeps a notes page for every slide; one that holds only the slide number has
  // no notes.
  // One deck has such a page for each of its thirteen slides, the other for its third.
  for (name, pages) in [
    ("HMEM-Horizon-Mediated-Episodic-Memory.pptx", 13),
    ("Activity-7.pptx", 1),
  ] {
    let deck = open(name);
    assert_eq!(deck.props.notes, Some(pages), "{name}");
    assert_eq!(deck.metadata().notes_count, 0, "{name}");
    assert!(deck.notes().is_empty(), "{name}");
  }
}

#[test]
fn comments_of_both_forms_are_read_with_who_wrote_them() {
  let deck = Deck::new()
    .slide(&text_box(0.0, 0.0, 9.0, 9.0, &para("slide")))
    .rel("old", "/comments", "ppt/comments/comment1.xml", false)
    .rel("new", "/comments", "ppt/comments/modernComment_1.xml", false)
    .xml(
      "ppt/comments/comment1.xml",
      "cmLst",
      r#"<p:cm authorId="0" dt="2026-11-15T09:30:00.000" idx="1"><p:pos x="10" y="10"/><p:text>Is this agreed?</p:text></p:cm>"#,
    )
    .xml(
      "ppt/comments/modernComment_1.xml",
      "cmLst",
      r#"<p188:cm id="{1}" authorId="{B}" created="2026-11-16T10:00:00.000"><p188:replyLst><p188:reply id="{2}" authorId="{A}" created="2026-11-16T11:00:00.000"><p188:txBody><a:bodyPr/><a:p><a:r><a:t>Yes.</a:t></a:r></a:p></p188:txBody></p188:reply></p188:replyLst><p188:txBody><a:bodyPr/><a:p><a:r><a:t>Still open?</a:t></a:r></a:p></p188:txBody></p188:cm>"#,
    )
    .link("ppt/presentation.xml", "ca", "/commentAuthors", "ppt/commentAuthors.xml", false)
    .link("ppt/presentation.xml", "au", "/authors", "ppt/authors.xml", false)
    .xml(
      "ppt/commentAuthors.xml",
      "cmAuthorLst",
      r#"<p:cmAuthor id="0" name="Ana" initials="A" lastIdx="1" clrIdx="0"/>"#,
    )
    .xml(
      "ppt/authors.xml",
      "authorLst",
      r#"<p188:author id="{A}" name="Ana" initials="A" userId="u" providerId="p"/><p188:author id="{B}" name="Ben" initials="B" userId="v" providerId="p"/>"#,
    )
    .open();
  let comments: Vec<(Option<&str>, Option<&str>, &str)> = deck.slides[0]
    .comments
    .iter()
    .map(|c| (c.author.as_deref(), c.date.as_deref(), c.text.as_str()))
    .collect();
  assert_eq!(
    comments,
    [
      (
        Some("Ana"),
        Some("2026-11-15T09:30:00.000"),
        "Is this agreed?"
      ),
      (Some("Ben"), Some("2026-11-16T10:00:00.000"), "Still open?"),
      // A reply follows the comment it answers.
      (Some("Ana"), Some("2026-11-16T11:00:00.000"), "Yes."),
    ]
  );
  assert_eq!(deck.slides[0].comments[0].initials.as_deref(), Some("A"));
  let annotations = deck.annotations();
  assert_eq!(annotations.len(), 3);
  assert!(annotations.iter().all(|a| {
    a.subtype == AnnotationKind::Comment && a.page == 1 && a.rect.is_none() && a.text.is_empty()
  }));
  assert_eq!(annotations[1].content.as_deref(), Some("Still open?"));
  assert_eq!(deck.metadata().comment_count, 3);
}

#[test]
fn what_repeats_on_every_slide_is_set_apart_from_the_body() {
  let furniture = [
    shape(
      r#"<p:ph type="dt" idx="10"/>"#,
      &xfrm(20.0, 10.0, 200.0, 20.0),
      &para("15 November"),
    ),
    shape(
      r#"<p:ph type="ftr" idx="11"/>"#,
      &xfrm(300.0, 500.0, 360.0, 30.0),
      &para("Acme"),
    ),
    shape(
      r#"<p:ph type="sldNum" idx="12"/>"#,
      &xfrm(800.0, 500.0, 100.0, 30.0),
      r#"<a:p><a:fld id="{1}" type="slidenum"><a:t>1</a:t></a:fld></a:p>"#,
    ),
  ]
  .concat();
  let slide = |title: &str| {
    [
      placeholder(r#"type="title""#, &para(title)),
      text_box(50.0, 150.0, 400.0, 100.0, &para("body text")),
      furniture.clone(),
    ]
    .concat()
  };
  let deck = Deck::new()
    .slide(&slide("First"))
    .slide(&slide("Second"))
    .open();
  assert_eq!(
    deck.structured_text()[1],
    StructuredText {
      page: 2,
      // The date sits in the top half of the slide; the footer and the number in the bottom.
      header: "15 November".to_owned(),
      body: "Second\nbody text".to_owned(),
      footer: "Acme\n1".to_owned(),
    }
  );
  // The text of a slide is its body.
  assert_eq!(deck.slides[0].text(), "First\nbody text");
  assert_eq!(deck.text(), "First\nbody text\n\nSecond\nbody text");

  // A deck of one slide has nothing that repeats: all of it is body.
  let single = Deck::new().slide(&slide("Only")).open();
  let [only] = single.structured_text().try_into().unwrap();
  assert_eq!((only.header.as_str(), only.footer.as_str()), ("", ""));
  assert_eq!(only.body, "Only\n15 November\nbody text\nAcme\n1");
}

/// Where a link leads (an address, or a place in the deck), the text it covers, and how far
/// down the slide its box starts.
type LinkSeen<'a> = (Option<&'a str>, Option<&'a str>, &'a str, Option<f64>);

#[test]
fn links_are_annotations_with_the_box_they_are_in_and_the_text_they_cover() {
  let linked = r#"<a:p><a:r><a:t>See </a:t></a:r><a:r><a:rPr b="1"><a:hlinkClick r:id="site"/></a:rPr><a:t>the </a:t></a:r><a:r><a:rPr><a:hlinkClick r:id="site"/></a:rPr><a:t>terms</a:t></a:r><a:r><a:t> and </a:t></a:r><a:r><a:rPr><a:hlinkClick r:id="next"/></a:rPr><a:t>slide two</a:t></a:r></a:p>"#;
  let button = r#"<p:sp><p:nvSpPr><p:cNvPr id="3" name="Button"><a:hlinkClick r:id="" action="ppaction://hlinkshowjump?jump=endshow"/></p:cNvPr><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="0" y="5080000"/><a:ext cx="1270000" cy="635000"/></a:xfrm></p:spPr><p:txBody><a:p><a:r><a:t>End</a:t></a:r></a:p></p:txBody></p:sp>"#;
  let deck = Deck::new()
    .slide(
      &[
        text_box(50.0, 60.0, 400.0, 100.0, linked),
        button.to_owned(),
      ]
      .concat(),
    )
    .rel("site", "/hyperlink", "https://example.com/terms", true)
    .rel("next", "/slide", "ppt/slides/slide2.xml", false)
    .slide("")
    .open();
  let annotations = deck.annotations();
  let found: Vec<LinkSeen<'_>> = annotations
    .iter()
    .map(|a| {
      (
        a.uri.as_deref(),
        a.dest.as_deref(),
        a.text.as_str(),
        a.rect.map(|r| r.y),
      )
    })
    .collect();
  // Two runs to one target are one link, though one of them is bold.
  assert_eq!(
    found,
    [
      (
        Some("https://example.com/terms"),
        None,
        "the terms",
        Some(60.0)
      ),
      (None, Some("2"), "slide two", Some(60.0)),
      (None, Some("endshow"), "End", Some(400.0)),
    ]
  );
  let metadata = deck.metadata();
  assert_eq!((metadata.link_count, metadata.annotation_count), (3, 3));
  assert_eq!(deck.annotations()[0].part, PartKind::Slide);
}

#[test]
fn what_is_not_a_presentation_is_refused_by_name() {
  let refusal = |bytes: Vec<u8>| Presentation::open(bytes).unwrap_err().to_string();
  let mut legacy = OLE_MAGIC.to_vec();
  legacy.extend_from_slice(&[0; 64]);
  assert_eq!(
    refusal(legacy),
    "unsupported format: OLE compound file: a legacy .ppt, or a presentation protected with a password"
  );
  let sibling =
    |path: &str| std::fs::read(format!("{}/../{path}", env!("CARGO_MANIFEST_DIR"))).unwrap();
  assert_eq!(
    refusal(sibling("xlsxdown-node/__test__/fixtures/minimal.xlsx")),
    "unsupported format: workbook"
  );
  assert_eq!(
    refusal(sibling("docxdown-node/__test__/fixtures/lo-pandoc.docx")),
    "unsupported format: Word document package"
  );
  assert!(refusal(b"not a zip at all".to_vec()).starts_with("malformed presentation: "));
  assert!(refusal(Vec::new()).starts_with("malformed presentation: "));
}

#[test]
fn a_slide_that_cannot_be_read_is_left_out_and_said_to_be() {
  let deck = Deck::new()
    .slide(&text_box(0.0, 0.0, 9.0, 9.0, &para("one")))
    .slide("")
    .part("ppt/slides/slide2.xml", "<p:sld")
    .slide(&text_box(0.0, 0.0, 9.0, 9.0, &para("three")))
    .open();
  let numbers: Vec<u32> = deck.slides.iter().map(|slide| slide.number).collect();
  assert_eq!(numbers, [1, 3]);
  assert_eq!(deck.slide_count, 3);
  assert_eq!(deck.warnings.len(), 1);
  assert!(deck.warnings[0].starts_with("ppt/slides/slide2.xml was left out: "));
}

/// The visible characters a shape is set as on a page: its paragraphs with their bullets.
fn set_count(shape: &Shape) -> usize {
  let paragraphs =
    |paragraphs: &[Paragraph]| -> usize { paragraphs.iter().map(|p| visible(&p.line())).sum() };
  match shape {
    Shape::Text(shape) => paragraphs(&shape.paragraphs),
    Shape::Table(table) => table
      .rows
      .iter()
      .flat_map(|row| &row.cells)
      .map(|cell| paragraphs(&cell.paragraphs))
      .sum(),
    Shape::Group(group) => group.shapes.iter().map(set_count).sum(),
    Shape::Diagram(diagram) if diagram.shapes.is_empty() => {
      diagram.nodes.iter().map(|node| visible(node)).sum()
    }
    Shape::Diagram(diagram) => diagram.shapes.iter().map(set_count).sum(),
    Shape::Picture(_) | Shape::Chart(_) | Shape::Media(_) => 0,
  }
}

#[test]
fn a_layout_has_a_page_for_each_slide_and_every_word_on_it() {
  for name in FIXTURES {
    let deck = open(name);
    let pages = deck.layout();
    let placements = deck.placements();
    assert_eq!(pages.len(), deck.slides.len(), "{name}");
    for (page, slide) in pages.iter().zip(&deck.slides) {
      assert_eq!(page.page, slide.number, "{name}");
      assert_eq!(
        (page.width, page.height),
        (deck.width, deck.height),
        "{name}"
      );
      for word in &page.words {
        assert!(
          0.0 <= word.x0 && word.x0 < word.x1 && word.x1 <= page.width,
          "{name} slide {}: {word:?}",
          slide.number
        );
        assert!(
          0.0 <= word.y0 && word.y0 < word.y1 && word.y1 <= page.height,
          "{name} slide {}: {word:?}",
          slide.number
        );
      }
      // Nothing is lost in the setting: what is set is what was read, bullets and all.
      let set: usize = page.words.iter().map(|word| visible(&word.text)).sum();
      let read: usize = slide.shapes.iter().map(set_count).sum();
      assert_eq!(set, read, "{name} slide {}", slide.number);
      let lined: u32 = page.lines.iter().map(|line| line.word_count).sum();
      assert_eq!(
        lined as usize,
        page.words.len(),
        "{name} slide {}",
        slide.number
      );
      // Every picture of the slide is on the page, within it.
      let pictures = placements.iter().filter(|p| p.page == slide.number).count();
      assert_eq!(page.images.len(), pictures, "{name} slide {}", slide.number);
      for image in &page.images {
        assert!(
          0.0 <= image.x0 && image.x0 < image.x1 && image.x1 <= page.width,
          "{name} slide {}: {image:?}",
          slide.number
        );
        assert!(
          0.0 <= image.y0 && image.y0 < image.y1 && image.y1 <= page.height,
          "{name} slide {}: {image:?}",
          slide.number
        );
      }
    }
  }
}

#[test]
fn no_slide_of_the_fixtures_holds_a_date_a_footer_or_a_number_so_all_of_it_is_body() {
  for name in FIXTURES {
    let deck = open(name);
    let structured = deck.structured_text();
    assert_eq!(structured.len(), deck.slides.len(), "{name}");
    for (found, slide) in structured.iter().zip(&deck.slides) {
      assert_eq!(found.page, slide.number, "{name}");
      assert_eq!(
        (found.header.as_str(), found.footer.as_str()),
        ("", ""),
        "{name}"
      );
      assert_eq!(found.body, slide.text(), "{name} slide {}", slide.number);
    }
    let per_page = deck.text_per_page();
    assert_eq!(per_page.len(), deck.slides.len(), "{name}");
    let joined: Vec<&str> = per_page.iter().map(|(_, text)| text.as_str()).collect();
    assert_eq!(deck.text(), joined.join("\n\n"), "{name}");
  }
}

#[test]
fn a_table_is_a_grid_with_its_merged_ranges_named() {
  let cell = |attributes: &str, body: &str| {
    format!("<a:tc {attributes}><a:txBody><a:bodyPr/>{body}</a:txBody></a:tc>")
  };
  let table = format!(
    r#"<a:tbl><a:tblGrid><a:gridCol w="1270000"/><a:gridCol w="1270000"/><a:gridCol w="1905000"/></a:tblGrid>
       <a:tr h="381000">{}{}{}</a:tr><a:tr h="381000">{}{}{}</a:tr><a:tr h="381000">{}{}{}</a:tr></a:tbl>"#,
    cell(r#"gridSpan="2""#, &para("Item")),
    cell(r#"hMerge="1""#, ""),
    cell("", &para("Price")),
    cell(
      r#"rowSpan="2""#,
      &[para("Widget"), para("in two colors")].concat()
    ),
    cell("", &para("blue")),
    cell("", &para("10.00")),
    cell(r#"vMerge="1""#, ""),
    cell("", &para("red")),
    cell("", &para("12.00")),
  );
  let deck = Deck::new()
    .slide(&text_box(0.0, 0.0, 9.0, 9.0, &para("no table here")))
    .slide(&crate::testing::graphic_frame(
      (50.0, 60.0, 350.0, 90.0),
      &table,
    ))
    .open();
  let [grid] = deck.tables().try_into().unwrap();
  assert_eq!((grid.index, grid.page), (0, 2));
  assert_eq!((grid.row_count, grid.column_count), (3, 3));
  assert_eq!(grid.columns, [100.0, 100.0, 150.0]);
  assert_eq!(grid.merges, ["A1:B1", "A2:A3"]);
  let cells: Vec<(u32, u32, &str)> = grid
    .cells
    .iter()
    .map(|cell| (cell.row, cell.column, cell.text.as_str()))
    .collect();
  assert_eq!(
    cells,
    [
      (0, 0, "Item"),
      (0, 2, "Price"),
      // A cell keeps its paragraphs as lines.
      (1, 0, "Widget\nin two colors"),
      (1, 1, "blue"),
      (1, 2, "10.00"),
      (2, 1, "red"),
      (2, 2, "12.00"),
    ]
  );
  assert_eq!(deck.metadata().table_count, 1);
}

#[test]
fn charts_and_clips_are_listed_with_the_slide_they_are_on() {
  let film = format!(
    r#"<p:pic><p:nvPicPr><p:cNvPr id="6" name="Film"/><p:cNvPicPr/><p:nvPr><a:videoFile r:link="clip"/></p:nvPr></p:nvPicPr><p:blipFill><a:blip r:embed="poster"/></p:blipFill><p:spPr>{}</p:spPr></p:pic>"#,
    xfrm(50.0, 60.0, 160.0, 90.0)
  );
  let deck = Deck::new()
    .slide("")
    .slide(&crate::testing::graphic_frame(
      (10.0, 20.0, 300.0, 200.0),
      r#"<c:chart r:id="chart"/>"#,
    ))
    .rel("chart", "/chart", "ppt/charts/chart1.xml", false)
    .part(
      "ppt/charts/chart1.xml",
      r#"<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:chart><c:plotArea><c:pieChart><c:ser><c:tx><c:v>Share</c:v></c:tx><c:val><c:numRef><c:numCache><c:ptCount val="1"/><c:pt idx="0"><c:v>7</c:v></c:pt></c:numCache></c:numRef></c:val></c:ser></c:pieChart></c:plotArea></c:chart></c:chartSpace>"#,
    )
    .slide(&film)
    .rel("clip", "/video", "ppt/media/media1.mp4", false)
    .rel("poster", "/image", "ppt/media/image1.png", false)
    .part("ppt/media/media1.mp4", "film")
    .part("ppt/media/image1.png", "")
    .content_type("ppt/media/media1.mp4", "video/mp4")
    .open();
  let charts = deck.charts();
  let [chart] = charts.as_slice() else {
    panic!("one chart was expected");
  };
  assert_eq!(chart.page, 2);
  assert_eq!(chart.info.name.as_deref(), Some("Frame"));
  assert_eq!(chart.chart.series[0].kind, ooxmldown::ChartKind::Pie);
  assert_eq!(chart.chart.series[0].values, [Some(7.0)]);

  let [clip] = deck.clips().try_into().unwrap();
  assert_eq!((clip.page, clip.media, clip.poster), (3, 0, Some(0)));
  assert_eq!(
    clip.frame.map(|frame| (frame.x, frame.y)),
    Some((50.0, 60.0))
  );
  assert_eq!(deck.media[0].data, b"film");
  let metadata = deck.metadata();
  assert_eq!((metadata.chart_count, metadata.media_count), (1, 1));
  // A clip's poster is a picture of the deck but not a picture placed on the slide.
  assert_eq!((deck.images.len(), deck.placements().len()), (1, 0));
}

#[test]
fn words_are_counted_from_the_text_of_the_slides_and_not_from_what_repeats_on_them() {
  let data = r#"<dgm:dataModel xmlns:dgm="http://schemas.openxmlformats.org/drawingml/2006/diagram" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><dgm:ptLst><dgm:pt modelId="1"><dgm:t><a:p><a:r><a:t>Plan</a:t></a:r></a:p></dgm:t></dgm:pt><dgm:pt modelId="2"><dgm:t><a:p><a:r><a:t>Build</a:t></a:r></a:p><a:p><a:r><a:t>and test</a:t></a:r></a:p></dgm:t></dgm:pt></dgm:ptLst></dgm:dataModel>"#;
  let cell = |text: &str| format!("<a:tc><a:txBody>{}</a:txBody></a:tc>", para(text));
  let table = format!(
    r#"<a:tbl><a:tblGrid><a:gridCol w="1"/><a:gridCol w="1"/></a:tblGrid><a:tr h="1">{}{}</a:tr></a:tbl>"#,
    cell("Unit price"),
    cell("10.00"),
  );
  let tree = [
    placeholder(r#"type="title""#, &para("Two words")),
    // A bullet is not a word, and a paragraph with nothing in it is not a paragraph.
    text_box(
      0.0,
      100.0,
      9.0,
      9.0,
      r#"<a:p><a:pPr><a:buChar char="•"/></a:pPr><a:r><a:t>one  more</a:t></a:r></a:p><a:p/>"#,
    ),
    crate::testing::graphic_frame((0.0, 200.0, 9.0, 9.0), &table),
    crate::testing::graphic_frame((0.0, 300.0, 9.0, 9.0), r#"<dgm:relIds r:dm="data"/>"#),
    placeholder(r#"type="ftr" idx="11""#, &para("Acme, confidential")),
    placeholder(
      r#"type="sldNum" idx="12""#,
      r#"<a:p><a:fld id="{1}" type="slidenum"><a:t>1</a:t></a:fld></a:p>"#,
    ),
  ]
  .concat();
  let deck = Deck::new()
    .slide(&tree)
    .rel("data", "/diagramData", "ppt/diagrams/data1.xml", false)
    .part("ppt/diagrams/data1.xml", data)
    .notes(&para("These five words are notes."))
    .open();
  let metadata = deck.metadata();
  // Two words, one more, Unit price, 10.00, Plan, Build, and test: 2 + 2 + 2 + 1 + 1 + 1 + 2.
  assert_eq!(
    (
      metadata.word_count,
      metadata.paragraph_count,
      metadata.character_count
    ),
    (11, 7, 8 + 7 + 9 + 5 + 4 + 5 + 7)
  );
  assert_eq!(
    (
      metadata.table_count,
      metadata.diagram_count,
      metadata.notes_count
    ),
    (1, 1, 1)
  );
  // The same words `text()` returns, which has the bullet and not the footer or the number.
  let words = deck
    .text()
    .split_whitespace()
    .filter(|word| *word != "•")
    .count();
  assert_eq!(words, 11);
}

#[test]
fn a_deck_that_lists_more_than_ten_thousand_slides_is_a_limit_error() {
  let deck = |slides: usize| (0..slides).fold(Deck::new(), |deck, _| deck.slide(""));
  let error = deck(10_001).read().unwrap_err();
  assert_eq!(
    error.to_string(),
    "limit exceeded: the deck lists 10001 slides; the limit is 10000"
  );
  let full = deck(10_000).open();
  assert_eq!((full.slide_count, full.slides.len()), (10_000, 10_000));
}
