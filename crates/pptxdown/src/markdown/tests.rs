use crate::testing::{Deck, graphic_frame, para, placeholder, shape, text_box, xfrm};

const TEXT_STYLES: &str = r#"<p:bodyStyle><a:lvl1pPr><a:buChar char="•"/></a:lvl1pPr><a:lvl2pPr><a:buChar char="–"/></a:lvl2pPr></p:bodyStyle>"#;

/// A layout with a title across the top and a body under it.
fn layout() -> String {
  [
    shape(
      r#"<p:ph type="title"/>"#,
      &xfrm(50.0, 20.0, 860.0, 60.0),
      "",
    ),
    shape(r#"<p:ph idx="1"/>"#, &xfrm(50.0, 100.0, 860.0, 150.0), ""),
    shape(
      r#"<p:ph type="ftr" idx="11"/>"#,
      &xfrm(300.0, 500.0, 360.0, 30.0),
      "",
    ),
  ]
  .concat()
}

#[test]
fn a_slide_is_a_heading_then_what_it_holds_then_its_notes() {
  let body = [
    r#"<a:p><a:r><a:t>North grew </a:t></a:r><a:r><a:rPr b="1"/><a:t>12%</a:t></a:r></a:p>"#,
    r#"<a:p><a:pPr lvl="1"/><a:r><a:t>driven by two accounts</a:t></a:r></a:p>"#,
    r#"<a:p><a:r><a:t>South was flat, see the </a:t></a:r><a:r><a:rPr><a:hlinkClick r:id="notes"/></a:rPr><a:t>notes</a:t></a:r></a:p>"#,
  ]
  .concat();
  let first = [
    // Stored last-to-first: the order below is the order the slide is read in.
    text_box(50.0, 400.0, 400.0, 30.0, &para("1. not a list")),
    text_box(50.0, 300.0, 400.0, 30.0, &para("Total due: 25.50 USD")),
    placeholder(r#"type="ftr" idx="11""#, &para("Acme, confidential")),
    placeholder(r#"idx="1""#, &body),
    placeholder(r#"type="title""#, &para("Revenue *by* region")),
  ]
  .concat();
  let numbered = |scheme: &str, text: &str| {
    format!(
      r#"<a:p><a:pPr><a:buAutoNum type="{scheme}"/></a:pPr><a:r><a:t>{text}</a:t></a:r></a:p>"#
    )
  };
  let second = text_box(
    50.0,
    100.0,
    400.0,
    200.0,
    &[
      numbered("arabicPeriod", "First"),
      numbered("arabicPeriod", "Second"),
      numbered("alphaLcParenR", "a letter"),
    ]
    .concat(),
  );
  let deck = Deck::new()
    .master("", TEXT_STYLES)
    .layout("Title and Content", &layout())
    .slide(&first)
    .rel("notes", "/hyperlink", "https://example.com/notes", true)
    .notes(&[para("Mention the accounts."), para("Pause.")].concat())
    .slide(&second)
    .open();
  assert_eq!(
    deck.markdown(),
    [
      r"## Revenue \*by\* region",
      "",
      "- North grew **12%**",
      "    - driven by two accounts",
      "- South was flat, see the [notes](<https://example.com/notes>)",
      "",
      "Total due: 25.50 USD",
      "",
      r"1\. not a list",
      "",
      "> **Notes:** Mention the accounts.",
      ">",
      "> Pause.",
      "",
      // A slide with no title is named by its number. A number keeps its value; a letter has
      // no Markdown form, so it stays as text.
      "## Slide 2",
      "",
      "1. First",
      "2. Second",
      "- a) a letter",
    ]
    .join("\n")
  );
}

const CHART: &str = r#"<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><c:chart><c:title><c:tx><c:rich><a:p><a:r><a:t>Sales | units</a:t></a:r></a:p></c:rich></c:tx></c:title><c:plotArea><c:barChart>
  <c:ser><c:tx><c:v>Actual</c:v></c:tx><c:cat><c:strRef><c:strCache><c:ptCount val="2"/><c:pt idx="0"><c:v>North</c:v></c:pt><c:pt idx="1"><c:v>South</c:v></c:pt></c:strCache></c:strRef></c:cat><c:val><c:numRef><c:numCache><c:ptCount val="2"/><c:pt idx="0"><c:v>12</c:v></c:pt><c:pt idx="1"><c:v>15.5</c:v></c:pt></c:numCache></c:numRef></c:val></c:ser>
  <c:ser><c:val><c:numRef><c:numCache><c:ptCount val="2"/><c:pt idx="0"><c:v>10</c:v></c:pt></c:numCache></c:numRef></c:val></c:ser>
</c:barChart></c:plotArea></c:chart></c:chartSpace>"#;

#[test]
fn a_table_is_a_pipe_table_a_chart_a_table_of_its_data_and_a_picture_a_picture() {
  let cell =
    |attributes: &str, body: &str| format!("<a:tc {attributes}><a:txBody>{body}</a:txBody></a:tc>");
  let table = format!(
    "<a:tbl><a:tblGrid><a:gridCol w=\"1\"/><a:gridCol w=\"1\"/><a:gridCol w=\"1\"/></a:tblGrid><a:tr h=\"1\">{}{}{}</a:tr><a:tr h=\"1\">{}{}{}</a:tr></a:tbl>",
    cell(r#"gridSpan="2""#, &para("Item")),
    cell(r#"hMerge="1""#, ""),
    cell("", &para("Price")),
    cell("", &[para("Widget"), para("blue | large")].concat()),
    cell("", ""),
    cell(
      "",
      r#"<a:p><a:r><a:rPr b="1"/><a:t>10.00</a:t></a:r></a:p>"#
    ),
  );
  let picture = |name: &str, id: &str, y: f64| {
    format!(
      r#"<p:pic><p:nvPicPr><p:cNvPr id="4" name="{name}" descr="A [{name}]"/><p:nvPr/></p:nvPicPr><p:blipFill><a:blip r:embed="{id}"/></p:blipFill><p:spPr>{}</p:spPr></p:pic>"#,
      xfrm(50.0, y, 100.0, 40.0)
    )
  };
  let film = format!(
    r#"<p:pic><p:nvPicPr><p:cNvPr id="6" name="Film"/><p:nvPr><a:videoFile r:link="clip"/></p:nvPr></p:nvPicPr><p:blipFill/><p:spPr>{}</p:spPr></p:pic>"#,
    xfrm(50.0, 460.0, 100.0, 40.0)
  );
  let tree = [
    graphic_frame((50.0, 50.0, 300.0, 60.0), &table),
    graphic_frame((50.0, 200.0, 300.0, 150.0), r#"<c:chart r:id="chart"/>"#),
    picture("logo", "png", 400.0),
    film,
  ]
  .concat();
  let deck = Deck::new()
    .slide_with(
      "",
      r#"<p:bg><p:bgPr><a:blipFill><a:blip r:embed="wall"/></a:blipFill></p:bgPr></p:bg>"#,
      &tree,
    )
    .rel("chart", "/chart", "ppt/charts/chart1.xml", false)
    .rel("png", "/image", "ppt/media/image 1.png", false)
    .rel("wall", "/image", "ppt/media/image2.jpeg", false)
    .rel("clip", "/video", "ppt/media/media1.mp4", false)
    .part("ppt/charts/chart1.xml", CHART)
    .part("ppt/media/image 1.png", "")
    .part("ppt/media/image2.jpeg", "")
    .part("ppt/media/media1.mp4", "")
    .open();
  assert_eq!(
    deck.markdown(),
    [
      "## Slide 1",
      "",
      // A merged cell is one filled cell with an empty one beside it.
      "| Item |  | Price |",
      "| --- | --- | --- |",
      r"| Widget<br>blue \| large |  | **10.00** |",
      "",
      r"**Sales | units**",
      "",
      "|  | Actual | Series 2 |",
      "| --- | --- | --- |",
      "| North | 12 | 10 |",
      "| South | 15.5 |  |",
      "",
      r"![A \[logo\]](<ppt/media/image 1.png>)",
      "",
      "[video](<ppt/media/media1.mp4>)",
    ]
    .join("\n")
  );
}

#[test]
fn what_a_group_and_a_diagram_hold_is_written_where_they_stand() {
  let data = r#"<dgm:dataModel xmlns:dgm="http://schemas.openxmlformats.org/drawingml/2006/diagram" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><dgm:ptLst><dgm:pt modelId="1"><dgm:t><a:p><a:r><a:t>Plan</a:t></a:r></a:p></dgm:t></dgm:pt><dgm:pt modelId="2"><dgm:t><a:p><a:r><a:t>Build *fast*</a:t></a:r></a:p></dgm:t></dgm:pt></dgm:ptLst></dgm:dataModel>"#;
  let group = format!(
    "<p:grpSp><p:grpSpPr>{}</p:grpSpPr>{}{}</p:grpSp>",
    xfrm(50.0, 50.0, 400.0, 100.0),
    text_box(50.0, 100.0, 100.0, 20.0, &para("lower in the group")),
    text_box(50.0, 50.0, 100.0, 20.0, &para("upper in the group")),
  );
  let tree = [
    group,
    graphic_frame((50.0, 300.0, 400.0, 100.0), r#"<dgm:relIds r:dm="data"/>"#),
    text_box(
      50.0,
      450.0,
      100.0,
      20.0,
      "<a:p><a:r><a:t>line one</a:t></a:r><a:br/><a:r><a:t>line two</a:t></a:r></a:p>",
    ),
  ]
  .concat();
  let deck = Deck::new()
    .slide(&tree)
    .rel("data", "/diagramData", "ppt/diagrams/data1.xml", false)
    .part("ppt/diagrams/data1.xml", data)
    .open();
  assert_eq!(
    deck.markdown(),
    [
      "## Slide 1",
      "",
      "upper in the group",
      "",
      "lower in the group",
      "",
      "- Plan",
      r"- Build \*fast\*",
      "",
      "line one\\",
      "line two",
    ]
    .join("\n")
  );
}
