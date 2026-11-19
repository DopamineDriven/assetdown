use super::*;
use crate::Pdf;
use lopdf::{
  Dictionary, EncryptionState, EncryptionVersion, Permissions, Stream, StringFormat, dictionary,
};

#[test]
fn parse_page_box_valid_integers() {
  let obj = Object::Array(vec![
    Object::Integer(0),
    Object::Integer(0),
    Object::Integer(612),
    Object::Integer(792),
  ]);
  let result = parse_page_box(&obj);
  assert_eq!(result, Some([0.0, 0.0, 612.0, 792.0]));
}

#[test]
fn parse_page_box_valid_reals() {
  let obj = Object::Array(vec![
    Object::Real(0.0),
    Object::Real(0.0),
    Object::Real(595.0),
    Object::Real(842.0),
  ]);
  let result = parse_page_box(&obj);
  assert_eq!(result, Some([0.0, 0.0, 595.0, 842.0]));
}

#[test]
fn parse_page_box_mixed_types() {
  let obj = Object::Array(vec![
    Object::Integer(0),
    Object::Real(0.5),
    Object::Integer(612),
    Object::Real(792.0),
  ]);
  let result = parse_page_box(&obj);
  assert_eq!(result, Some([0.0, 0.5, 612.0, 792.0]));
}

#[test]
fn parse_page_box_too_short() {
  let obj = Object::Array(vec![Object::Integer(0), Object::Integer(0)]);
  assert_eq!(parse_page_box(&obj), None);
}

#[test]
fn parse_page_box_non_numeric() {
  let obj = Object::Array(vec![
    Object::Integer(0),
    Object::Integer(0),
    Object::Name(b"bad".to_vec()),
    Object::Integer(792),
  ]);
  assert_eq!(parse_page_box(&obj), None);
}

#[test]
fn parse_page_box_not_array() {
  let obj = Object::Integer(42);
  assert_eq!(parse_page_box(&obj), None);
}

#[test]
fn parse_page_box_extra_elements_ignored() {
  let obj = Object::Array(vec![
    Object::Integer(0),
    Object::Integer(0),
    Object::Integer(612),
    Object::Integer(792),
    Object::Integer(999),
  ]);
  // Only first 4 used
  assert_eq!(parse_page_box(&obj), Some([0.0, 0.0, 612.0, 792.0]));
}

fn reference(id: u32) -> Object {
  Object::Reference((id, 0))
}

fn literal(bytes: &[u8]) -> Object {
  Object::String(bytes.to_vec(), StringFormat::Literal)
}

/// A document of one page, with these entries in its Info dictionary and these added to its
/// catalog.
fn described(info: Dictionary, in_catalog: Dictionary) -> Document {
  let mut doc = Document::with_version("1.7");
  let pages = dictionary! { "Type" => "Pages", "Kids" => vec![reference(2)], "Count" => 1 };
  let page = dictionary! {
    "Type" => "Page",
    "Parent" => reference(1),
    "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
  };
  let mut catalog = dictionary! { "Type" => "Catalog", "Pages" => reference(1) };
  catalog.extend(&in_catalog);
  doc.objects.insert((1, 0), pages.into());
  doc.objects.insert((2, 0), page.into());
  doc.objects.insert((3, 0), catalog.into());
  doc.objects.insert((4, 0), info.into());
  doc.trailer.set("Root", reference(3));
  doc.trailer.set("Info", reference(4));
  doc.max_id = 4;
  doc
}

fn titled(title: Object) -> Option<String> {
  let doc = described(dictionary! { "Title" => title }, Dictionary::new());
  extract_metadata(&doc).title
}

#[test]
fn the_info_dictionary_says_what_the_document_is() {
  let info = dictionary! {
    "Title" => literal(b"A Harbor at Dusk"),
    "Author" => literal(b"Ada Marsh"),
    "Subject" => literal(b"Notes on light"),
    "Keywords" => literal(b"harbor, dusk; light"),
    "Creator" => literal(b"A word processor"),
  };
  let meta = extract_metadata(&described(info, Dictionary::new()));
  assert_eq!(meta.title.as_deref(), Some("A Harbor at Dusk"));
  assert_eq!(meta.author.as_deref(), Some("Ada Marsh"));
  assert_eq!(meta.subject.as_deref(), Some("Notes on light"));
  // As the file writes them: one string, which the caller may split.
  assert_eq!(meta.keywords.as_deref(), Some("harbor, dusk; light"));
  assert_eq!(meta.creator.as_deref(), Some("A word processor"));
}

