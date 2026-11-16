use ooxmldown::ChartKind;

use crate::error::PresentationError;
use crate::model::{Frame, MediaKind, Shape};
use crate::testing::{Deck, emu, graphic_frame, para, placeholder, shape, text_box, xfrm};

fn frame(x: f64, y: f64, width: f64, height: f64) -> Option<Frame> {
  Some(Frame {
    x,
    y,
    width,
    height,
    rotation: 0.0,
  })
}

/// What a shape is, in a word.
fn kind(shape: &Shape) -> &'static str {
  match shape {
    Shape::Text(_) => "text",
    Shape::Picture(_) => "picture",
    Shape::Table(_) => "table",
    Shape::Chart(_) => "chart",
    Shape::Diagram(_) => "diagram",
    Shape::Media(_) => "media",
    Shape::Group(_) => "group",
  }
}

/// A group whose own box and whose children's space are given in points.
fn group(own: (f64, f64, f64, f64), space: (f64, f64, f64, f64), inner: &str) -> String {
  format!(
    r#"<p:grpSp><p:nvGrpSpPr><p:cNvPr id="9" name="Group"/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/><a:chOff x="{}" y="{}"/><a:chExt cx="{}" cy="{}"/></a:xfrm></p:grpSpPr>{inner}</p:grpSp>"#,
    emu(own.0),
    emu(own.1),
    emu(own.2),
    emu(own.3),
    emu(space.0),
    emu(space.1),
    emu(space.2),
    emu(space.3),
  )
}

#[test]
fn a_group_moves_and_scales_what_it_holds_and_so_does_a_group_inside_it() {
  // The outer group is shown at half the size its children were placed at. The inner one is
  // not resized, but its children are written a thousand points away from where they are.
  let inner = group(
    (200.0, 100.0, 100.0, 50.0),
    (1000.0, 1000.0, 100.0, 50.0),
    &text_box(1010.0, 1005.0, 20.0, 10.0, &para("deep")),
  );
  let tree = group(
    (100.0, 100.0, 200.0, 100.0),
    (0.0, 0.0, 400.0, 200.0),
    &[text_box(40.0, 20.0, 100.0, 40.0, &para("child")), inner].concat(),
  );
  let deck = Deck::new().slide(&tree).open();
  let Shape::Group(outer) = &deck.slides[0].shapes[0] else {
    panic!("a group was expected");
  };
  assert_eq!(outer.info.frame, frame(100.0, 100.0, 200.0, 100.0));
  assert_eq!(outer.info.name.as_deref(), Some("Group"));
  assert_eq!(
    outer.shapes[0].info().frame,
    frame(120.0, 110.0, 50.0, 20.0)
  );
  let Shape::Group(inner) = &outer.shapes[1] else {
    panic!("a group was expected inside the group");
  };
  assert_eq!(inner.info.frame, frame(200.0, 150.0, 50.0, 25.0));
  assert_eq!(inner.shapes[0].info().frame, frame(205.0, 152.5, 10.0, 5.0));
  assert_eq!(deck.slides[0].text(), "child\ndeep");
}

