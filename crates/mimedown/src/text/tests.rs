use super::*;

fn content(media_type: MediaType) -> Kind {
  Kind::Content(media_type)
}

fn of(text: &str) -> Kind {
  kind(text, false, None)
}

#[test]
fn text_is_decoded_by_its_byte_order_mark_or_as_utf8() {
  assert_eq!(
    decode("caf\u{e9}".as_bytes(), false),
    Some((Encoding::Utf8, "caf\u{e9}".to_string()))
  );
  assert_eq!(
    decode(b"\xEF\xBB\xBFhello", false),
    Some((Encoding::Utf8, "hello".to_string()))
  );
  assert_eq!(
    decode(b"\xFF\xFEh\0i\0", false),
    Some((Encoding::Utf16Le, "hi".to_string()))
  );
  assert_eq!(
    decode(b"\xFE\xFF\0h\0i", false),
    Some((Encoding::Utf16Be, "hi".to_string()))
  );
  assert_eq!(Encoding::Utf16Le.as_str(), "utf-16le");
}

#[test]
fn bytes_that_text_does_not_hold_are_not_text() {
  assert_eq!(decode(b"", false), None);
  assert_eq!(decode(b"a zero\0in the middle", false), None);
  assert_eq!(decode(b"a bell\x07rings", false), None);
  // Mostly bytes above ASCII and not UTF-8: no text of one byte to a character reads so.
  assert_eq!(decode(b"\x89PNG", false), None);
  // A tab, a line ending, a form feed and an escape are text's own.
  assert!(decode(b"one\ttwo\r\nthree\x0C\x1B[0m", false).is_some());
}

#[test]
fn a_text_that_is_not_utf8_is_one_byte_to_a_character() {
  let latin = b"Stra\xDFe, caf\xE9 and a good many plain words besides";
  let (encoding, text) = decode(latin, false).unwrap();
  assert_eq!(
    (encoding, encoding.as_str()),
    (Encoding::EightBit, "unknown-8bit")
  );
  assert!(text.starts_with("Stra\u{df}e, caf\u{e9}"));
}

#[test]
fn a_character_cut_in_two_by_the_end_of_the_bytes_given_is_not_a_fault() {
  let bytes = "caf\u{e9}".as_bytes();
  let cut_short = &bytes[..bytes.len() - 1];
  assert_eq!(
    decode(cut_short, true),
    Some((Encoding::Utf8, "caf".to_string()))
  );
  // The same bytes as a whole file are not UTF-8, and too short to be anything else.
  assert_eq!(decode(cut_short, false).map(|(encoding, _)| encoding), None);
}

#[test]
fn how_a_text_begins_says_what_some_kinds_are() {
  assert_eq!(
    of("BEGIN:VCARD\nVERSION:4.0\nFN:A Name\nEND:VCARD\n"),
    content(MediaType::Vcard)
  );
  assert_eq!(
    of("BEGIN:VCALENDAR\nVERSION:2.0\n"),
    content(MediaType::Calendar)
  );
  assert_eq!(
    of("#EXTM3U\n#EXT-X-VERSION:3\n"),
    content(MediaType::HlsPlaylist)
  );
  assert_eq!(
    of("WEBVTT\n\n00:01.000 --> 00:04.000\nHello\n"),
    content(MediaType::Vtt)
  );
  assert_eq!(
    of("%!PS-Adobe-3.0\n%%Creator: by hand\n"),
    content(MediaType::Postscript)
  );
  assert_eq!(
    of("just words, and nothing that says what they are"),
    content(MediaType::Txt)
  );
}

#[test]
fn a_script_is_named_for_the_interpreter_its_first_line_asks_for() {
  let cases = [
    ("#!/bin/sh\necho hi\n", MediaType::Shell),
    ("#!/usr/bin/env bash\nset -e\n", MediaType::Shell),
    (
      "#!/usr/bin/env -S python3 -u\nprint(1)\n",
      MediaType::Python,
    ),
    ("#!/usr/bin/python3.12\nprint(1)\n", MediaType::Python),
    (
      "#!/usr/bin/env node\nconsole.log(1)\n",
      MediaType::JavaScript,
    ),
    ("#!/usr/bin/ruby\nputs 1\n", MediaType::Ruby),
    ("#!/bin/tcsh\n", MediaType::CShell),
  ];
  for (script, media_type) in cases {
    assert_eq!(of(script), content(media_type), "{script:?}");
  }
  // An interpreter with no type here is a text like any other.
  assert_eq!(
    of("#!/usr/bin/awk -f\n{ print }\n"),
    content(MediaType::Txt)
  );
}

