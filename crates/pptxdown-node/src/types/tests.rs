//! What JavaScript is given for decks put together here as plain data. The decks in the fixtures
//! folder hold no table, link, chart, clip, group or SmartArt, so these are where the shapes
//! JavaScript sees for those are checked.

use pptxdown as lib;

use super::*;

/// A slide is 960 by 540 points here.
const HEIGHT: f64 = 540.0;

fn frame(x: f64, y: f64, width: f64, height: f64) -> Option<lib::Frame> {
  Some(lib::Frame {
    x,
    y,
    width,
    height,
    rotation: 0.0,
  })
}

fn info(frame: Option<lib::Frame>, stacking: u32) -> lib::ShapeInfo {
  lib::ShapeInfo {
    frame,
    stacking,
    ..Default::default()
  }
}

fn run(text: &str, link: Option<lib::LinkTarget>) -> lib::Run {
  lib::Run {
    text: text.to_owned(),
    link,
    ..Default::default()
  }
}

fn paragraph(runs: Vec<lib::Run>) -> lib::Paragraph {
  lib::Paragraph {
    runs,
    ..Default::default()
  }
}

fn words(text: &str) -> Vec<lib::Paragraph> {
  vec![paragraph(vec![run(text, None)])]
}

fn text(info: lib::ShapeInfo, paragraphs: Vec<lib::Paragraph>) -> lib::Shape {
  lib::Shape::Text(lib::TextShape {
    info,
    paragraphs,
    insets: lib::Insets {
      left: 7.2,
      top: 3.6,
      right: 7.2,
      bottom: 3.6,
    },
    anchor: lib::Anchor::Top,
    wrap: true,
    font_scale: 1.0,
  })
}

fn slide(number: u32, shapes: Vec<lib::Shape>) -> lib::Slide {
  lib::Slide {
    number,
    name: format!("ppt/slides/slide{number}.xml"),
    title: None,
    layout: None,
    section: None,
    hidden: false,
    shapes,
    notes: Vec::new(),
    comments: Vec::new(),
  }
}

fn deck(slides: Vec<lib::Slide>) -> Presentation {
  Presentation {
    props: lib::DocProps::default(),
    has_macros: false,
    is_template: false,
    is_show: false,
    width: 960.0,
    height: HEIGHT,
    size_kind: None,
    language: None,
    slide_count: u32::try_from(slides.len()).unwrap(),
    slides,
    images: Vec::new(),
    media: Vec::new(),
    warnings: Vec::new(),
  }
}

fn image(name: &str, mime: &'static str, data: &[u8]) -> lib::Image {
  lib::Image {
    name: name.to_owned(),
    mime: Some(mime),
    width: Some(4),
    height: Some(3),
    data: data.to_vec(),
    linked: false,
  }
}

fn cell(column: u32, col_span: u32, row_span: u32, text: &str) -> lib::TableCell {
  lib::TableCell {
    column,
    col_span,
    row_span,
    paragraphs: words(text),
  }
}

#[test]
fn options_default_to_reading_hidden_slides() {
  assert!(options_of(None).hidden_slides);
  assert!(options_of(Some(PptxOptions::default())).hidden_slides);
  let off = PptxOptions {
    hidden_slides: Some(false),
  };
  assert!(!options_of(Some(off)).hidden_slides);
}

#[test]
fn a_slide_sized_for_a_screen_names_the_papers_it_lies_between() {
  let meta = metadata_of(&deck(vec![slide(1, Vec::new())]));
  assert_eq!(
    (meta.slide_count, meta.width, meta.height),
    (1, 960.0, 540.0)
  );
  // 960 by 540 is larger than A4 and smaller than Legal.
  let PaperSize::Unknown {
    closest_to: (Some(StandardPaperSize::A4 { .. }), Some(StandardPaperSize::Legal { .. })),
  } = meta.paper_size
  else {
    panic!("a size between A4 and Legal was expected");
  };
  let mut paper = deck(Vec::new());
  (paper.width, paper.height) = (792.0, 612.0);
  // The standard's own dimensions, portrait, whichever way the slide lies.
  let PaperSize::Letter { width, height } = metadata_of(&paper).paper_size else {
    panic!("Letter was expected");
  };
  assert_eq!((width, height), (612.0, 792.0));
}

