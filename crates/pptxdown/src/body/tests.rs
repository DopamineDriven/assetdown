use crate::model::{LinkTarget, Paragraph, Shape};
use crate::testing::{Deck, para, text_box};

/// The paragraphs of a text box holding `body` on a slide of a deck `deck` adds to.
fn paragraphs(deck: Deck, body: &str) -> Vec<Paragraph> {
  let deck = deck.slide(&text_box(0.0, 0.0, 100.0, 100.0, body)).open();
  match deck.slides.into_iter().next().unwrap().shapes.remove(0) {
    Shape::Text(shape) => shape.paragraphs,
    other => panic!("{other:?} where a text shape was expected"),
  }
}

fn run(properties: &str, text: &str) -> String {
  format!("<a:r><a:rPr {properties}/><a:t>{text}</a:t></a:r>")
}

#[test]
fn runs_that_are_formatted_alike_are_one_and_breaks_fields_and_equations_are_text() {
  let body = format!(
    r#"<a:p>{}{}<a:br><a:rPr b="1"/></a:br>{}<a:fld id="{{1}}" type="slidenum"><a:rPr/><a:t>7</a:t></a:fld>{}
         <mc:AlternateContent><mc:Choice Requires="a14"><m:oMath><m:r><m:t>x=</m:t></m:r><m:r><m:t>1</m:t></m:r></m:oMath></mc:Choice><mc:Fallback>{}</mc:Fallback></mc:AlternateContent>
         <a:endParaRPr sz="900"/></a:p>"#,
    run(r#"b="1""#, "Total "),
    run(r#"b="1" lang="en-US""#, "due"),
    run(
      r#"i="1" u="sng" strike="sngStrike" sz="1050""#,
      "now\ttabbed"
    ),
    run("", " of 9 "),
    run("", "picture of the equation"),
  );
  let read = paragraphs(Deck::new(), &body);
  let runs = &read[0].runs;
  let texts: Vec<&str> = runs.iter().map(|run| run.text.as_str()).collect();
  // The two bold runs and the bold break are one run; the equation is read once.
  assert_eq!(texts, ["Total due\n", "now\ttabbed", "7", " of 9 x=1"]);
  assert_eq!(read[0].text(), "Total due\nnow\ttabbed7 of 9 x=1");
  assert!(runs[0].bold && !runs[0].italic);
  let slanted = &runs[1];
  assert!(slanted.italic && slanted.underline && slanted.strike && !slanted.bold);
  assert_eq!(slanted.size, Some(10.5));
  assert_eq!(runs[2].field.as_deref(), Some("slidenum"));
  assert_eq!(runs[3].field, None);
}

#[test]
fn a_link_leads_to_an_address_to_another_slide_or_makes_a_jump() {
  let link = |attributes: &str, text: &str| {
    format!("<a:r><a:rPr><a:hlinkClick {attributes}/></a:rPr><a:t>{text}</a:t></a:r>")
  };
  let body = format!(
    "<a:p>{}{}{}{}{}{}</a:p>",
    link(r#"r:id="site""#, "the "),
    link(r#"r:id="site""#, "site"),
    link(
      r#"r:id="other" action="ppaction://hlinksldjump""#,
      "slide two"
    ),
    link(
      r#"r:id="" action="ppaction://hlinkshowjump?jump=nextslide""#,
      "next"
    ),
    link(r#"r:id="missing""#, "nowhere"),
    link(r#"r:id="file""#, "a part that is no slide"),
  );
  let deck = Deck::new()
    .slide(&text_box(0.0, 0.0, 100.0, 100.0, &body))
    .rel("site", "/hyperlink", "https://example.com/a", true)
    .rel("other", "/slide", "ppt/slides/slide2.xml", false)
    .rel("file", "/image", "ppt/media/image1.png", false)
    .slide(&text_box(0.0, 0.0, 1.0, 1.0, &para("two")))
    .open();
  let Shape::Text(shape) = &deck.slides[0].shapes[0] else {
    panic!("a text shape was expected");
  };
  let links: Vec<(&str, Option<&LinkTarget>)> = shape.paragraphs[0]
    .runs
    .iter()
    .map(|run| (run.text.as_str(), run.link.as_ref()))
    .collect();
  let site = LinkTarget::External("https://example.com/a".to_owned());
  assert_eq!(
    links,
    [
      // Two runs to one target are one run.
      ("the site", Some(&site)),
      ("slide two", Some(&LinkTarget::Slide(2))),
      ("next", Some(&LinkTarget::Action("nextslide".to_owned()))),
      // A link that leads nowhere the deck knows is plain text.
      ("nowherea part that is no slide", None),
    ]
  );
}

fn bulleted(properties: &str, text: &str) -> String {
  format!("<a:p><a:pPr {properties}</a:pPr><a:r><a:t>{text}</a:t></a:r></a:p>")
}

#[test]
fn a_bullet_is_the_character_it_shows_and_a_symbol_fonts_is_a_dot() {
  let body = [
    bulleted(r#"><a:buChar char="–"/>"#, "dash"),
    bulleted(
      r#"><a:buFont typeface="Wingdings"/><a:buChar char="§"/>"#,
      "symbol font",
    ),
    bulleted(
      r#"><a:buFont typeface="Arial"/><a:buChar char="&#xF0B7;"/>"#,
      "private range",
    ),
    bulleted(r#"><a:buNone/>"#, "none"),
    bulleted(">", "silent"),
    // A paragraph with nothing in it shows no bullet.
    r#"<a:p><a:pPr><a:buChar char="•"/></a:pPr></a:p>"#.to_owned(),
    r#"<a:p><a:pPr><a:buChar char="•"/></a:pPr><a:r><a:t>   </a:t></a:r></a:p>"#.to_owned(),
  ]
  .concat();
  let bullets: Vec<(Option<String>, bool)> = paragraphs(Deck::new(), &body)
    .into_iter()
    .map(|paragraph| (paragraph.bullet, paragraph.ordered))
    .collect();
  let dot = |text: &str| (Some(text.to_owned()), false);
  assert_eq!(
    bullets,
    [
      dot("–"),
      dot("•"),
      dot("•"),
      (None, false),
      (None, false),
      (None, false),
      (None, false)
    ]
  );
}

#[test]
fn automatic_numbers_count_by_level_and_start_over_where_a_list_is_broken() {
  let numbered = |level: u8, scheme: &str, text: &str| {
    bulleted(
      &format!(r#"lvl="{level}"><a:buAutoNum type="{scheme}"/>"#),
      text,
    )
  };
  let body = [
    numbered(0, "arabicPeriod", "a"),
    numbered(0, "arabicPeriod", "b"),
    numbered(1, "alphaLcParenR", "c"),
    numbered(1, "alphaLcParenR", "d"),
    numbered(0, "arabicPeriod", "e"),
    // A deeper level starts over under the next item.
    numbered(1, "alphaLcParenR", "f"),
    // A paragraph with no number ends the list on its level.
    bulleted(r#"><a:buNone/>"#, "g"),
    numbered(0, "arabicPeriod", "h"),
    // Another scheme is another list; each of its paragraphs says where it starts.
    bulleted(
      r#"><a:buAutoNum type="romanUcParenBoth" startAt="4"/>"#,
      "i",
    ),
    bulleted(
      r#"><a:buAutoNum type="romanUcParenBoth" startAt="4"/>"#,
      "j",
    ),
    numbered(0, "alphaUcPeriod", "k"),
    numbered(0, "arabicPlain", "l"),
    numbered(0, "romanLcParenR", "m"),
  ]
  .concat();
  let read = paragraphs(Deck::new(), &body);
  let markers: Vec<&str> = read
    .iter()
    .map(|paragraph| paragraph.bullet.as_deref().unwrap_or(""))
    .collect();
  assert_eq!(
    markers,
    [
      "1.", "2.", "a)", "b)", "3.", "a)", "", "1.", "(IV)", "(V)", "A.", "1", "i)"
    ]
  );
  assert!(read[0].ordered && !read[6].ordered);
  assert_eq!(read[2].line(), "a) c");
  assert_eq!(read[6].line(), "g");
}

#[test]
fn a_numbered_paragraph_with_nothing_in_it_shows_no_number_and_takes_none() {
  let numbered = |start: &str, text: &str| {
    format!(
      r#"<a:p><a:pPr><a:buAutoNum type="arabicPeriod"{start}/></a:pPr>{}</a:p>"#,
      if text.is_empty() {
        String::new()
      } else {
        format!("<a:r><a:t>{text}</a:t></a:r>")
      }
    )
  };
  let body = [
    numbered("", "a"),
    numbered("", ""),
    numbered("", "b"),
    // A paragraph that states another start begins a new list there, and the paragraphs
    // that carry it on state the same start.
    numbered(r#" startAt="7""#, "c"),
    numbered(r#" startAt="7""#, ""),
    numbered(r#" startAt="7""#, "d"),
    // Text with no bullet ends the list, though the paragraph after it states the same start.
    para("plain"),
    numbered(r#" startAt="7""#, "e"),
  ]
  .concat();
  let read = paragraphs(Deck::new(), &body);
  let markers: Vec<(&str, bool)> = read
    .iter()
    .map(|paragraph| (paragraph.bullet.as_deref().unwrap_or(""), paragraph.ordered))
    .collect();
  assert_eq!(
    markers,
    [
      ("1.", true),
      ("", false),
      ("2.", true),
      ("7.", true),
      ("", false),
      ("8.", true),
      ("", false),
      ("7.", true)
    ]
  );
}
