use super::*;
use crate::source::Piece;
use crate::testing::image_facts;
use crate::{Evidence, MediaType, specs, specs_source};

fn detail(facts: &ImageFacts) -> &SvgDetail {
  match &facts.detail {
    ImageDetail::Svg(detail) => detail,
    other => panic!("not an SVG's detail: {other:?}"),
  }
}

fn svg(root: &str) -> ImageFacts {
  let found = specs(
    format!("{root}<rect width=\"9999\" height=\"9999\" stroke-width=\"2\"/></svg>").as_bytes(),
  );
  assert_eq!(
    (found.media_type, found.evidence),
    (MediaType::Svg, Some(Evidence::Content))
  );
  image_facts(found)
}

#[test]
fn the_size_is_what_the_root_says_in_whatever_unit_it_says_it() {
  let facts = svg("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"640\" height=\"480px\">");
  assert_eq!((facts.width, facts.height), (640, 480));
  assert_eq!(
    (facts.color_model, facts.color_space, facts.frames),
    (ColorModel::Vector, ColorSpace::Srgb, 1)
  );
  assert_eq!(
    (
      detail(&facts).width.as_deref(),
      detail(&facts).height.as_deref()
    ),
    (Some("640"), Some("480px"))
  );
  // An inch is 96 pixels; a point is a seventy-second of one.
  let facts = svg("<svg width=\"2in\" height=\"72pt\">");
  assert_eq!((facts.width, facts.height), (192, 96));
  let facts = svg("<svg width=\"2.54cm\" height=\"25.4mm\">");
  assert_eq!((facts.width, facts.height), (96, 96));
}

#[test]
fn a_size_that_is_a_share_of_something_else_is_the_view_box_s() {
  let facts = svg("<svg width=\"100%\" height=\"100%\" viewBox=\"0 0 320 200\">");
  assert_eq!((facts.width, facts.height), (320, 200));
  assert_eq!(detail(&facts).view_box, Some([0.0, 0.0, 320.0, 200.0]));
  assert_eq!(detail(&facts).width.as_deref(), Some("100%"));
  let facts = svg("<svg viewBox=\"0,0,24,24\">");
  assert_eq!(
    (facts.width, facts.height, facts.aspect_ratio()),
    (24, 24, Some(1.0))
  );
}

#[test]
fn a_root_with_neither_is_an_svg_without_a_size_and_not_an_error() {
  let found = specs(b"<svg xmlns=\"http://www.w3.org/2000/svg\"><circle r=\"5\"/></svg>");
  assert_eq!(found.warnings.len(), 0);
  let facts = image_facts(found);
  assert_eq!(
    (facts.width, facts.height, facts.aspect_ratio()),
    (0, 0, None)
  );
  assert_eq!(detail(&facts).view_box, None);
}

#[test]
fn a_head_that_stops_inside_the_first_tag_is_words_for_now_and_asks_for_the_rest() {
  let root = format!(
    "<svg xmlns=\"http://www.w3.org/2000/svg\" data-note=\"{}\" width=\"640\" height=\"480\">",
    "x".repeat(400)
  );
  let file = format!("{root}<g/></svg>");
  let pieces = [Piece {
    offset: 0,
    bytes: &file.as_bytes()[..300],
  }];
  let head = specs_source(&Source::from_pieces(&pieces, None).unwrap());
  // The tag is not closed yet, so what it opens cannot be said.
  assert_eq!(head.media_type, MediaType::Txt);
  assert_eq!(head.needs.map(|range| range.offset), Some(300));
  assert_eq!(image_facts(specs(file.as_bytes())).width, 640);
}