#[test]
fn a_document_that_says_nothing_of_itself_has_none_of_these() {
  let meta = extract_metadata(&described(Dictionary::new(), Dictionary::new()));
  assert_eq!(
    (
      meta.title,
      meta.author,
      meta.subject,
      meta.keywords,
      meta.language
    ),
    (None, None, None, None, None)
  );
  assert!(!meta.is_encrypted);
}

#[test]
fn a_text_string_is_read_in_whichever_way_it_is_written() {
  // Sixteen bits to a character behind a mark, in either order of bytes.
  let big = Object::String(
    vec![0xFE, 0xFF, 0x00, 0x4D, 0x00, 0xFC],
    StringFormat::Hexadecimal,
  );
  assert_eq!(titled(big).as_deref(), Some("M\u{fc}"));
  let little = Object::String(
    vec![0xFF, 0xFE, 0x4D, 0x00, 0xFC, 0x00],
    StringFormat::Hexadecimal,
  );
  assert_eq!(titled(little).as_deref(), Some("M\u{fc}"));
  // UTF-8 behind its mark, as the second version of the format allows.
  assert_eq!(
    titled(literal(b"\xEF\xBB\xBFCaf\xC3\xA9")).as_deref(),
    Some("Caf\u{e9}")
  );
  // The format's own encoding of one byte to a character.
  assert_eq!(
    titled(literal(b"Stra\xDFe")).as_deref(),
    Some("Stra\u{df}e")
  );
  // UTF-8 with no mark, which the format does not allow and many writers produce.
  assert_eq!(
    titled(literal(b"Caf\xC3\xA9 \xE2\x80\x94 Men\xC3\xBC")).as_deref(),
    Some("Caf\u{e9} \u{2014} Men\u{fc}")
  );
}

#[test]
fn a_text_string_that_holds_nothing_is_none() {
  assert_eq!(titled(literal(b"")), None);
  assert_eq!(titled(literal(b"  \0")), None);
  assert_eq!(titled(Object::Integer(7)), None);
  // What surrounds the words is not part of them.
  assert_eq!(
    titled(literal(b"  A Harbor at Dusk\0")).as_deref(),
    Some("A Harbor at Dusk")
  );
}

#[test]
fn a_text_string_kept_in_an_object_of_its_own_is_followed() {
  let mut doc = described(dictionary! { "Title" => reference(9) }, Dictionary::new());
  doc.objects.insert((9, 0), literal(b"Kept elsewhere"));
  assert_eq!(
    extract_metadata(&doc).title.as_deref(),
    Some("Kept elsewhere")
  );
}

#[test]
fn the_catalog_names_the_language_of_the_text() {
  let doc = described(
    Dictionary::new(),
    dictionary! { "Lang" => literal(b"en-GB") },
  );
  assert_eq!(extract_metadata(&doc).language.as_deref(), Some("en-GB"));
  let mut doc = described(Dictionary::new(), dictionary! { "Lang" => reference(9) });
  doc.objects.insert((9, 0), literal(b"de-DE"));
  assert_eq!(extract_metadata(&doc).language.as_deref(), Some("de-DE"));
  let doc = described(Dictionary::new(), dictionary! { "Lang" => literal(b"") });
  assert_eq!(extract_metadata(&doc).language, None);
}

#[test]
fn a_file_that_was_encrypted_says_so_once_it_is_open() {
  let mut doc = described(
    dictionary! { "Title" => literal(b"Under lock") },
    Dictionary::new(),
  );
  doc.trailer.set(
    "ID",
    vec![literal(b"0123456789abcdef"), literal(b"0123456789abcdef")],
  );
  // Locked for changes, and open to anyone for reading: no password is asked for.
  let state = EncryptionState::try_from(EncryptionVersion::V2 {
    document: &doc,
    owner_password: "owner",
    user_password: "",
    key_length: 128,
    permissions: Permissions::all(),
  })
  .unwrap();
  doc.encrypt(&state).unwrap();
  let mut bytes = Vec::new();
  doc.save_to(&mut bytes).unwrap();

  let meta = Pdf::open(bytes).unwrap().metadata();
  assert!(meta.is_encrypted);
  // What it says of itself is read as written, not as it lay encrypted.
  assert_eq!(meta.title.as_deref(), Some("Under lock"));
}

/// A picture of one pixel, kept as the page's resources keep one.
fn picture() -> Object {
  let dict = dictionary! {
    "Type" => "XObject",
    "Subtype" => "Image",
    "Width" => 1,
    "Height" => 1,
    "BitsPerComponent" => 8,
    "ColorSpace" => "DeviceGray",
  };
  Stream::new(dict, vec![0x80]).into()
}