#[test]
fn a_table_is_keyed_by_row_and_then_by_column_in_the_order_columns_stand() {
  let table = lib::Shape::Table(lib::Table {
    info: info(frame(50.0, 60.0, 350.0, 90.0), 0),
    columns: vec![100.0, 100.0, 150.0],
    rows: vec![
      lib::TableRow {
        height: 30.0,
        cells: vec![cell(0, 2, 1, "Item"), cell(2, 1, 1, "Price")],
      },
      // A row as wide as a sheet: the twenty-seventh column is AA, which comes after Z.
      lib::TableRow {
        height: 30.0,
        cells: vec![
          cell(0, 1, 1, "first"),
          cell(1, 1, 1, "second"),
          cell(25, 1, 1, "twenty-sixth"),
          cell(26, 1, 1, "twenty-seventh"),
        ],
      },
    ],
  });
  let tables = tables_of(&deck(vec![slide(1, Vec::new()), slide(2, vec![table])]));
  let [table] = tables.as_slice() else {
    panic!("one table was expected");
  };
  assert_eq!((table.index, table.page), (0, 2));
  assert_eq!((table.row_count, table.column_count), (2, 27));
  assert_eq!(table.merges, ["A1:B1"]);
  assert_eq!(
    table.frame.map(|frame| (frame.x, frame.y)),
    Some((50.0, 60.0))
  );
  let rows: Vec<&str> = table.rows.keys().map(String::as_str).collect();
  assert_eq!(rows, ["1", "2"]);
  let columns: Vec<&str> = table.rows["2"].keys().map(AsRef::as_ref).collect();
  assert_eq!(columns, ["A", "B", "Z", "AA"]);
  let found = &table.rows["2"][&ColumnKey("AA".to_owned())];
  assert_eq!(
    (
      found.reference.as_str(),
      found.row,
      found.column,
      found.text.as_str()
    ),
    ("AA2", 1, 26, "twenty-seventh")
  );
  let merged = &table.rows["1"][&ColumnKey("A".to_owned())];
  assert_eq!((merged.col_span, merged.row_span), (2, 1));
  // The cell a merged one covers has no key.
  assert!(!table.rows["1"].contains_key(&ColumnKey("B".to_owned())));
}

/// Where an annotation leads (an address, or a place in the deck), the text it covers, and its
/// box.
type Seen<'a> = (Option<&'a str>, Option<&'a str>, &'a str, Option<&'a [f64]>);

