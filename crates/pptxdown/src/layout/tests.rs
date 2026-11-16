use contentdown::{PageLayout, PlacedImage, TextSource, TextWord};

use super::ESTIMATED;
use crate::testing::{Deck, emu, graphic_frame, para, placeholder, shape, text_box, xfrm};

// A slide here is 960 by 540 points. Text whose size nothing states is 18 points: a character
// is 9 points wide and a line 21.6 tall. A box keeps 7.2 points clear on its left and right and
// 3.6 at its top and bottom.

fn pages(tree: &str) -> Vec<PageLayout> {
  Deck::new().slide(tree).open().layout()
}

fn one_page(tree: &str) -> PageLayout {
  let mut pages = pages(tree);
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

/// A shape at a place given in points whose text body starts with `properties`.
fn boxed(at: (f64, f64, f64, f64), properties: &str, paragraphs: &str) -> String {
  shape(
    "",
    &xfrm(at.0, at.1, at.2, at.3),
    &format!("{properties}{paragraphs}"),
  )
}

#[test]
fn words_are_set_in_their_box_and_wrapped_at_its_right_edge() {
  let text = ["aaaa"; 7].join(" ");
  let page = one_page(&text_box(100.0, 50.0, 300.0, 200.0, &para(&text)));
  assert_eq!((page.page, page.width, page.height), (1, 960.0, 540.0));
  assert_eq!((page.rotate, page.source), (0, TextSource::Native));
  assert_eq!(page.warnings, [ESTIMATED]);
  // Text runs from 107.2 to 392.8. A word is 36 points wide and the next starts 45 further on:
  // six fit, and the seventh would end at 413.2.
  assert_eq!(lines(&page), [["aaaa"; 6].join(" ").as_str(), "aaaa"]);
  let (first, second) = (&page.lines[0], &page.lines[1]);
  close(first.x0, 107.2);
  close(first.x1, 107.2 + 5.0 * 45.0 + 36.0);
  // The first line starts 53.6 points down the slide; a page is measured up from its bottom.
  close(first.y1, 540.0 - 53.6);
  close(first.y0, 540.0 - 53.6 - 21.6);
  close(second.x0, 107.2);
  close(second.y1, first.y0);
  let third = &page.words[2];
  close(third.x0, 107.2 + 90.0);
  close(third.x1, 107.2 + 126.0);
  assert_eq!((third.font_size, third.line), (Some(18.0), 0));
  assert_eq!(page.words[6].line, 1);
}

#[test]
fn text_starts_at_the_top_the_middle_or_the_bottom_of_its_box() {
  let page = one_page(
    &[
      boxed((20.0, 300.0, 300.0, 100.0), "<a:bodyPr/>", &para("top")),
      boxed(
        (330.0, 300.0, 300.0, 100.0),
        r#"<a:bodyPr anchor="ctr"/>"#,
        &para("middle"),
      ),
      boxed(
        (640.0, 300.0, 300.0, 100.0),
        r#"<a:bodyPr anchor="b"/>"#,
        &para("bottom"),
      ),
    ]
    .concat(),
  );
  // The room for text is from 303.6 to 396.4, which is 92.8 points; a line takes 21.6.
  close(word(&page, "top").y1, 540.0 - 303.6);
  close(word(&page, "middle").y1, 540.0 - (303.6 + 35.6));
  close(word(&page, "bottom").y1, 540.0 - (303.6 + 71.2));
  close(word(&page, "bottom").y0, 540.0 - 396.4);
}

#[test]
fn alignment_moves_a_line_and_a_level_and_a_bullet_move_the_text_in() {
  let aligned = |properties: &str, text: &str| {
    format!("<a:p><a:pPr {properties}</a:pPr><a:r><a:t>{text}</a:t></a:r></a:p>")
  };
  let page = one_page(&text_box(
    100.0,
    50.0,
    300.0,
    200.0,
    &[
      aligned(r#"algn="ctr">"#, "aaaa"),
      aligned(r#"algn="r">"#, "bbbb"),
      aligned(r#"lvl="1">"#, "cccc"),
      aligned(r#"><a:buChar char="•"/>"#, "dddd"),
    ]
    .concat(),
  ));
  assert_eq!(lines(&page), ["aaaa", "bbbb", "cccc", "• dddd"]);
  // A word of 36 points in a line of 285.6 has 124.8 on either side of it.
  close(word(&page, "aaaa").x0, 107.2 + 124.8);
  close(word(&page, "bbbb").x1, 392.8);
  // Each level starts 18 points further in.
  close(word(&page, "cccc").x0, 107.2 + 18.0);
  // A bullet is a word of its own with a space after it.
  close(word(&page, "•").x0, 107.2);
  close(word(&page, "dddd").x0, 107.2 + 9.0 + 9.0);
  close(word(&page, "dddd").y1, 540.0 - 53.6 - 3.0 * 21.6);
}

#[test]
fn a_word_is_as_large_and_as_heavy_as_its_run_and_fitted_text_is_set_smaller() {
  let page = one_page(
    &[
      text_box(
        100.0,
        50.0,
        400.0,
        100.0,
        r#"<a:p><a:r><a:rPr sz="2400" b="1"><a:latin typeface="Arial"/></a:rPr><a:t>In</a:t></a:r><a:r><a:rPr sz="1200" i="1"/><a:t>voice 4711</a:t></a:r></a:p>"#,
      ),
      // PowerPoint shrank this text to half its size to fit its box, and says so.
      boxed(
        (100.0, 200.0, 400.0, 100.0),
        r#"<a:bodyPr><a:normAutofit fontScale="50000"/></a:bodyPr>"#,
        &para("fitted"),
      ),
      // A box that does not wrap keeps its text on one line however long it is.
      boxed(
        (100.0, 350.0, 50.0, 50.0),
        r#"<a:bodyPr wrap="none"/>"#,
        &para("one long line of text"),
      ),
    ]
    .concat(),
  );
  assert_eq!(
    lines(&page),
    ["Invoice 4711", "fitted", "one long line of text"]
  );
  // A word across two runs is one word, in the formatting it starts in.
  let invoice = word(&page, "Invoice");
  assert_eq!(
    (
      invoice.bold,
      invoice.italic,
      invoice.font_size,
      invoice.font_name.as_deref()
    ),
    (Some(true), Some(false), Some(24.0), Some("Arial"))
  );
  close(invoice.x1 - invoice.x0, 7.0 * 12.0);
  let number = word(&page, "4711");
  assert_eq!(
    (
      number.bold,
      number.italic,
      number.font_size,
      number.font_name.as_deref()
    ),
    (Some(false), Some(true), Some(12.0), None)
  );
  // Words of two sizes on one line stand on one baseline, and the line is as tall as the
  // larger needs.
  close(number.y0, invoice.y0);
  close(invoice.y1 - invoice.y0, 1.2 * 24.0);

  let fitted = word(&page, "fitted");
  assert_eq!(fitted.font_size, Some(9.0));
  close(fitted.x1 - fitted.x0, 6.0 * 4.5);
  close(fitted.y1 - fitted.y0, 1.2 * 9.0);
}

#[test]
fn a_table_sets_each_cell_in_its_own_rectangle() {
  let cell = |attributes: &str, text: &str| {
    format!(
      "<a:tc {attributes}><a:txBody><a:bodyPr/>{}</a:txBody></a:tc>",
      para(text)
    )
  };
  let stated = format!(
    r#"<a:tbl><a:tblGrid><a:gridCol w="{w}"/><a:gridCol w="{w}"/><a:gridCol w="{wide}"/></a:tblGrid>
       <a:tr h="{h}">{}{}{}</a:tr><a:tr h="{h}">{}{}{}</a:tr></a:tbl>"#,
    cell(r#"gridSpan="2""#, "Item"),
    cell(r#"hMerge="1""#, ""),
    cell("", "Price"),
    cell("", "Widget"),
    cell("", "blue"),
    cell("", "10.00"),
    w = emu(100.0),
    wide = emu(150.0),
    h = emu(30.0),
  );
  // A table that states no widths and no heights is divided evenly.
  let bare = format!(
    r#"<a:tbl><a:tblGrid><a:gridCol w="0"/><a:gridCol w="0"/></a:tblGrid><a:tr h="0">{}{}</a:tr><a:tr h="0">{}{}</a:tr></a:tbl>"#,
    cell("", "north"),
    cell("", "east"),
    cell("", "south"),
    cell("", "west"),
  );
  let page = one_page(
    &[
      graphic_frame((50.0, 60.0, 350.0, 90.0), &stated),
      graphic_frame((500.0, 300.0, 400.0, 100.0), &bare),
    ]
    .concat(),
  );
  assert_eq!(
    lines(&page),
    [
      "Item Price",
      "Widget blue 10.00",
      "north east",
      "south west"
    ]
  );
  // The columns start at 50, 150 and 250, and the second row 30 points under the first.
  close(word(&page, "Item").x0, 57.2);
  close(word(&page, "Price").x0, 257.2);
  close(word(&page, "Item").y1, 540.0 - 63.6);
  close(word(&page, "blue").x0, 157.2);
  close(word(&page, "10.00").x0, 257.2);
  close(word(&page, "10.00").y1, 540.0 - 93.6);

  close(word(&page, "north").x0, 507.2);
  close(word(&page, "east").x0, 707.2);
  close(word(&page, "south").y1, 540.0 - 353.6);
  close(word(&page, "west").x0, 707.2);
}

#[test]
fn text_taller_than_the_room_left_on_the_slide_is_set_closer_together() {
  let page = one_page(&text_box(
    100.0,
    500.0,
    300.0,
    30.0,
    &[para("one"), para("two"), para("three")].concat(),
  ));
  assert_eq!(lines(&page), ["one", "two", "three"]);
  // Three lines need 64.8 points and there are 36.4 from where they start to the slide's
  // bottom edge: each line gets its share of that.
  let share = 36.4 / 3.0;
  close(word(&page, "one").y1, 36.4);
  close(word(&page, "one").y0, 36.4 - share);
  close(word(&page, "two").y1, 36.4 - share);
  close(word(&page, "three").y0, 0.0);
  // The words are still said to be the size the file gives them.
  assert_eq!(word(&page, "three").font_size, Some(18.0));

  // Text that runs past its box but not past the slide is left as it is.
  let roomy = one_page(&text_box(
    100.0,
    100.0,
    300.0,
    30.0,
    &[para("one"), para("two"), para("three")].concat(),
  ));
  close(word(&roomy, "three").y1, 540.0 - 103.6 - 2.0 * 21.6);
  close(word(&roomy, "three").y0, 540.0 - 103.6 - 3.0 * 21.6);
}

#[test]
fn a_shape_with_no_box_is_set_across_the_slide_and_said_to_be() {
  let pages = Deck::new()
    .slide(
      &[
        placeholder(r#"type="body" idx="1""#, &para("nowhere")),
        placeholder(r#"type="title""#, &para("either")),
      ]
      .concat(),
    )
    .slide(&text_box(100.0, 50.0, 300.0, 200.0, &para("placed")))
    .open()
    .layout();
  // The slide less 36 points on every side.
  let nowhere = word(&pages[0], "nowhere");
  close(nowhere.x0, 43.2);
  close(nowhere.y1, 540.0 - 39.6);
  assert_eq!(
    pages[0].warnings,
    [
      ESTIMATED,
      "2 shape(s) state no box, and neither does the layout or the master; they are set across the slide"
    ]
  );
  // The count is of the slide, not of the deck.
  assert_eq!(
    (pages[1].page, pages[1].warnings.as_slice()),
    (2, [ESTIMATED.to_owned()].as_slice())
  );
}

#[test]
fn a_picture_is_placed_by_its_box_and_what_lies_off_the_slide_is_brought_onto_it() {
  let picture = |id: &str, at: (f64, f64, f64, f64)| {
    format!(
      r#"<p:pic><p:nvPicPr><p:cNvPr id="4" name="Picture"/><p:cNvPicPr/><p:nvPr/></p:nvPicPr><p:blipFill><a:blip r:embed="{id}"/></p:blipFill><p:spPr>{}</p:spPr></p:pic>"#,
      xfrm(at.0, at.1, at.2, at.3)
    )
  };
  let pages = Deck::new()
    .slide(
      &[
        picture("logo", (10.0, 20.0, 100.0, 50.0)),
        picture("photo", (900.0, 500.0, 100.0, 100.0)),
        text_box(940.0, 100.0, 200.0, 50.0, &para("off the edge")),
      ]
      .concat(),
    )
    .rel("logo", "/image", "ppt/media/image1.png", false)
    .rel("photo", "/image", "ppt/media/image2.jpeg", false)
    .part("ppt/media/image1.png", "")
    .part("ppt/media/image2.jpeg", "")
    .open()
    .layout();
  let placed = |name: &str, x0, y0, x1, y1| PlacedImage {
    xobject_name: name.to_owned(),
    x0,
    y0,
    x1,
    y1,
  };
  assert_eq!(
    pages[0].images,
    [
      placed("ppt/media/image1.png", 10.0, 470.0, 110.0, 520.0),
      // Only the part of a box that is on the slide is on the page.
      placed("ppt/media/image2.jpeg", 900.0, 0.0, 960.0, 40.0),
    ]
  );
  for word in &pages[0].words {
    assert!(
      0.0 <= word.x0 && word.x0 < word.x1 && word.x1 <= 960.0,
      "{word:?}"
    );
  }
  assert_eq!(lines(&pages[0]).concat().replace(' ', ""), "offtheedge");
}

#[test]
fn what_a_group_and_a_diagram_hold_is_set_where_it_stands() {
  let data = r#"<dgm:dataModel xmlns:dgm="http://schemas.openxmlformats.org/drawingml/2006/diagram" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><dgm:ptLst><dgm:pt modelId="1"><dgm:t><a:p><a:r><a:t>Plan</a:t></a:r></a:p></dgm:t></dgm:pt><dgm:pt modelId="2"><dgm:t><a:p><a:r><a:t>Build</a:t></a:r></a:p></dgm:t></dgm:pt></dgm:ptLst></dgm:dataModel>"#;
  // The group is shown at half the size its children were placed at.
  let group = format!(
    r#"<p:grpSp><p:grpSpPr><a:xfrm><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/><a:chOff x="0" y="0"/><a:chExt cx="{}" cy="{}"/></a:xfrm></p:grpSpPr>{}</p:grpSp>"#,
    emu(100.0),
    emu(100.0),
    emu(200.0),
    emu(100.0),
    emu(400.0),
    emu(200.0),
    text_box(40.0, 20.0, 300.0, 100.0, &para("child")),
  );
  let page = Deck::new()
    .slide(
      &[
        group,
        graphic_frame((100.0, 300.0, 400.0, 100.0), r#"<dgm:relIds r:dm="data"/>"#),
        graphic_frame((600.0, 300.0, 300.0, 200.0), r#"<c:chart r:id="chart"/>"#),
      ]
      .concat(),
    )
    .rel("data", "/diagramData", "ppt/diagrams/data1.xml", false)
    .rel("chart", "/chart", "ppt/charts/chart1.xml", false)
    .part("ppt/diagrams/data1.xml", data)
    .part(
      "ppt/charts/chart1.xml",
      r#"<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:chart><c:plotArea><c:barChart><c:ser><c:tx><c:v>Sales</c:v></c:tx></c:ser></c:barChart></c:plotArea></c:chart></c:chartSpace>"#,
    )
    .open()
    .layout()
    .remove(0);
  // A chart is data: none of it is set as words.
  assert_eq!(lines(&page), ["child", "Plan", "Build"]);
  close(word(&page, "child").x0, 120.0 + 7.2);
  close(word(&page, "child").y1, 540.0 - (110.0 + 3.6));
  // A diagram with no drawing has its nodes one to a line in its own box.
  close(word(&page, "Plan").x0, 107.2);
  close(word(&page, "Plan").y1, 540.0 - 303.6);
  close(word(&page, "Build").y1, 540.0 - 303.6 - 21.6);
}

#[test]
fn an_invoice_on_a_slide_is_read_from_the_page_it_is_set_as() {
  let deck = Deck::new()
    .slide(
      &[
        placeholder(r#"type="title""#, &para("Invoice 4711")),
        text_box(50.0, 150.0, 400.0, 40.0, &para("Bill to Acme Corp")),
        text_box(50.0, 400.0, 400.0, 40.0, &para("Total due: 25.50 USD")),
      ]
      .concat(),
    )
    .open();
  let invoice = deck.invoice(None);
  let fields = &invoice.fields;
  assert_eq!(
    fields.invoice_id.as_ref().map(|field| field.value.as_str()),
    Some("4711")
  );
  assert_eq!(
    fields.amount_due.as_ref().map(|field| field.value),
    Some(25.5)
  );
  assert_eq!(fields.currency_code.as_deref(), Some("USD"));
}
