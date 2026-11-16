use super::*;

const W: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main""#;

fn numbering(inner: &str) -> Numbering {
  let text = format!("<w:numbering {W}>{inner}</w:numbering>");
  Numbering::parse(&Xml::parse(&text).unwrap())
}

fn level(at: u8, format: &str, text: &str, start: u32) -> String {
  format!(
    r#"<w:lvl w:ilvl="{at}"><w:start w:val="{start}"/><w:numFmt w:val="{format}"/><w:lvlText w:val="{text}"/></w:lvl>"#
  )
}

/// The markers of a run of list paragraphs, each given as `(instance, level)`.
fn markers(numbering: &Numbering, items: &[(u32, u8)]) -> Vec<String> {
  let mut counters = Counters::default();
  items
    .iter()
    .map(|&(id, level)| {
      counters
        .next(numbering, id, level)
        .map_or_else(|| "none".to_owned(), |item| item.marker)
    })
    .collect()
}

#[test]
fn counts_are_written_in_the_format_of_their_level() {
  let defined = numbering(&format!(
    r#"<w:abstractNum w:abstractNumId="0">{}{}{}{}{}</w:abstractNum><w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num>"#,
    level(0, "upperRoman", "%1.", 1),
    level(1, "upperLetter", "%2.", 1),
    level(2, "decimalZero", "%1-%2-%3", 9),
    level(3, "lowerRoman", "(%4)", 4),
    level(4, "lowerLetter", "%5)", 26),
  ));
  assert_eq!(
    markers(
      &defined,
      &[
        (1, 0),
        (1, 1),
        (1, 2),
        (1, 2),
        (1, 3),
        (1, 4),
        (1, 4),
        (1, 0)
      ]
    ),
    ["I.", "A.", "I-A-09", "I-A-10", "(iv)", "z)", "aa)", "II."]
  );
  assert_eq!(roman(1994), "mcmxciv");
  assert_eq!(letters(52), "zz");
}

#[test]
fn instances_of_one_definition_count_together_until_one_says_to_start_over() {
  let defined = numbering(&format!(
    r#"<w:abstractNum w:abstractNumId="5">{}</w:abstractNum>
       <w:num w:numId="1"><w:abstractNumId w:val="5"/></w:num>
       <w:num w:numId="2"><w:abstractNumId w:val="5"/></w:num>
       <w:num w:numId="3"><w:abstractNumId w:val="5"/><w:lvlOverride w:ilvl="0"><w:startOverride w:val="1"/></w:lvlOverride></w:num>"#,
    level(0, "decimal", "%1.", 1),
  ));
  // The second instance goes on from the first. The third starts over, once.
  assert_eq!(
    markers(&defined, &[(1, 0), (2, 0), (2, 0), (3, 0), (3, 0), (1, 0)]),
    ["1.", "2.", "3.", "1.", "2.", "3."]
  );
}

#[test]
fn an_instance_can_redefine_a_level() {
  let defined = numbering(&format!(
    r#"<w:abstractNum w:abstractNumId="0">{}</w:abstractNum>
       <w:num w:numId="1"><w:abstractNumId w:val="0"/><w:lvlOverride w:ilvl="0">{}</w:lvlOverride></w:num>"#,
    level(0, "decimal", "%1.", 1),
    level(0, "lowerLetter", "%1)", 3),
  ));
  assert_eq!(markers(&defined, &[(1, 0), (1, 0)]), ["c)", "d)"]);
}

#[test]
fn bullets_and_unnumbered_levels_are_not_ordered() {
  let defined = numbering(&format!(
    r#"<w:abstractNum w:abstractNumId="0">{}{}{}</w:abstractNum><w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num>"#,
    level(0, "bullet", "&#xF0B7;", 1),
    level(1, "bullet", "–", 1),
    level(2, "none", "", 1),
  ));
  let mut counters = Counters::default();
  let mut item = |level| counters.next(&defined, 1, level).unwrap();
  // A bullet from a symbol font's private range means nothing outside the font.
  let first = item(0);
  assert_eq!(
    (first.marker.as_str(), first.ordered, first.level),
    ("•", false, 0)
  );
  assert_eq!(item(1).marker, "–");
  let unnumbered = item(2);
  assert_eq!(
    (unnumbered.marker.as_str(), unnumbered.ordered),
    ("", false)
  );
  // Lists nest nine deep; a deeper level is read as the ninth, which this list does not define.
  assert_eq!(counters.next(&defined, 1, 200), None);
  assert_eq!(counters.next(&defined, 42, 0), None);
}