#[test]
fn a_link_has_its_box_in_the_page_frame_and_a_comment_has_none() {
  let external = lib::LinkTarget::External("https://example.com/terms".to_owned());
  let linked = text(
    info(frame(100.0, 50.0, 300.0, 200.0), 0),
    vec![paragraph(vec![
      run("See the ", None),
      run("terms", Some(external)),
    ])],
  );
  let button = text(
    lib::ShapeInfo {
      link: Some(lib::LinkTarget::Slide(2)),
      ..info(frame(0.0, 400.0, 100.0, 50.0), 1)
    },
    words("Next"),
  );
  let unplaced = text(
    lib::ShapeInfo {
      link: Some(lib::LinkTarget::Action("endshow".to_owned())),
      ..info(None, 2)
    },
    words("End"),
  );
  let mut first = slide(1, vec![linked, button, unplaced]);
  first.comments.push(lib::Comment {
    author: Some("Ana".to_owned()),
    initials: Some("A".to_owned()),
    date: Some("2026-11-15T09:30:00.000".to_owned()),
    text: "Is this agreed?".to_owned(),
  });
  let annotations = annotations_of(&deck(vec![first, slide(2, Vec::new())]));
  let seen: Vec<Seen<'_>> = annotations
    .iter()
    .map(|a| {
      (
        a.uri.as_deref(),
        a.dest.as_deref(),
        a.text.as_str(),
        a.rect.as_deref(),
      )
    })
    .collect();
  // A box 50 points down a slide of 540 and 200 tall runs from 290 to 490, measured up.
  let terms = [100.0, 290.0, 400.0, 490.0];
  let next = [0.0, 90.0, 100.0, 140.0];
  assert_eq!(
    seen,
    [
      (
        Some("https://example.com/terms"),
        None,
        "terms",
        Some(terms.as_slice())
      ),
      (None, Some("2"), "Next", Some(next.as_slice())),
      (None, Some("endshow"), "End", None),
      (None, None, "", None),
    ]
  );
  assert!(
    annotations[..3].iter().all(|a| {
      a.subtype == AnnotationSubtype::Link && a.part == PartKind::Slide && a.page == 1
    })
  );
  let comment = &annotations[3];
  assert_eq!(comment.subtype, AnnotationSubtype::Comment);
  assert_eq!(
    (
      comment.content.as_deref(),
      comment.author.as_deref(),
      comment.date.as_deref()
    ),
    (
      Some("Is this agreed?"),
      Some("Ana"),
      Some("2026-11-15T09:30:00.000")
    )
  );
}

/// A deck with one of everything on its first slide: a title, a picture drawn from an SVG that
/// is also the slide's background, a chart, SmartArt with no drawing, a clip with a poster, and
/// a group that holds a text box.
fn everything() -> Presentation {
  let title = text(
    lib::ShapeInfo {
      name: Some("Title 1".to_owned()),
      placeholder: Some(lib::Placeholder {
        kind: lib::PlaceholderKind::CenteredTitle,
        index: None,
      }),
      ..info(frame(50.0, 20.0, 860.0, 60.0), 5)
    },
    vec![lib::Paragraph {
      level: 1,
      bullet: Some("2.".to_owned()),
      ordered: true,
      align: Some(lib::Align::Center),
      runs: vec![lib::Run {
        text: "Revenue".to_owned(),
        bold: true,
        size: Some(44.0),
        font: Some("Arial".to_owned()),
        field: Some("slidenum".to_owned()),
        ..Default::default()
      }],
    }],
  );
  let picture = lib::Shape::Picture(lib::Picture {
    info: lib::ShapeInfo {
      alt_text: Some("A logo".to_owned()),
      ..info(frame(0.0, 0.0, 960.0, 540.0), 0)
    },
    image: 0,
    vector: Some(1),
    background: true,
  });
  let chart = lib::Shape::Chart(lib::ChartShape {
    info: info(frame(10.0, 100.0, 300.0, 200.0), 1),
    chart: lib::Chart {
      title: Some("Sales".to_owned()),
      series: vec![lib::ChartSeries {
        kind: lib::ChartKind::Line,
        name: Some("Actual".to_owned()),
        categories: vec!["North".to_owned(), "South".to_owned()],
        values: vec![Some(12.0), None],
      }],
    },
  });
  let diagram = lib::Shape::Diagram(lib::Diagram {
    info: info(frame(320.0, 100.0, 300.0, 200.0), 2),
    shapes: Vec::new(),
    nodes: vec!["Plan".to_owned(), "Build".to_owned()],
  });
  let clip = lib::Shape::Media(lib::MediaShape {
    info: info(frame(640.0, 100.0, 160.0, 90.0), 3),
    media: 0,
    poster: Some(2),
  });
  let group = lib::Shape::Group(lib::Group {
    info: info(frame(10.0, 320.0, 400.0, 100.0), 4),
    shapes: vec![text(
      info(frame(20.0, 330.0, 100.0, 20.0), 6),
      words("inside"),
    )],
  });
  let mut first = slide(1, vec![picture, title, chart, diagram, clip, group]);
  first.title = Some("Revenue".to_owned());
  first.layout = Some("Title Slide".to_owned());
  first.notes = words("Say this.");
  let mut deck = deck(vec![first, slide(2, Vec::new())]);
  deck.images = vec![
    image("ppt/media/image1.png", "image/png", b"png"),
    image("ppt/media/image2.svg", "image/svg+xml", b"<svg/>"),
    image("ppt/media/image3.jpeg", "image/jpeg", b"poster"),
  ];
  deck.media = vec![lib::Media {
    name: "ppt/media/media1.mp4".to_owned(),
    kind: lib::MediaKind::Video,
    mime: Some("video/mp4".to_owned()),
    data: b"film".to_vec(),
    linked: false,
  }];
  deck
}

