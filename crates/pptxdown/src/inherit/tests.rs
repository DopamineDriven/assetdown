use crate::model::{Align, Anchor, Frame, Insets, Paragraph, Shape, TextShape};
use crate::testing::{Deck, para, placeholder, shape, text_box, xfrm};

fn text(shape: &Shape) -> &TextShape {
  match shape {
    Shape::Text(shape) => shape,
    other => panic!("{other:?} where a text shape was expected"),
  }
}

fn frame(x: f64, y: f64, width: f64, height: f64) -> Option<Frame> {
  Some(Frame {
    x,
    y,
    width,
    height,
    rotation: 0.0,
  })
}

/// The shape of a slide whose text is `wanted`.
fn find<'a>(shapes: &'a [Shape], wanted: &str) -> &'a TextShape {
  shapes
    .iter()
    .map(text)
    .find(|shape| shape.paragraphs.iter().any(|p| p.text() == wanted))
    .unwrap_or_else(|| panic!("no shape holds {wanted}"))
}

#[test]
fn a_placeholder_with_no_box_takes_the_layouts_and_then_the_masters() {
  let master = [
    shape(
      r#"<p:ph type="title"/>"#,
      &xfrm(50.0, 20.0, 860.0, 80.0),
      "",
    ),
    shape(
      r#"<p:ph type="body" idx="1"/>"#,
      &xfrm(50.0, 120.0, 860.0, 380.0),
      "",
    ),
    shape(
      r#"<p:ph type="ftr" idx="11"/>"#,
      &xfrm(300.0, 500.0, 360.0, 30.0),
      "",
    ),
  ]
  .concat();
  let layout = [
    // The layout's title states no box either: the master's stands.
    placeholder(r#"type="title""#, ""),
    shape(r#"<p:ph idx="1"/>"#, &xfrm(60.0, 130.0, 400.0, 360.0), ""),
    shape(
      r#"<p:ph type="pic" idx="13"/>"#,
      &xfrm(480.0, 130.0, 420.0, 360.0),
      "",
    ),
  ]
  .concat();
  let slide = [
    placeholder(r#"type="title""#, &para("title")),
    placeholder(r#"idx="1""#, &para("by number")),
    // No placeholder of the layout has this number; the one of its kind stands in.
    placeholder(r#"type="pic" idx="99""#, &para("by kind")),
    placeholder(r#"type="ftr" idx="11""#, &para("footer")),
    shape(
      r#"<p:ph idx="1"/>"#,
      &xfrm(10.0, 10.0, 100.0, 50.0),
      &para("own"),
    ),
    placeholder(r#"type="dt" idx="10""#, &para("nowhere")),
  ]
  .concat();
  let deck = Deck::new()
    .master(&master, "")
    .layout("Title and Content", &layout)
    .slide(&slide)
    .open();
  let shapes = &deck.slides[0].shapes;
  let frames = ["title", "by number", "by kind", "footer", "own", "nowhere"]
    .map(|wanted| find(shapes, wanted).info.frame);
  assert_eq!(
    frames,
    [
      frame(50.0, 20.0, 860.0, 80.0),
      frame(60.0, 130.0, 400.0, 360.0),
      frame(480.0, 130.0, 420.0, 360.0),
      frame(300.0, 500.0, 360.0, 30.0),
      frame(10.0, 10.0, 100.0, 50.0),
      None,
    ]
  );
  assert_eq!(deck.slides[0].layout.as_deref(), Some("Title and Content"));
  assert_eq!(deck.slides[0].title.as_deref(), Some("title"));
}

const TEXT_STYLES: &str = r#"<p:titleStyle><a:lvl1pPr algn="ctr"><a:buNone/><a:defRPr sz="4400" b="1"><a:latin typeface="Arial"/></a:defRPr></a:lvl1pPr></p:titleStyle>
  <p:bodyStyle>
    <a:lvl1pPr><a:buChar char="•"/><a:defRPr sz="2800"><a:latin typeface="+mn-lt"/></a:defRPr></a:lvl1pPr>
    <a:lvl2pPr><a:buChar char="–"/><a:defRPr sz="2400"/></a:lvl2pPr>
  </p:bodyStyle>
  <p:otherStyle><a:lvl1pPr><a:defRPr sz="1200"/></a:lvl1pPr></p:otherStyle>"#;

/// Size, bold, italic, font and bullet of a paragraph's first run.
type Look<'a> = (Option<f64>, bool, bool, Option<&'a str>, Option<&'a str>);

fn look(paragraph: &Paragraph) -> Look<'_> {
  let run = &paragraph.runs[0];
  (
    run.size,
    run.bold,
    run.italic,
    run.font.as_deref(),
    paragraph.bullet.as_deref(),
  )
}

#[test]
fn text_is_as_the_nearest_layer_that_says_so_says() {
  let layout = [
    placeholder(r#"type="ctrTitle""#, ""),
    shape(
      r#"<p:ph idx="1"/>"#,
      "",
      r#"<a:lstStyle><a:lvl1pPr><a:defRPr sz="2000" i="1"/></a:lvl1pPr></a:lstStyle>"#,
    ),
  ]
  .concat();
  let body = [
    para("level one"),
    r#"<a:p><a:pPr lvl="1"/><a:r><a:t>level two</a:t></a:r></a:p>"#.to_owned(),
    r#"<a:p><a:pPr algn="r"><a:buNone/></a:pPr><a:r><a:rPr sz="1000" b="1" i="0"/><a:t>its own</a:t></a:r></a:p>"#.to_owned(),
  ]
  .concat();
  let slide = [
    placeholder(r#"type="ctrTitle""#, &para("title")),
    placeholder(r#"idx="1""#, &body),
    text_box(0.0, 0.0, 10.0, 10.0, &para("a text box")),
    shape(
      "",
      "",
      &format!(
        r#"<a:lstStyle><a:lvl1pPr><a:defRPr sz="900"/></a:lvl1pPr></a:lstStyle>{}"#,
        para("list style")
      ),
    ),
    // Level nine is the deepest; nothing above the deck's defaults speaks for it.
    text_box(
      0.0,
      0.0,
      10.0,
      10.0,
      r#"<a:p><a:pPr lvl="8"/><a:r><a:t>deep</a:t></a:r></a:p>"#,
    ),
  ]
  .concat();
  let deck = Deck::new()
    .defaults(
      r#"<a:lvl1pPr><a:defRPr sz="1800"/></a:lvl1pPr><a:lvl9pPr><a:defRPr sz="1400"/></a:lvl9pPr>"#,
    )
    .master("", TEXT_STYLES)
    .layout("Title Slide", &layout)
    .slide(&slide)
    .open();
  let shapes = &deck.slides[0].shapes;

  let title = &find(shapes, "title").paragraphs[0];
  // The master's title style: large, bold, centered, with no bullet.
  assert_eq!(look(title), (Some(44.0), true, false, Some("Arial"), None));
  assert_eq!(title.align, Some(Align::Center));

  let body = &find(shapes, "level one").paragraphs;
  // The layout sets the first level's size and slant; the bullet is still the master's. A
  // font named through the theme names no font.
  assert_eq!(look(&body[0]), (Some(20.0), false, true, None, Some("•")));
  assert_eq!(look(&body[1]), (Some(24.0), false, false, None, Some("–")));
  assert_eq!((body[0].level, body[1].level), (0, 1));
  // What the paragraph and the run say themselves wins.
  assert_eq!(look(&body[2]), (Some(10.0), true, false, None, None));
  assert_eq!(body[2].align, Some(Align::Right));

  // A text box is not body text: it takes the master's other style.
  let plain = &find(shapes, "a text box").paragraphs[0];
  assert_eq!(look(plain), (Some(12.0), false, false, None, None));
  assert_eq!(look(&find(shapes, "list style").paragraphs[0]).0, Some(9.0));
  assert_eq!(look(&find(shapes, "deep").paragraphs[0]).0, Some(14.0));
}

#[test]
fn how_text_sits_in_its_box_is_inherited_too() {
  let layout = shape(
    r#"<p:ph idx="1"/>"#,
    "",
    r#"<a:bodyPr lIns="0" tIns="127000" anchor="ctr"/>"#,
  );
  let slide = [
    placeholder(r#"idx="1""#, &para("inherits")),
    shape(
      r#"<p:ph idx="1"/>"#,
      "",
      &format!(
        r#"<a:bodyPr anchor="b" wrap="none"><a:normAutofit fontScale="62500" lnSpcReduction="20000"/></a:bodyPr>{}"#,
        para("overrides")
      ),
    ),
    text_box(0.0, 0.0, 10.0, 10.0, &para("default")),
  ]
  .concat();
  let deck = Deck::new()
    .master("", "")
    .layout("L", &layout)
    .slide(&slide)
    .open();
  let shapes = &deck.slides[0].shapes;
  let inherits = find(shapes, "inherits");
  assert_eq!(
    inherits.insets,
    Insets {
      left: 0.0,
      top: 10.0,
      right: 7.2,
      bottom: 3.6
    }
  );
  assert_eq!(
    (inherits.anchor, inherits.wrap, inherits.font_scale),
    (Anchor::Middle, true, 1.0)
  );
  let overrides = find(shapes, "overrides");
  assert_eq!(overrides.insets.left, 0.0);
  assert_eq!(
    (overrides.anchor, overrides.wrap, overrides.font_scale),
    (Anchor::Bottom, false, 0.625)
  );
  let default = find(shapes, "default");
  assert_eq!(
    default.insets,
    Insets {
      left: 7.2,
      top: 3.6,
      right: 7.2,
      bottom: 3.6
    }
  );
  assert_eq!(default.anchor, Anchor::Top);
}

#[test]
fn a_slide_with_no_layout_or_a_layout_that_cannot_be_read_still_reads() {
  let slide = [
    placeholder(r#"type="title""#, &para("alone")),
    text_box(1.0, 2.0, 3.0, 4.0, &para("box")),
  ]
  .concat();
  let bare = Deck::new().slide(&slide).open();
  assert_eq!(bare.slides[0].layout, None);
  assert_eq!(find(&bare.slides[0].shapes, "alone").info.frame, None);
  assert_eq!(
    find(&bare.slides[0].shapes, "box").info.frame,
    frame(1.0, 2.0, 3.0, 4.0)
  );
  assert_eq!(bare.warnings, Vec::<String>::new());

  // The slide names a layout whose part is not XML.
  let broken = Deck::new()
    .part("ppt/slideLayouts/slideLayout1.xml", "<p:sldLayout")
    .slide(&slide)
    .open();
  assert_eq!(broken.slides[0].title.as_deref(), Some("alone"));
  assert_eq!(broken.slides[0].layout, None);
}
