use super::*;
use crate::write::compound;

/// A property's value, as a property stream keeps it.
enum Value<'a> {
  /// The code page the stream's text is in.
  CodePage(u16),
  /// Text in that code page.
  Text(&'a [u8]),
  /// Text two bytes to a character.
  Wide(&'a str),
  /// A moment, in tenths of a microsecond since 1601.
  Time(u64),
  Int(i32),
}

/// A property stream of one set of properties, each a number and a value.
fn stream(properties: &[(u32, Value<'_>)]) -> Vec<u8> {
  let mut values: Vec<Vec<u8>> = Vec::new();
  for (_, value) in properties {
    let mut kept = Vec::new();
    match value {
      Value::CodePage(page) => {
        kept.extend(2u32.to_le_bytes());
        kept.extend(page.to_le_bytes());
      }
      Value::Text(text) => {
        kept.extend(0x1Eu32.to_le_bytes());
        kept.extend(u32::try_from(text.len() + 1).unwrap().to_le_bytes());
        kept.extend(*text);
        kept.push(0);
      }
      Value::Wide(text) => {
        let units: Vec<u16> = text.encode_utf16().chain([0]).collect();
        kept.extend(0x1Fu32.to_le_bytes());
        kept.extend(u32::try_from(units.len()).unwrap().to_le_bytes());
        kept.extend(units.iter().flat_map(|unit| unit.to_le_bytes()));
      }
      Value::Time(ticks) => {
        kept.extend(0x40u32.to_le_bytes());
        kept.extend(ticks.to_le_bytes());
      }
      Value::Int(value) => {
        kept.extend(3u32.to_le_bytes());
        kept.extend(value.to_le_bytes());
      }
    }
    kept.resize(kept.len().next_multiple_of(4), 0);
    values.push(kept);
  }
  // The set: its length, how many properties, where each is, and then the values.
  let mut at = 8 + properties.len() * 8;
  let mut listed = Vec::new();
  for ((id, _), value) in properties.iter().zip(&values) {
    listed.extend(id.to_le_bytes());
    listed.extend(u32::try_from(at).unwrap().to_le_bytes());
    at += value.len();
  }
  let mut set = u32::try_from(at).unwrap().to_le_bytes().to_vec();
  set.extend(u32::try_from(properties.len()).unwrap().to_le_bytes());
  set.extend(listed);
  set.extend(values.concat());
  // The stream: a mark of its byte order, a version, who wrote it, a count of sets, and for
  // the one set what it is and where it starts.
  let mut out = vec![0xFE, 0xFF, 0, 0];
  out.extend([0u8; 20]);
  out.extend(1u32.to_le_bytes());
  out.extend([0u8; 16]);
  out.extend(48u32.to_le_bytes());
  out.extend(set);
  out
}

fn summary_of(streams: &[(&str, &[u8])]) -> Summary {
  let file = compound(streams).unwrap();
  CompoundFile::open(&file).unwrap().summary()
}

/// 2026-11-15 09:30:00, in tenths of a microsecond since 1601.
const A_MORNING: u64 = 134_392_086_000_000_000;

#[test]
fn what_a_document_says_about_itself_is_read_from_its_two_property_streams() {
  let first = stream(&[
    (1, Value::CodePage(1252)),
    (2, Value::Text(b"Order form")),
    (3, Value::Text(b"Parts")),
    (4, Value::Text(b"A. Clerk")),
    (5, Value::Text(b"orders; parts")),
    (6, Value::Text(b"For the spring")),
    (7, Value::Text(b"Normal.dot")),
    (8, Value::Text(b"B. Checker")),
    (9, Value::Text(b"7")),
    (12, Value::Time(A_MORNING)),
    (13, Value::Time(A_MORNING + 36_000_000_000)),
    (18, Value::Text(b"Microsoft Excel")),
  ]);
  let second = stream(&[
    (1, Value::CodePage(1252)),
    (2, Value::Text(b"Forms")),
    (14, Value::Text(b"C. Lead")),
    (15, Value::Text(b"Acme Tooling")),
    (23, Value::Int(0x000C_0000)),
    (28, Value::Text(b"en-US")),
  ]);
  let summary = summary_of(&[
    ("Workbook", b"records"),
    ("\u{5}SummaryInformation", &first),
    ("\u{5}DocumentSummaryInformation", &second),
  ]);
  let text = |value: &str| Some(value.to_string());
  assert_eq!(
    summary,
    Summary {
      title: text("Order form"),
      subject: text("Parts"),
      author: text("A. Clerk"),
      keywords: text("orders; parts"),
      comments: text("For the spring"),
      template: text("Normal.dot"),
      last_author: text("B. Checker"),
      revision: text("7"),
      created: text("2026-11-15T09:30:00Z"),
      modified: text("2026-11-15T10:30:00Z"),
      application: text("Microsoft Excel"),
      category: text("Forms"),
      manager: text("C. Lead"),
      company: text("Acme Tooling"),
      // The version is two halves of one number: twelve, and nought.
      app_version: text("12.0000"),
      language: text("en-US"),
    }
  );
}

#[test]
fn text_is_read_in_the_code_page_its_stream_states() {
  let title = |properties: &[(u32, Value<'_>)]| {
    summary_of(&[("\u{5}SummaryInformation", &stream(properties))]).title
  };
  let western = title(&[
    (1, Value::CodePage(1252)),
    (2, Value::Text(b"Caf\xE9 \x93one\x94")),
  ]);
  assert_eq!(western.as_deref(), Some("Caf\u{e9} \u{201c}one\u{201d}"));
  let cyrillic = title(&[
    (1, Value::CodePage(1251)),
    (2, Value::Text(b"\xD1\xF7\xE5\xF2")),
  ]);
  assert_eq!(cyrillic.as_deref(), Some("\u{421}\u{447}\u{435}\u{442}"));
  let japanese = title(&[
    (1, Value::CodePage(932)),
    (2, Value::Text(b"\x93\xFA\x96\x7B")),
  ]);
  assert_eq!(japanese.as_deref(), Some("\u{65e5}\u{672c}"));
  let utf8 = title(&[
    (1, Value::CodePage(65001)),
    (2, Value::Text("Zo\u{eb}".as_bytes())),
  ]);
  assert_eq!(utf8.as_deref(), Some("Zo\u{eb}"));
  // Text two bytes to a character needs no code page, and one that follows the text it
  // describes is still the stream's.
  let wide = title(&[
    (2, Value::Wide("\u{65e5}\u{672c}")),
    (1, Value::CodePage(1252)),
  ]);
  assert_eq!(wide.as_deref(), Some("\u{65e5}\u{672c}"));
  // A stream whose page is the two-byte one keeps its plain text that way too.
  let units: Vec<u8> = "\u{6708} A"
    .encode_utf16()
    .flat_map(u16::to_le_bytes)
    .chain([0])
    .collect();
  let two_byte = title(&[(1, Value::CodePage(1200)), (2, Value::Text(&units))]);
  assert_eq!(two_byte.as_deref(), Some("\u{6708} A"));
}

#[test]
fn text_in_a_code_page_that_is_not_known_is_given_only_when_it_is_plain() {
  let title = |text: &[u8]| {
    let properties = [(1, Value::CodePage(37)), (2, Value::Text(text))];
    summary_of(&[("\u{5}SummaryInformation", &stream(&properties))]).title
  };
  assert_eq!(title(b"Plain words").as_deref(), Some("Plain words"));
  assert_eq!(title(b"Caf\xE9"), None);
}

#[test]
fn a_moment_of_nought_and_text_with_nothing_in_it_are_not_stated() {
  let summary = summary_of(&[(
    "\u{5}SummaryInformation",
    &stream(&[
      (1, Value::CodePage(1252)),
      (2, Value::Text(b"  ")),
      (4, Value::Text(b"")),
      (12, Value::Time(0)),
      (13, Value::Time(A_MORNING)),
    ]),
  )]);
  assert_eq!(
    (summary.title, summary.author, summary.created),
    (None, None, None)
  );
  assert_eq!(summary.modified.as_deref(), Some("2026-11-15T09:30:00Z"));
}

#[test]
fn moments_are_given_to_the_second_whatever_the_year() {
  let created = |ticks: u64| {
    summary_of(&[(
      "\u{5}SummaryInformation",
      &stream(&[(12, Value::Time(ticks))]),
    )])
    .created
  };
  // The last second of a leap day, and the first of the day after.
  let leap_day_end = 127_225_727_990_000_000u64;
  assert_eq!(
    created(leap_day_end).as_deref(),
    Some("2004-02-29T23:59:59Z")
  );
  assert_eq!(
    created(leap_day_end + 10_000_000).as_deref(),
    Some("2004-03-01T00:00:00Z")
  );
  // The start of 1970, which other clocks count from.
  assert_eq!(
    created(116_444_736_000_000_000).as_deref(),
    Some("1970-01-01T00:00:00Z")
  );
}

#[test]
fn a_file_with_no_property_streams_or_a_broken_one_states_nothing() {
  assert_eq!(summary_of(&[("Workbook", b"records")]), Summary::default());
  let whole = stream(&[(1, Value::CodePage(1252)), (2, Value::Text(b"Order form"))]);
  // Cut in the middle of the title, in the middle of the list of properties, and to nothing.
  for cut in [whole.len() - 6, 60, 10, 0] {
    let summary = summary_of(&[("\u{5}SummaryInformation", &whole[..cut])]);
    assert_eq!(summary, Summary::default(), "cut at {cut}");
  }
  // A property that says it is kept past the end of the stream is passed over.
  let mut astray = whole.clone();
  astray[48 + 8 + 8 + 4..48 + 8 + 8 + 8].copy_from_slice(&0x00FF_FFFFu32.to_le_bytes());
  assert_eq!(
    summary_of(&[("\u{5}SummaryInformation", &astray)]),
    Summary::default()
  );
}