#[test]
fn a_slide_lists_each_kind_of_shape_with_what_every_shape_has() {
  let slides = slides_of(&everything());
  let first = &slides[0];
  assert_eq!(
    (first.page, first.name.as_str(), first.title.as_deref()),
    (1, "ppt/slides/slide1.xml", Some("Revenue"))
  );
  assert_eq!(first.layout.as_deref(), Some("Title Slide"));
  assert_eq!(first.notes.as_deref(), Some("Say this."));
  // The slide's text has what its shapes hold; a picture, a chart and a clip hold none.
  assert_eq!(first.text, "2. Revenue\nPlan\nBuild\ninside");
  assert_eq!(slides[1].notes, None);

  let [
    Shape::Picture {
      image,
      name,
      vector,
      background,
      alt_text,
      stacking,
      ..
    },
    Shape::Text {
      text,
      paragraphs,
      shape_name,
      placeholder,
      frame,
      ..
    },
    Shape::Chart { title, series, .. },
    Shape::Diagram {
      text: diagram_text,
      shapes: drawn,
      nodes,
      ..
    },
    Shape::Media {
      media,
      name: clip_name,
      kind,
      mime,
      poster,
      ..
    },
    Shape::Group { shapes: held, .. },
  ] = first.shapes.as_slice()
  else {
    panic!("the six shapes were expected in the order the deck has them");
  };

  assert_eq!((*image, name.as_str()), (0, "ppt/media/image1.png"));
  assert_eq!(vector.as_deref(), Some("ppt/media/image2.svg"));
  assert_eq!(
    (*background, alt_text.as_deref(), *stacking),
    (Some(true), Some("A logo"), 0)
  );

  assert_eq!(text, "2. Revenue");
  assert_eq!(shape_name.as_deref(), Some("Title 1"));
  assert_eq!(*placeholder, Some(PlaceholderKind::CenteredTitle));
  assert_eq!(
    frame.map(|frame| (frame.x, frame.width)),
    Some((50.0, 860.0))
  );
  let [heading] = paragraphs.as_slice() else {
    panic!("one paragraph was expected");
  };
  assert_eq!(
    (
      heading.text.as_str(),
      heading.level,
      heading.bullet.as_deref(),
      heading.ordered
    ),
    ("Revenue", 1, Some("2."), true)
  );
  assert_eq!(heading.align, Some(Align::Center));
  let [bold] = heading.runs.as_slice() else {
    panic!("one run was expected");
  };
  // A flag is there only when it is set.
  assert_eq!(
    (bold.bold, bold.italic, bold.underline, bold.strike),
    (Some(true), None, None, None)
  );
  assert_eq!(
    (bold.size, bold.font.as_deref(), bold.field.as_deref()),
    (Some(44.0), Some("Arial"), Some("slidenum"))
  );

  assert_eq!(title.as_deref(), Some("Sales"));
  let [line] = series.as_slice() else {
    panic!("one series was expected");
  };
  assert_eq!(
    (line.kind, line.name.as_deref()),
    (ChartKind::Line, Some("Actual"))
  );
  assert_eq!(line.categories, ["North", "South"]);
  assert_eq!(line.values, [Some(12.0), None]);

  assert_eq!((diagram_text.as_str(), drawn.len()), ("Plan\nBuild", 0));
  assert_eq!(nodes, &["Plan", "Build"]);

  assert_eq!(
    (*media, clip_name.as_str(), *kind),
    (0, "ppt/media/media1.mp4", MediaKind::Video)
  );
  assert_eq!(mime.as_deref(), Some("video/mp4"));
  assert_eq!(poster.as_deref(), Some("ppt/media/image3.jpeg"));

  let [
    Shape::Text {
      text: inside,
      stacking: place,
      ..
    },
  ] = held.as_slice()
  else {
    panic!("a text shape was expected in the group");
  };
  assert_eq!((inside.as_str(), *place), ("inside", 6));
}