/// A page that keeps these under these names and paints what `content` says.
fn page_painting(doc: &mut Document, id: u32, kept: Dictionary, content: &[u8]) {
  let content_id = id + 100;
  doc.objects.insert(
    (content_id, 0),
    Stream::new(Dictionary::new(), content.to_vec()).into(),
  );
  let page = dictionary! {
    "Type" => "Page",
    "Parent" => reference(1),
    "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
    "Resources" => dictionary! { "XObject" => kept },
    "Contents" => reference(content_id),
  };
  doc.objects.insert((id, 0), page.into());
}

#[test]
fn the_images_the_pages_paint_are_counted_without_decoding_them() {
  let mut doc = Document::with_version("1.7");
  for id in [20, 21, 22] {
    doc.objects.insert((id, 0), picture());
  }
  // Something a page paints that is drawing and not a picture, holding a picture of its own.
  let drawing = dictionary! {
    "Type" => "XObject",
    "Subtype" => "Form",
    "Resources" => dictionary! { "XObject" => dictionary! { "Inner" => reference(22) } },
  };
  doc
    .objects
    .insert((23, 0), Stream::new(drawing, b"/Inner Do".to_vec()).into());
  // The first page paints one of the two pictures it keeps, twice.
  page_painting(
    &mut doc,
    2,
    dictionary! { "Painted" => reference(20), "Kept" => reference(21) },
    b"q /Painted Do Q q /Painted Do Q",
  );
  // The second paints a picture and a drawing.
  page_painting(
    &mut doc,
    3,
    dictionary! { "Photo" => reference(21), "Drawing" => reference(23) },
    b"/Photo Do /Drawing Do",
  );
  let pages =
    dictionary! { "Type" => "Pages", "Kids" => vec![reference(2), reference(3)], "Count" => 2 };
  doc.objects.insert((1, 0), pages.into());
  doc.objects.insert(
    (4, 0),
    dictionary! { "Type" => "Catalog", "Pages" => reference(1) }.into(),
  );
  doc.trailer.set("Root", reference(4));
  doc.max_id = 200;

  // One on the first page and one on the second: a picture is counted once for each page
  // that paints it, and one that is kept and never painted is not counted.
  assert_eq!(crate::images::count_images(&doc), 2);
  // It is the number of images the full reading returns.
  assert_eq!(
    usize::try_from(crate::images::count_images(&doc)).unwrap(),
    crate::images::extract_images_raw(&doc).len()
  );
  assert_eq!(
    crate::images::count_images(&Document::with_version("1.7")),
    0
  );
}

#[test]
fn a_file_that_asks_for_a_password_says_it_is_encrypted_and_nothing_it_cannot_read() {
  let info = dictionary! {
    "Title" => literal(b"Under lock"),
    "Creator" => literal(b"A word processor"),
    "CreationDate" => literal(b"D:20261118033000Z"),
  };
  let mut doc = described(info, dictionary! { "Lang" => literal(b"en-GB") });
  doc.trailer.set(
    "ID",
    vec![literal(b"0123456789abcdef"), literal(b"0123456789abcdef")],
  );
  let state = EncryptionState::try_from(EncryptionVersion::V2 {
    document: &doc,
    owner_password: "owner",
    user_password: "a password",
    key_length: 128,
    permissions: Permissions::all(),
  })
  .unwrap();
  doc.encrypt(&state).unwrap();
  let mut bytes = Vec::new();
  doc.save_to(&mut bytes).unwrap();

  // It opens, and nothing in it can be read: not its pages, and none of its strings.
  let meta = Pdf::open(bytes).unwrap().metadata();
  assert!(meta.is_encrypted);
  assert_eq!(meta.page_count, 0);
  assert_eq!(
    (meta.title, meta.language, meta.creator, meta.creation_date),
    (None, None, None, None)
  );
}

#[test]
fn a_date_is_written_out_in_iso_8601() {
  assert_eq!(
    pdf_date_to_iso8601("D:20240131093015-06'00'"),
    "2024-01-31T09:30:15-06:00"
  );
  assert_eq!(
    pdf_date_to_iso8601("D:20240131093015Z"),
    "2024-01-31T09:30:15Z"
  );
  assert_eq!(pdf_date_to_iso8601("2024"), "2024-01-01T00:00:00");
}

#[test]
fn a_date_that_holds_more_than_ascii_is_returned_as_written() {
  // A damaged file: the character stands across the fourteenth byte, where the time zone begins.
  let across_the_zone = "D:2024013109301é";
  assert_eq!(pdf_date_to_iso8601(across_the_zone), across_the_zone);
  // And across the fourth, where the year ends.
  let across_the_year = "D:202é0131";
  assert_eq!(pdf_date_to_iso8601(across_the_year), across_the_year);
}
