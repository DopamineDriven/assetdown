use super::*;

const W: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main""#;

fn styles(inner: &str) -> Styles {
  let text = format!("<w:styles {W}>{inner}</w:styles>");
  Styles::parse(&Xml::parse(&text).unwrap())
}

#[test]
fn a_style_takes_what_it_does_not_state_from_the_style_it_is_based_on() {
  let found = styles(
    r#"<w:style w:type="paragraph" w:styleId="Base"><w:name w:val="Base"/><w:pPr><w:jc w:val="both"/><w:pageBreakBefore/></w:pPr><w:rPr><w:rFonts w:ascii="Garamond"/><w:sz w:val="24"/><w:i/></w:rPr></w:style>
       <w:style w:type="paragraph" w:styleId="Quote"><w:name w:val="Quote"/><w:basedOn w:val="Base"/><w:pPr><w:pageBreakBefore w:val="0"/><w:numPr><w:ilvl w:val="1"/><w:numId w:val="7"/></w:numPr></w:pPr><w:rPr><w:sz w:val="20"/></w:rPr></w:style>"#,
  );
  let quote = found.paragraph(Some("Quote"));
  assert_eq!(quote.name.as_deref(), Some("Quote"));
  assert_eq!(quote.align, Some(Align::Justify));
  assert_eq!(quote.list, Some((7, 1)));
  // Its own answer wins over the one under it.
  assert!(!quote.page_break_before);
  assert_eq!(quote.run.size, Some(10.0));
  assert_eq!(quote.run.font.as_deref(), Some("Garamond"));
  assert_eq!(quote.run.italic, Some(true));
  assert!(found.paragraph(Some("Base")).page_break_before);
}

#[test]
fn a_heading_is_known_by_its_outline_level_or_by_its_name() {
  let found = styles(
    r#"<w:style w:type="paragraph" w:styleId="Titre2"><w:name w:val="heading 2"/></w:style>
       <w:style w:type="paragraph" w:styleId="Chapter"><w:name w:val="Chapter"/><w:pPr><w:outlineLvl w:val="0"/></w:pPr></w:style>
       <w:style w:type="paragraph" w:styleId="Sub"><w:name w:val="Subchapter"/><w:basedOn w:val="Chapter"/></w:style>
       <w:style w:type="paragraph" w:styleId="Body"><w:name w:val="Body Text"/><w:pPr><w:outlineLvl w:val="9"/></w:pPr></w:style>
       <w:style w:type="paragraph" w:styleId="H"><w:name w:val="Heading 12"/></w:style>"#,
  );
  let heading = |id: &str| found.paragraph(Some(id)).heading;
  assert_eq!(heading("Titre2"), Some(2));
  assert_eq!(heading("Chapter"), Some(1));
  assert_eq!(heading("Sub"), Some(1));
  // Outline level 9 is body text, and there is no heading 12.
  assert_eq!(heading("Body"), None);
  assert_eq!(heading("H"), None);
}

#[test]
fn a_paragraph_with_no_style_has_the_default_one_and_an_unknown_style_has_none() {
  let found = styles(
    r#"<w:docDefaults><w:rPrDefault><w:rPr><w:sz w:val="22"/><w:lang w:val="de-DE"/></w:rPr></w:rPrDefault></w:docDefaults>
       <w:style w:type="paragraph" w:default="1" w:styleId="Standard"><w:name w:val="Normal"/><w:rPr><w:b/></w:rPr></w:style>
       <w:style w:type="character" w:default="1" w:styleId="Absatz"><w:name w:val="Default Paragraph Font"/></w:style>"#,
  );
  assert_eq!(found.paragraph(None).name.as_deref(), Some("Normal"));
  assert_eq!(found.paragraph(None).run.bold, Some(true));
  assert_eq!(found.paragraph(Some("Missing")).name, None);
  assert_eq!(found.defaults.size, Some(11.0));
  assert_eq!(found.language.as_deref(), Some("de-DE"));
}

#[test]
fn styles_based_on_each_other_in_a_circle_still_resolve() {
  let found = styles(
    r#"<w:style w:type="paragraph" w:styleId="A"><w:name w:val="A"/><w:basedOn w:val="B"/><w:rPr><w:b/></w:rPr></w:style>
       <w:style w:type="paragraph" w:styleId="B"><w:name w:val="B"/><w:basedOn w:val="A"/><w:rPr><w:i/></w:rPr></w:style>"#,
  );
  let a = found.paragraph(Some("A"));
  assert_eq!((a.run.bold, a.run.italic), (Some(true), Some(true)));
}

#[test]
fn run_properties_read_sizes_in_half_points_and_tell_off_from_absent() {
  let text = format!(
    r#"<w:rPr {W}><w:b w:val="0"/><w:i w:val="true"/><w:u w:val="none"/><w:dstrike/><w:vertAlign w:val="superscript"/><w:sz w:val="19"/><w:rFonts w:hAnsi="Arial"/><w:rStyle w:val="Strong"/></w:rPr>"#
  );
  let doc = Xml::parse(&text).unwrap();
  let read = RunProps::read(Some(doc.root_element()));
  assert_eq!(read.bold, Some(false));
  assert_eq!(read.italic, Some(true));
  assert_eq!(read.underline, Some(false));
  assert_eq!(read.strike, Some(true));
  assert_eq!(read.vertical, Some(VerticalAlign::Superscript));
  assert_eq!(read.size, Some(9.5));
  assert_eq!(read.font.as_deref(), Some("Arial"));
  assert_eq!(read.style.as_deref(), Some("Strong"));
  assert_eq!(RunProps::read(None), RunProps::default());
}