#[test]
fn pictures_and_clips_are_read_with_their_bytes_before_they_are_buffers() {
  let deck = everything();
  let images = images_read(&deck);
  let [image] = images.as_slice() else {
    panic!("one placed picture was expected");
  };
  assert_eq!((image.page, image.image_index), (1, 0));
  assert_eq!(
    (image.name.as_str(), image.mime.as_deref()),
    ("ppt/media/image1.png", Some("image/png"))
  );
  assert_eq!(
    (image.width, image.height, image.linked, image.background),
    (Some(4), Some(3), false, true)
  );
  assert_eq!(image.data, b"png");
  assert_eq!(image.alt_text.as_deref(), Some("A logo"));
  // The SVG the picture is drawn from comes with it; the clip's poster is not a placed picture.
  let vector = image.vector.as_ref().unwrap();
  assert_eq!(
    (
      vector.name.as_str(),
      vector.mime.as_deref(),
      vector.data.as_slice()
    ),
    (
      "ppt/media/image2.svg",
      Some("image/svg+xml"),
      b"<svg/>".as_slice()
    )
  );

  let clips = media_read(&deck);
  let [clip] = clips.as_slice() else {
    panic!("one clip was expected");
  };
  assert_eq!(
    (clip.page, clip.name.as_str(), clip.kind),
    (1, "ppt/media/media1.mp4", MediaKind::Video)
  );
  assert_eq!(
    (clip.mime.as_deref(), clip.data.as_slice(), clip.linked),
    (Some("video/mp4"), b"film".as_slice(), false)
  );
  assert_eq!(clip.poster.as_deref(), Some("ppt/media/image3.jpeg"));
  assert_eq!(
    clip.frame.map(|frame| (frame.x, frame.y)),
    Some((640.0, 100.0))
  );

  let read = document_read(&deck);
  assert_eq!(
    (read.images.len(), read.media.len(), read.slides.len()),
    (1, 1, 2)
  );
  assert_eq!(
    (
      read.metadata.image_count,
      read.metadata.media_count,
      read.metadata.chart_count
    ),
    (1, 1, 1)
  );
}

#[test]
fn charts_and_notes_are_listed_with_the_slide_they_are_on() {
  let deck = everything();
  let charts = charts_of(&deck);
  let [chart] = charts.as_slice() else {
    panic!("one chart was expected");
  };
  assert_eq!(
    (chart.index, chart.page, chart.title.as_deref()),
    (0, 1, Some("Sales"))
  );
  assert_eq!(
    chart.frame.map(|frame| (frame.width, frame.height)),
    Some((300.0, 200.0))
  );
  assert_eq!(chart.series.len(), 1);

  let notes = notes_of(&deck);
  let [note] = notes.as_slice() else {
    panic!("notes for one slide were expected");
  };
  assert_eq!(
    (note.page, note.text.as_str(), note.paragraphs.len()),
    (1, "Say this.", 1)
  );

  let pages = text_per_page_of(&deck);
  assert_eq!(pages.len(), 2);
  assert_eq!((pages[1].page, pages[1].text.as_str()), (2, ""));
  let structured = structured_text_of(&deck);
  assert_eq!(
    (structured[0].header.as_str(), structured[0].footer.as_str()),
    ("", "")
  );
  assert_eq!(structured[0].body, pages[0].text);
}
