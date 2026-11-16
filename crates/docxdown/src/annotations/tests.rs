use super::*;
use crate::media::Placement;
use crate::testing::{Setup, document, p};

/// A picture, through the relationship `id`.
fn picture(id: &str) -> String {
  format!(
    r#"<w:r><w:drawing><wp:inline><wp:extent cx="254000" cy="127000"/><wp:docPr id="1" name="p" descr="{id}"/><a:graphic><a:graphicData><pic:pic><pic:blipFill><a:blip r:embed="{id}"/></pic:blipFill></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>"#
  )
}

const PAGE_BREAK: &str = r#"<w:p><w:r><w:br w:type="page"/></w:r></w:p>"#;

fn sample() -> Document {
  // Page 1: a link made with a field, a note, two pictures. Page 2: a comment on two words,
  // a link to a bookmark, the first picture again.
  let body = format!(
    r#"<w:p><w:r><w:t xml:space="preserve">See </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText>HYPERLINK "https://example.com/terms"</w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>the terms</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:footnoteReference w:id="1"/></w:r></w:p>
       <w:p>{}{}</w:p>{PAGE_BREAK}
       <w:p><w:commentRangeStart w:id="1"/><w:r><w:t>Net thirty</w:t></w:r><w:commentRangeEnd w:id="1"/><w:r><w:commentReference w:id="1"/></w:r><w:r><w:t xml:space="preserve"> days, see </w:t></w:r><w:hyperlink w:anchor="schedule"><w:r><w:t>the schedule</w:t></w:r></w:hyperlink></w:p>
       <w:p>{}</w:p>
       <w:sectPr><w:headerReference w:type="default" r:id="h1"/></w:sectPr>"#,
    picture("logo"),
    picture("chart"),
    picture("logo"),
  );
  let header = format!(
    r#"<w:p>{}<w:hyperlink w:anchor="home"><w:r><w:t>Acme</w:t></w:r></w:hyperlink></w:p>"#,
    picture("logo")
  );
  let note = r#"<w:p><w:r><w:t xml:space="preserve">As published at </w:t></w:r><w:hyperlink w:anchor="source"><w:r><w:t>the source</w:t></w:r></w:hyperlink></w:p>"#;
  // The header is read with no relationships of its own here, so its picture is not found;
  // the body's are.
  document(
    &body,
    Setup {
      rels: &[
        ("logo", "word/media/logo.png", false),
        ("chart", "https://example.com/chart.png", true),
        ("h1", "word/header1.xml", false),
      ],
      headers: &[("word/header1.xml", &header)],
      footnotes: &[note],
      comments: &[("Ana", &p("Is this agreed?")), ("Ben", &p("Left over."))],
      ..Setup::default()
    },
  )
}

#[test]
fn links_and_comments_are_listed_page_by_page_with_headers_last() {
  let found = sample().annotations();
  let summary: Vec<(Option<u32>, PartKind, AnnotationKind, &str)> = found
    .iter()
    .map(|a| (a.page, a.part, a.subtype, a.text.as_str()))
    .collect();
  assert_eq!(
    summary,
    [
      (Some(1), PartKind::Body, AnnotationKind::Link, "the terms"),
      // A note's link is on the page that refers to the note.
      (
        Some(1),
        PartKind::Footnote,
        AnnotationKind::Link,
        "the source"
      ),
      (
        Some(2),
        PartKind::Body,
        AnnotationKind::Comment,
        "Net thirty"
      ),
      (
        Some(2),
        PartKind::Body,
        AnnotationKind::Link,
        "the schedule"
      ),
      // A header is on no one page, and a comment nothing refers to is on none.
      (None, PartKind::Header, AnnotationKind::Link, "Acme"),
      (None, PartKind::Body, AnnotationKind::Comment, ""),
    ]
  );
  assert_eq!(found[0].uri.as_deref(), Some("https://example.com/terms"));
  assert_eq!(
    (found[0].dest.as_deref(), found[0].content.as_deref()),
    (None, None)
  );
  assert_eq!(
    (found[3].uri.as_deref(), found[3].dest.as_deref()),
    (None, Some("schedule"))
  );
  let comment = &found[2];
  assert_eq!(comment.content.as_deref(), Some("Is this agreed?"));
  assert_eq!(comment.author.as_deref(), Some("Ana"));
  assert_eq!(comment.date.as_deref(), Some("2026-11-15T09:30:00Z"));
  assert_eq!(found[5].content.as_deref(), Some("Left over."));
}

#[test]
fn a_picture_is_one_image_and_a_placement_for_each_place_it_is_used() {
  let document = sample();
  let names: Vec<(&str, bool)> = document
    .images
    .iter()
    .map(|image| (image.name.as_str(), image.linked))
    .collect();
  assert_eq!(
    names,
    [
      ("word/media/logo.png", false),
      ("https://example.com/chart.png", true)
    ]
  );
  let placed: Vec<(Option<u32>, u32, u32)> = document
    .placements()
    .iter()
    .map(|p| (p.page, p.image_index, p.image))
    .collect();
  // The index starts over on each page.
  assert_eq!(placed, [(Some(1), 0, 0), (Some(1), 1, 1), (Some(2), 0, 0)]);
  let first: &Placement = &document.placements()[0];
  assert_eq!(
    (
      first.display_width,
      first.display_height,
      first.alt_text.as_deref(),
      first.part
    ),
    (Some(20.0), Some(10.0), Some("logo"), PartKind::Body)
  );

  let metadata = document.metadata();
  assert_eq!((metadata.image_count, metadata.annotation_count), (3, 6));
  assert_eq!((metadata.link_count, metadata.comment_count), (4, 2));
  assert_eq!((metadata.footnote_count, metadata.endnote_count), (1, 0));
  assert_eq!((metadata.header_count, metadata.pages_counted), (1, 2));
  // No application recorded a page count, so the counted one is the page count.
  assert_eq!((metadata.pages_recorded, metadata.page_count), (None, 2));
  assert_eq!(
    (metadata.lines_recorded, metadata.paragraphs_recorded),
    (None, None)
  );
}