#[test]
fn json_is_what_reads_through_as_json() {
  assert_eq!(
    of(r#"{"name": "dusk", "tags": ["a", "b"], "size": -1.5e3, "ok": true, "none": null}"#),
    content(MediaType::Json)
  );
  assert_eq!(of("[1, 2, 3]\n"), content(MediaType::Json));
  assert_eq!(of("  \n{}\n"), content(MediaType::Json));
  // What begins like JSON and is not.
  assert_eq!(of("[section]\nkey = value\n"), content(MediaType::Txt));
  assert_eq!(
    of("[1]: https://example.com \"a footnote\"\n"),
    content(MediaType::Txt)
  );
  assert_eq!(of("{ not: 'json' }"), content(MediaType::Txt));
  assert_eq!(of("[1, 2,, 3]"), content(MediaType::Txt));
}

#[test]
fn a_value_on_every_line_is_json_lines() {
  assert_eq!(
    of("{\"a\": 1}\n{\"a\": 2}\n{\"a\": 3}\n"),
    content(MediaType::Jsonl)
  );
  assert_eq!(of("{\"a\": 1}\nand then words\n"), content(MediaType::Txt));
}

#[test]
fn json_cut_off_by_the_end_of_the_bytes_given_is_still_json() {
  assert_eq!(
    kind("{\"name\": \"dusk\", \"tags\": [\"a\", \"b", true, None),
    content(MediaType::Json)
  );
  // The same text as a whole file is not JSON.
  assert_eq!(
    kind("{\"name\": \"dusk\", \"tags\": [\"a\", \"b", false, None),
    content(MediaType::Txt)
  );
}

#[test]
fn markup_is_named_for_its_root() {
  assert_eq!(
    of("<!DOCTYPE html>\n<html lang=\"en\"><head>"),
    content(MediaType::Html)
  );
  assert_eq!(of("<html><body>"), content(MediaType::Html));
  assert_eq!(
    of("<?xml version=\"1.0\"?>\n<html xmlns=\"http://www.w3.org/1999/xhtml\">"),
    content(MediaType::Xhtml)
  );
  assert_eq!(
    of(
      "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!-- a note -->\n<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10\">"
    ),
    content(MediaType::Svg)
  );
  assert_eq!(
    of("<svg viewBox=\"0 0 10 10\"><path d=\"M0 0\"/></svg>"),
    content(MediaType::Svg)
  );
  assert_eq!(
    of("<?xml version=\"1.0\"?>\n<COLLADA version=\"1.4.1\">"),
    content(MediaType::Collada)
  );
  assert_eq!(
    of("<?xml version=\"1.0\"?>\n<catalog><book/></catalog>"),
    content(MediaType::Xml)
  );
  assert_eq!(
    of("<feed xmlns=\"http://www.w3.org/2005/Atom\">"),
    content(MediaType::Xml)
  );
  // An angle bracket and no markup behind it.
  assert_eq!(
    of("<3 you, and other things people type"),
    content(MediaType::Txt)
  );
  assert_eq!(
    of("<p>a fragment, with nothing to say it is XML</p>"),
    content(MediaType::Txt)
  );
}

#[test]
fn the_root_s_own_attributes_are_read_and_no_others() {
  let root = markup::root("<?xml version='1.0'?><!DOCTYPE svg [<!ENTITY a \"b>c\">]>\n<svg:svg width=\"4cm\" data-note='a > b' viewBox=\"0 0 1 1\"><rect width=\"9\"/>").unwrap();
  assert_eq!(
    (
      root.name.as_str(),
      root.declared_xml,
      root.doctype.as_deref()
    ),
    ("svg", true, Some("svg"))
  );
  assert_eq!(root.attribute("width"), Some("4cm"));
  assert_eq!(root.attribute("data-note"), Some("a > b"));
  assert_eq!(root.attributes.len(), 3);
  // A tag that the text stops inside is not a root yet.
  assert_eq!(markup::root("<svg width=\"10\" hei"), None);
  assert_eq!(markup::root("plain words"), None);
}

#[test]
fn a_name_says_what_the_bytes_cannot() {
  let named = |text: &str, name: &str| kind(text, false, Some(name));
  assert_eq!(
    named("# A heading\n\nSome words.\n", "README.md"),
    Kind::Named(MediaType::Markdown)
  );
  assert_eq!(
    named("a,b,c\n1,2,3\n", "report.csv"),
    Kind::Named(MediaType::Csv)
  );
  assert_eq!(
    named("fn main() {}\n", "src/main.rs"),
    Kind::Named(MediaType::Rust)
  );
  assert_eq!(
    named("const a = 1;\n", "index.ts"),
    Kind::Named(MediaType::TypeScript)
  );
  assert_eq!(
    named("key: value\n", "config.YAML"),
    Kind::Named(MediaType::Yaml)
  );
  assert_eq!(
    named("v 0 0 0\nv 1 0 0\nf 1 2 3\n", "chair.obj"),
    Kind::Named(MediaType::ModelObj)
  );
  // The kinds of JSON look alike, and the name says which.
  assert_eq!(
    named("{\"asset\": {\"version\": \"2.0\"}}", "scene.gltf"),
    Kind::Named(MediaType::Gltf)
  );
  assert_eq!(
    named("{\"a\": 1}\n{\"a\": 2}\n", "events.ndjson"),
    Kind::Named(MediaType::Ndjson)
  );
  assert_eq!(named("{\"a\": 1}", "data.json"), content(MediaType::Json));
}

#[test]
fn a_name_is_not_believed_against_the_bytes_or_for_a_type_text_cannot_be() {
  let named = |text: &str, name: &str| kind(text, false, Some(name));
  // What the text says of itself comes first.
  assert_eq!(
    named("<!DOCTYPE html><html>", "notes.md"),
    content(MediaType::Html)
  );
  assert_eq!(named("#!/bin/sh\n", "run.py"), content(MediaType::Shell));
  // Words are not a picture, whatever they are called.
  assert_eq!(named("just words", "photo.png"), content(MediaType::Txt));
  assert_eq!(named("just words", "report.pdf"), content(MediaType::Txt));
  assert_eq!(named("just words", "no-extension"), content(MediaType::Txt));
}

#[test]
fn a_text_is_counted_by_its_lines_and_its_words() {
  let counted = counts("  A heading\n\nsome words here\nand a last line");
  assert_eq!((counted.lines, counted.words), (4, 9));
  assert_eq!(
    counted.preview.as_deref(),
    Some("A heading\n\nsome words here\nand a last line")
  );
  let long = counts(&"word ".repeat(400));
  assert_eq!(
    long.preview.map(|preview| preview.chars().count()),
    Some(499)
  );
  assert_eq!(counts("   \n").preview, None);
}

#[test]
fn html_is_html_whether_or_not_its_values_are_quoted() {
  for start in [
    "<!DOCTYPE html><html lang=en><head><title>x</title></head>",
    "<!doctype html><meta charset=utf-8><title>x</title>",
    "<html class=no-js lang=\"en\">",
    "<!doctype html><html hidden data-n = 3 ><body>",
  ] {
    assert_eq!(of(start), content(MediaType::Html), "{start}");
  }
  let root = markup::root("<svg width=96 height='64' hidden viewBox=\"0 0 96 64\"/>").unwrap();
  assert_eq!(root.attribute("width"), Some("96"));
  assert_eq!(root.attribute("height"), Some("64"));
  assert_eq!(root.attribute("hidden"), Some(""));
  assert_eq!(root.attribute("viewBox"), Some("0 0 96 64"));
}

#[test]
fn text_that_opens_with_an_angle_bracket_is_markup_or_cut_short_or_neither() {
  use markup::Opening;
  for neither in [
    "<<<<<<< HEAD\nours\n",
    "<?php echo 1;",
    "<3 of it",
    "< 5 and > 2",
    "plain",
  ] {
    assert_eq!(markup::opening(neither), Opening::NotMarkup, "{neither}");
  }
  for cut_short in [
    "<?xml version=\"1.0\"?><!-- a comment the bytes given stop in",
    "<?xml version=\"1.0\" encod",
    "<!DOCTYPE ht",
    "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"9",
    "<svg",
    "<?xml version=\"1.0\"?>\n",
  ] {
    assert_eq!(
      markup::opening(cut_short),
      Opening::Unfinished,
      "{cut_short}"
    );
  }
  assert!(matches!(markup::opening("<a href=x>"), Opening::Root(_)));
}

#[test]
fn sixteen_bit_numbers_are_not_text_in_utf16() {
  // Behind a byte-order mark: control characters and a zero, which text does not hold.
  assert_eq!(decode(b"\xFF\xFE\x01\0\x02\0\0\0", false), None);
  // A table of small numbers, each two bytes: every other byte is zero, as in western text.
  let table: Vec<u8> = (1u16..=200)
    .cycle()
    .take(2000)
    .flat_map(u16::to_le_bytes)
    .collect();
  assert_eq!(decode_leniently(&table, false), None);
  // A few bytes are too few to tell by.
  assert_eq!(decode_leniently(b"A\0\x01\x02", false), None);
  assert_eq!(decode_leniently(b"h\0i\0", false), None);
  // Text with no mark in front of it is still read.
  let line = "A plain line of text.\n";
  let unmarked: Vec<u8> = line.encode_utf16().flat_map(u16::to_le_bytes).collect();
  assert_eq!(
    decode_leniently(&unmarked, false),
    Some((Encoding::Utf16Le, line.to_string()))
  );
}

#[test]
fn a_pair_cut_in_two_by_the_end_of_the_bytes_given_is_not_a_fault_in_utf16_either() {
  // A character outside the first plane is two units. The bytes given stop after the first.
  let mut bytes = vec![0xFF, 0xFE];
  bytes.extend("hi".encode_utf16().flat_map(u16::to_le_bytes));
  bytes.extend(0xD83Du16.to_le_bytes());
  assert_eq!(
    decode(&bytes, true),
    Some((Encoding::Utf16Le, "hi".to_string()))
  );
  // In a whole file the same bytes are a fault.
  assert_eq!(decode(&bytes, false), None);
}

#[test]
fn a_name_tells_the_kinds_of_json_apart() {
  // To most programs `.jsonc` and `.json5` are JSON, which is what the tables say first. A
  // file's own name is more exact than that.
  assert_eq!(
    kind("{a: 1}", false, Some("x.json5")),
    Kind::Named(MediaType::Json5)
  );
  assert_eq!(
    kind("{ // why\n \"a\": 1 }", false, Some("x.jsonc")),
    Kind::Named(MediaType::Jsonc)
  );
  assert_eq!(
    kind("{\"a\": 1}", false, Some("tsconfig.jsonc")),
    Kind::Named(MediaType::Jsonc)
  );
  assert_eq!(
    kind("{\"a\": 1}", false, Some("x.json")),
    content(MediaType::Json)
  );
}

#[test]
fn a_page_that_claims_xhtml_is_html_unless_it_opens_as_xml() {
  // The doctype and the namespace of XHTML, on a page that opens as HTML does. Pages written
  // so are served as HTML and seldom parse as XML: named `.xhtml`, a browser reads one with
  // its XML parser and shows none of it.
  let claimed = "\r\n<!DOCTYPE html PUBLIC \"-//W3C//DTD XHTML 1.0 Transitional//EN\" \"http://www.w3.org/TR/xhtml1/DTD/xhtml1-transitional.dtd\">\r\n\r\n<html xmlns=\"http://www.w3.org/1999/xhtml\">\r\n<head><title>A page</title>";
  assert_eq!(of(claimed), content(MediaType::Html));
  assert_eq!(
    of("<html xmlns=\"http://www.w3.org/1999/xhtml\"><head>"),
    content(MediaType::Html)
  );
  // An XML declaration and no namespace: HTML with a line in front of it.
  assert_eq!(
    of("<?xml version=\"1.0\"?>\n<html><head>"),
    content(MediaType::Html)
  );
  // Both: the page opens as XML does and names the namespace.
  let meant = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE html PUBLIC \"-//W3C//DTD XHTML 1.0 Strict//EN\" \"http://www.w3.org/TR/xhtml1/DTD/xhtml1-strict.dtd\">\n<html xmlns=\"http://www.w3.org/1999/xhtml\" xml:lang=\"en\">";
  assert_eq!(of(meant), content(MediaType::Xhtml));
}