#[test]
fn shapes_are_read_title_first_then_in_rows_from_the_top() {
  // Stored back to front: the caption that was added last is stored last.
  let tree = [
    text_box(50.0, 300.0, 400.0, 100.0, &para("lower")),
    text_box(500.0, 100.0, 400.0, 150.0, &para("right")),
    text_box(50.0, 105.0, 400.0, 150.0, &para("left")),
    placeholder(r#"type="ftr" idx="11""#, &para("no box")),
    shape(
      r#"<p:ph type="title"/>"#,
      &xfrm(50.0, 400.0, 800.0, 60.0),
      &para("title"),
    ),
    text_box(50.0, 20.0, 400.0, 30.0, &para("caption")),
  ]
  .concat();
  let deck = Deck::new().slide(&tree).open();
  let read: Vec<(String, u32)> = deck.slides[0]
    .shapes
    .iter()
    .map(|shape| (shape.text(), shape.info().stacking))
    .collect();
  let at = |text: &str, stacking: u32| (text.to_owned(), stacking);
  // "left" starts five points below "right", which is the same row; a shape with no box
  // comes after the ones that have one.
  assert_eq!(
    read,
    [
      at("title", 4),
      at("caption", 5),
      at("left", 2),
      at("right", 1),
      at("lower", 0),
      at("no box", 3)
    ]
  );
}

fn picture(name: &str, non_visual: &str, blip: &str, at: (f64, f64, f64, f64)) -> String {
  format!(
    r#"<p:pic><p:nvPicPr><p:cNvPr id="4" name="{name}" descr="A {name}"/><p:cNvPicPr/><p:nvPr>{non_visual}</p:nvPr></p:nvPicPr><p:blipFill>{blip}</p:blipFill><p:spPr>{}</p:spPr></p:pic>"#,
    xfrm(at.0, at.1, at.2, at.3)
  )
}

#[test]
fn pictures_clips_and_the_background_are_found_with_their_boxes() {
  let tree = [
    picture("logo", "", r#"<a:blip r:embed="png"/>"#, (10.0, 10.0, 100.0, 50.0)),
    picture(
      "drawing",
      "",
      r#"<a:blip r:embed="png"><a:extLst><a:ext uri="{96DAC541}"><asvg:svgBlip r:embed="svg"/></a:ext></a:extLst></a:blip>"#,
      (10.0, 100.0, 100.0, 50.0),
    ),
    picture("remote", "", r#"<a:blip r:link="web"/>"#, (10.0, 200.0, 100.0, 50.0)),
    picture(
      "film",
      r#"<a:videoFile r:link="clipLink"/><p:extLst><p:ext uri="{DAA4B4D4}"><p14:media r:embed="clip"/></p:ext></p:extLst>"#,
      r#"<a:blip r:embed="poster"/>"#,
      (10.0, 300.0, 160.0, 90.0),
    ),
    picture("sound", r#"<a:audioFile r:link="tune"/>"#, "", (10.0, 400.0, 20.0, 20.0)),
    picture("lost", "", r#"<a:blip r:embed="nothing"/>"#, (0.0, 0.0, 1.0, 1.0)),
  ]
  .concat();
  let deck = Deck::new()
    .slide_with(
      "",
      r#"<p:bg><p:bgPr><a:blipFill><a:blip r:embed="wall"/></a:blipFill></p:bgPr></p:bg>"#,
      &tree,
    )
    .rel("png", "/image", "ppt/media/image1.png", false)
    .rel("svg", "/image", "ppt/media/image2.svg", false)
    .rel("web", "/image", "https://example.com/remote.png", true)
    .rel("poster", "/image", "ppt/media/image3.png", false)
    .rel("clip", "/media", "ppt/media/media1.mp4", false)
    .rel("clipLink", "/video", "ppt/media/media1.mp4", false)
    .rel("tune", "/audio", "https://example.com/tune.mp3", true)
    .rel("wall", "/image", "ppt/media/image4.jpeg", false)
    .part("ppt/media/media1.mp4", "not really a film")
    .content_type("ppt/media/media1.mp4", "video/mp4")
    .open();
  let shapes = &deck.slides[0].shapes;
  let kinds: Vec<&str> = shapes.iter().map(kind).collect();
  assert_eq!(
    kinds,
    ["picture", "picture", "picture", "picture", "media", "media"]
  );

  let names: Vec<(&str, bool)> = deck
    .images
    .iter()
    .map(|i| (i.name.as_str(), i.linked))
    .collect();
  // The same picture used twice is one image; a picture whose part is missing is still listed.
  assert_eq!(
    names,
    [
      ("ppt/media/image4.jpeg", false),
      ("ppt/media/image1.png", false),
      ("ppt/media/image2.svg", false),
      ("https://example.com/remote.png", true),
      ("ppt/media/image3.png", false),
    ]
  );
  let Shape::Picture(wall) = &shapes[0] else {
    panic!("the background was expected first");
  };
  assert!(wall.background);
  assert_eq!(wall.info.frame, frame(0.0, 0.0, 960.0, 540.0));
  let Shape::Picture(drawing) = &shapes[2] else {
    panic!("a picture was expected");
  };
  assert_eq!(
    (drawing.image, drawing.vector, drawing.background),
    (1, Some(2), false)
  );
  assert_eq!(drawing.info.alt_text.as_deref(), Some("A drawing"));
  assert_eq!(drawing.info.frame, frame(10.0, 100.0, 100.0, 50.0));

  let Shape::Media(film) = &shapes[4] else {
    panic!("a clip was expected");
  };
  assert_eq!((film.media, film.poster), (0, Some(4)));
  let clips: Vec<(&str, MediaKind, Option<&str>, usize, bool)> = deck
    .media
    .iter()
    .map(|m| {
      (
        m.name.as_str(),
        m.kind,
        m.mime.as_deref(),
        m.data.len(),
        m.linked,
      )
    })
    .collect();
  assert_eq!(
    clips,
    [
      (
        "ppt/media/media1.mp4",
        MediaKind::Video,
        Some("video/mp4"),
        17,
        false
      ),
      (
        "https://example.com/tune.mp3",
        MediaKind::Audio,
        None,
        0,
        true
      ),
    ]
  );
  // The parts that could not be read are named.
  assert_eq!(deck.warnings.len(), 4);
  assert!(deck.warnings.iter().all(|w| w.contains("ppt/media/image")));
}

#[test]
fn a_table_keeps_the_cells_that_cover_and_drops_the_ones_they_cover() {
  let cell = |attributes: &str, text: &str| {
    format!(
      "<a:tc {attributes}><a:txBody><a:bodyPr/>{}</a:txBody><a:tcPr/></a:tc>",
      para(text)
    )
  };
  let table = format!(
    r#"<a:tbl><a:tblPr firstRow="1"/><a:tblGrid><a:gridCol w="{w}"/><a:gridCol w="{w}"/><a:gridCol w="{wide}"/></a:tblGrid>
       <a:tr h="{h}">{}{}{}</a:tr><a:tr h="{h}">{}{}{}</a:tr><a:tr h="{h}">{}{}{}</a:tr></a:tbl>"#,
    cell(r#"gridSpan="2""#, "Item"),
    cell(r#"hMerge="1""#, ""),
    cell("", "Price"),
    cell(r#"rowSpan="2""#, "Widget"),
    cell("", "blue"),
    cell("", "10.00"),
    cell(r#"vMerge="1""#, ""),
    cell("", "red"),
    cell("", "12.00"),
    w = emu(100.0),
    wide = emu(150.0),
    h = emu(30.0),
  );
  let deck = Deck::new()
    .slide(&graphic_frame((50.0, 60.0, 350.0, 90.0), &table))
    .open();
  let Shape::Table(table) = &deck.slides[0].shapes[0] else {
    panic!("a table was expected");
  };
  assert_eq!(table.info.frame, frame(50.0, 60.0, 350.0, 90.0));
  assert_eq!(table.columns, [100.0, 100.0, 150.0]);
  let cells: Vec<Vec<(String, u32, u32, u32)>> = table
    .rows
    .iter()
    .map(|row| {
      row
        .cells
        .iter()
        .map(|c| (c.text(), c.column, c.col_span, c.row_span))
        .collect()
    })
    .collect();
  let at = |text: &str, column, col_span, row_span| (text.to_owned(), column, col_span, row_span);
  assert_eq!(
    cells,
    [
      vec![at("Item", 0, 2, 1), at("Price", 2, 1, 1)],
      vec![
        at("Widget", 0, 1, 2),
        at("blue", 1, 1, 1),
        at("10.00", 2, 1, 1)
      ],
      vec![at("red", 1, 1, 1), at("12.00", 2, 1, 1)],
    ]
  );
  assert_eq!(table.rows[0].height, 30.0);
  assert_eq!(
    deck.slides[0].text(),
    "Item\tPrice\nWidget\tblue\t10.00\nred\t12.00"
  );
}

const CHART: &str = r#"<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:chart><c:plotArea><c:lineChart><c:ser><c:tx><c:v>Sales</c:v></c:tx><c:val><c:numRef><c:numCache><c:ptCount val="2"/><c:pt idx="0"><c:v>3</c:v></c:pt><c:pt idx="1"><c:v>4</c:v></c:pt></c:numCache></c:numRef></c:val></c:ser></c:lineChart></c:plotArea></c:chart></c:chartSpace>"#;

#[test]
fn a_chart_is_read_from_its_part_as_data() {
  let tree = [
    graphic_frame((10.0, 20.0, 300.0, 200.0), r#"<c:chart r:id="chart"/>"#),
    graphic_frame((10.0, 240.0, 300.0, 200.0), r#"<c:chart r:id="broken"/>"#),
    graphic_frame((10.0, 460.0, 300.0, 20.0), r#"<c:chart r:id="absent"/>"#),
  ]
  .concat();
  let deck = Deck::new()
    .slide(&tree)
    .rel("chart", "/chart", "ppt/charts/chart1.xml", false)
    .rel("broken", "/chart", "ppt/charts/chart2.xml", false)
    .part("ppt/charts/chart1.xml", CHART)
    .part("ppt/charts/chart2.xml", "<c:chartSpace")
    .open();
  assert_eq!(deck.slides[0].shapes.len(), 1);
  let Shape::Chart(found) = &deck.slides[0].shapes[0] else {
    panic!("a chart was expected");
  };
  assert_eq!(found.info.frame, frame(10.0, 20.0, 300.0, 200.0));
  let series = &found.chart.series[0];
  assert_eq!(
    (
      series.kind,
      series.name.as_deref(),
      series.values.as_slice()
    ),
    (
      ChartKind::Line,
      Some("Sales"),
      [Some(3.0), Some(4.0)].as_slice()
    )
  );
  // A chart is data, not text; and a chart that cannot be read is said to be left out.
  assert_eq!(deck.slides[0].text(), "");
  assert_eq!(deck.warnings.len(), 1);
  assert!(deck.warnings[0].starts_with("ppt/charts/chart2.xml was left out: "));
}

const DIAGRAM_DATA: &str = r#"<dgm:dataModel xmlns:dgm="http://schemas.openxmlformats.org/drawingml/2006/diagram" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:dsp="http://schemas.microsoft.com/office/drawing/2008/diagram"><dgm:ptLst>
    <dgm:pt modelId="0" type="doc"/>
    <dgm:pt modelId="1"><dgm:t><a:bodyPr/><a:p><a:r><a:t>Plan</a:t></a:r></a:p></dgm:t></dgm:pt>
    <dgm:pt modelId="2" type="node"><dgm:t><a:bodyPr/><a:p><a:r><a:t>Build</a:t></a:r></a:p><a:p><a:r><a:t>and test</a:t></a:r></a:p></dgm:t></dgm:pt>
    <dgm:pt modelId="3" type="parTrans"><dgm:t><a:bodyPr/><a:p><a:r><a:t>link text</a:t></a:r></a:p></dgm:t></dgm:pt>
    <dgm:pt modelId="4" type="pres"/>
  </dgm:ptLst>EXT</dgm:dataModel>"#;

const DIAGRAM_DRAWING: &str = r#"<dsp:drawing xmlns:dsp="http://schemas.microsoft.com/office/drawing/2008/diagram" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><dsp:spTree><dsp:nvGrpSpPr/><dsp:grpSpPr/>
    <dsp:sp modelId="{2}"><dsp:nvSpPr><dsp:cNvPr id="0" name=""/><dsp:cNvSpPr/></dsp:nvSpPr><dsp:spPr><a:xfrm><a:off x="1270000" y="0"/><a:ext cx="1270000" cy="635000"/></a:xfrm></dsp:spPr><dsp:txBody><a:bodyPr/><a:p><a:r><a:t>Build</a:t></a:r></a:p><a:p><a:r><a:t>and test</a:t></a:r></a:p></dsp:txBody></dsp:sp>
    <dsp:sp modelId="{1}"><dsp:nvSpPr><dsp:cNvPr id="0" name=""/><dsp:cNvSpPr/></dsp:nvSpPr><dsp:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="1270000" cy="635000"/></a:xfrm></dsp:spPr><dsp:txBody><a:bodyPr/><a:p><a:r><a:t>Plan</a:t></a:r></a:p></dsp:txBody></dsp:sp>
  </dsp:spTree></dsp:drawing>"#;

#[test]
fn smartart_is_read_from_its_drawing_and_from_its_nodes_when_it_has_none() {
  let frame_xml = |id: &str| {
    graphic_frame(
      (100.0, 200.0, 400.0, 100.0),
      &format!(r#"<dgm:relIds r:dm="{id}" r:lo="lo" r:qs="qs" r:cs="cs"/>"#),
    )
  };
  let named = DIAGRAM_DATA.replace(
    "EXT",
    r#"<dgm:extLst><a:ext uri="x"><dsp:dataModelExt relId="drawn" minVer="x"/></a:ext></dgm:extLst>"#,
  );
  let deck = Deck::new()
    .slide(&[frame_xml("data"), frame_xml("bare")].concat())
    .rel("data", "/diagramData", "ppt/diagrams/data1.xml", false)
    .rel(
      "drawn",
      "/diagramDrawing",
      "ppt/diagrams/drawing1.xml",
      false,
    )
    .rel("bare", "/diagramData", "ppt/diagrams/data2.xml", false)
    .part("ppt/diagrams/data1.xml", &named)
    .part("ppt/diagrams/drawing1.xml", DIAGRAM_DRAWING)
    .part("ppt/diagrams/data2.xml", &DIAGRAM_DATA.replace("EXT", ""))
    .open();
  let [Shape::Diagram(drawn), Shape::Diagram(bare)] = deck.slides[0].shapes.as_slice() else {
    panic!("two diagrams were expected");
  };
  // The drawing's boxes are measured from the diagram's own corner, and its shapes are put in
  // reading order like a slide's.
  let boxes: Vec<(String, Option<Frame>)> = drawn
    .shapes
    .iter()
    .map(|shape| (shape.text(), shape.info().frame))
    .collect();
  assert_eq!(
    boxes,
    [
      ("Plan".to_owned(), frame(100.0, 200.0, 100.0, 50.0)),
      (
        "Build\nand test".to_owned(),
        frame(200.0, 200.0, 100.0, 50.0)
      ),
    ]
  );
  assert!(drawn.nodes.is_empty());
  // With no drawing, the nodes' text stands in: the points that are nodes, in the file's order.
  assert!(bare.shapes.is_empty());
  assert_eq!(bare.nodes, ["Plan", "Build\nand test"]);
  assert_eq!(
    deck.slides[0].text(),
    "Plan\nBuild\nand test\nPlan\nBuild\nand test"
  );
}

#[test]
fn a_shape_kept_two_ways_is_read_once_and_a_shape_with_no_text_is_not_a_shape() {
  let tree = format!(
    r#"<mc:AlternateContent><mc:Choice Requires="p14">{}</mc:Choice><mc:Fallback>{}</mc:Fallback></mc:AlternateContent>
       <mc:AlternateContent><mc:Fallback>{}</mc:Fallback></mc:AlternateContent>
       {}{}<p:cxnSp><p:nvCxnSpPr><p:cNvPr id="7" name="Line"/></p:nvCxnSpPr><p:spPr/></p:cxnSp>"#,
    text_box(0.0, 0.0, 10.0, 10.0, &para("choice")),
    text_box(0.0, 0.0, 10.0, 10.0, &para("fallback")),
    text_box(0.0, 20.0, 10.0, 10.0, &para("only a fallback")),
    text_box(0.0, 40.0, 10.0, 10.0, "<a:p/>"),
    r#"<p:sp><p:nvSpPr><p:cNvPr id="3" name="Rectangle"/></p:nvSpPr><p:spPr/></p:sp>"#,
  );
  let deck = Deck::new().slide(&tree).open();
  let texts: Vec<String> = deck.slides[0].shapes.iter().map(Shape::text).collect();
  assert_eq!(texts, ["choice", "only a fallback"]);
}

#[test]
fn groups_nested_past_the_limit_are_a_limit_error() {
  let nest = |depth: usize| {
    format!(
      "{}{}{}",
      "<p:grpSp><p:grpSpPr/>".repeat(depth),
      text_box(0.0, 0.0, 10.0, 10.0, &para("deep")),
      "</p:grpSp>".repeat(depth)
    )
  };
  let error = Deck::new().slide(&nest(40)).read().unwrap_err();
  assert!(
    matches!(error, PresentationError::LimitExceeded(_)),
    "{error}"
  );
  assert_eq!(
    error.to_string(),
    "limit exceeded: shapes are grouped more than 32 levels deep"
  );
  assert_eq!(Deck::new().slide(&nest(5)).open().slides[0].text(), "deep");
}

#[test]
fn a_slide_may_hold_fifty_thousand_shapes_and_no_more() {
  let shape = "<p:sp><p:txBody><a:p><a:r><a:t>x</a:t></a:r></a:p></p:txBody></p:sp>";
  let full = Deck::new().slide(&shape.repeat(50_000)).open();
  assert_eq!(full.slides[0].shapes.len(), 50_000);
  let error = Deck::new().slide(&shape.repeat(50_001)).read().unwrap_err();
  assert_eq!(
    error.to_string(),
    "limit exceeded: a slide holds more than 50000 shapes"
  );
}
