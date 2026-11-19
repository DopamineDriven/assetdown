use super::*;
use crate::source::Piece;
use crate::{DocumentDetail, Facts, MediaType, specs, specs_source};

fn document(found: crate::Specs) -> DocumentFacts {
  match found.facts {
    Facts::Document(facts) => *facts,
    other => panic!(
      "no document facts for {}: {other:?} {:?}",
      found.media_type, found.warnings
    ),
  }
}

fn detail(facts: &DocumentFacts) -> &RtfDetail {
  match &facts.detail {
    DocumentDetail::Rtf(detail) => detail,
    other => panic!("not an RTF's detail: {other:?}"),
  }
}

/// The text of a document, as its preview gives it.
fn text_of(file: &[u8]) -> String {
  let found = specs(file);
  assert_eq!(found.warnings, Vec::<String>::new());
  document(found).text_preview.unwrap_or_default()
}

#[test]
fn the_group_near_the_start_says_what_the_document_is() {
  let file = br"{\rtf1\ansi\ansicpg1252\deff0\deflang1033
{\info{\title A Harbor at Dusk}{\author Ada Marsh}{\subject Notes on light}{\keywords harbor, dusk; light}{\creatim\yr2026\mo11\dy17\hr23\min36\sec27}{\revtim\yr2026\mo11\dy18\hr2\min5}\nofpages3\nofwords1200\nofchars7000}
\paperw12240\paperh15840
\pard The quick brown fox.\par}";
  let found = specs(file);
  assert_eq!(found.media_type, MediaType::Rtf);
  assert_eq!(found.warnings, Vec::<String>::new());
  assert_eq!(found.needs, None);
  let facts = document(found);
  assert_eq!(facts.title.as_deref(), Some("A Harbor at Dusk"));
  assert_eq!(facts.author.as_deref(), Some("Ada Marsh"));
  assert_eq!(facts.subject.as_deref(), Some("Notes on light"));
  assert_eq!(facts.keywords, ["harbor", "dusk", "light"]);
  // As the file writes them: with no zone, since it states none.
  assert_eq!(facts.created_date.as_deref(), Some("2026-11-17T23:36:27"));
  assert_eq!(facts.modified_date.as_deref(), Some("2026-11-18T02:05:00"));
  // The pages are the count the file last wrote down; the words are counted here.
  assert_eq!((facts.page_count, facts.word_count), (Some(3), Some(4)));
  assert_eq!(facts.language.as_deref(), Some("en-US"));
  assert_eq!(
    (facts.page_width, facts.page_height),
    (Some(612.0), Some(792.0))
  );
  assert_eq!(facts.text_preview.as_deref(), Some("The quick brown fox."));
  assert_eq!(detail(&facts).code_page, Some(1252));
}

#[test]
fn the_text_is_what_is_left_once_the_tables_are_set_aside() {
  let file = br"{\rtf1\ansi{\fonttbl{\f0 Arial;}{\f1\froman Times New Roman;}}{\colortbl;\red0\green0\blue0;}{\stylesheet{\s0 Normal;}{\s1 heading 1;}}{\*\generator Riched20 10.0.19041;}\pard First line\par Second \b bold\b0  line\par}";
  let found = specs(file);
  assert_eq!(found.warnings, Vec::<String>::new());
  let facts = document(found);
  assert_eq!(
    facts.text_preview.as_deref(),
    Some("First line\nSecond bold line")
  );
  assert_eq!(facts.word_count, Some(5));
  assert_eq!(
    detail(&facts).generator.as_deref(),
    Some("Riched20 10.0.19041")
  );
  assert_eq!(
    (facts.title, facts.page_count, facts.language),
    (None, None, None)
  );
}

#[test]
fn characters_written_as_escapes_are_read() {
  let file = br"{\rtf1\ansi\ansicpg1252 caf\'e9 \'93quoted\'94 \u8364? 5 \uc0\u233 t\'e9 \uc1 \u-10179?\u-8704? \{braces\} back\\slash a\~b x\emdash y\tab z\line w}";
  let found = specs(file);
  assert_eq!(found.warnings, Vec::<String>::new());
  let facts = document(found);
  assert_eq!(
    facts.text_preview.as_deref(),
    Some(
      "caf\u{e9} \u{201c}quoted\u{201d} \u{20ac} 5 \u{e9}t\u{e9} \u{1f600} {braces} back\\slash a\u{a0}b x\u{2014}y\tz\nw"
    )
  );
  assert_eq!(facts.word_count, Some(13));
}

#[test]
fn escapes_are_read_in_what_the_document_says_of_itself_too() {
  let facts = document(specs(
    br"{\rtf1\ansi\ansicpg1252{\info{\title Caf\'e9 \u8212? Men\u252?}}text}",
  ));
  assert_eq!(facts.title.as_deref(), Some("Caf\u{e9} \u{2014} Men\u{fc}"));
}

#[test]
fn line_endings_in_the_file_are_not_text() {
  assert_eq!(
    text_of(b"{\\rtf1 one line\r\nstill that line\\par\r\nthe next}"),
    "one linestill that line\nthe next"
  );
}

#[test]
fn the_page_s_size_is_given_in_points() {
  let facts = document(specs(br"{\rtf1\ansi\paperw11906\paperh16838 text}"));
  assert_eq!(
    (facts.page_width, facts.page_height),
    (Some(595.3), Some(841.9))
  );
  let none = document(specs(br"{\rtf1\ansi text}"));
  assert_eq!((none.page_width, none.page_height), (None, None));
}

#[test]
fn the_default_language_is_given_by_its_tag() {
  let language = |number: u32| {
    document(specs(
      format!("{{\\rtf1\\ansi\\deflang{number} text}}").as_bytes(),
    ))
    .language
  };
  assert_eq!(language(1033).as_deref(), Some("en-US"));
  assert_eq!(language(2057).as_deref(), Some("en-GB"));
  assert_eq!(language(1031).as_deref(), Some("de-DE"));
  assert_eq!(language(1036).as_deref(), Some("fr-FR"));
  assert_eq!(language(1041).as_deref(), Some("ja-JP"));
  assert_eq!(language(2052).as_deref(), Some("zh-CN"));
  // The number for no language, and one that is no language's.
  assert_eq!(language(1024), None);
  assert_eq!(language(9999), None);
}

#[test]
fn bytes_in_a_code_page_that_is_not_read_are_left_out_and_said_to_be() {
  let found = specs(br"{\rtf1\ansi\ansicpg1251 \'cf\'f0\'e8 word \u1055\'cf}");
  assert_eq!(
    found.warnings,
    ["characters above 127 written in code page 1251 are not read"]
  );
  let facts = document(found);
  // What stands in for a character already written by its number is not such a byte.
  assert_eq!(facts.text_preview.as_deref(), Some("word \u{41f}"));
  assert_eq!(detail(&facts).code_page, Some(1251));
}

#[test]
fn pictures_are_counted_and_are_not_text() {
  let file = br"{\rtf1\ansi one {\pict\pngblip\picw10\pich10 89504e470d0a1a0a}two {\pict\jpegblip ffd8ffe0}{\*\shppict{\pict\pngblip 00ff}}{\nonshppict{\pict\wmetafile8 0100}} three}";
  let found = specs(file);
  assert_eq!(found.warnings, Vec::<String>::new());
  let facts = document(found);
  // The same picture written a second time for older readers is one picture.
  assert_eq!(facts.image_count, Some(3));
  assert_eq!(facts.text_preview.as_deref(), Some("one two  three"));
}

#[test]
fn bytes_kept_as_they_are_inside_a_picture_are_stepped_over() {
  // Five bytes that would otherwise open and close groups.
  assert_eq!(
    text_of(b"{\\rtf1 before{\\pict\\bin5 {}{}\\}after}"),
    "beforeafter"
  );
}

#[test]
fn what_is_not_the_document_s_own_text_is_left_out() {
  let file = br#"{\rtf1\ansi Visible {\field{\*\fldinst HYPERLINK "http://example.com"}{\fldrslt the link}}{\header page top}{\footer page bottom}{\footnote a note}{\*\unknowndest secret}{\*\annotation a remark}.}"#;
  assert_eq!(text_of(file), "Visible the link.");
}

#[test]
fn a_file_of_which_only_the_start_was_given_is_read_as_far_as_it_goes() {
  let whole = [
    &br"{\rtf1\ansi{\info{\title A Harbor at Dusk}}\pard "[..],
    "word ".repeat(400).as_bytes(),
    b"}",
  ]
  .concat();
  let pieces = [Piece {
    offset: 0,
    bytes: &whole[..300],
  }];
  let size = u64::try_from(whole.len()).unwrap();
  let found = specs_source(&Source::from_pieces(&pieces, Some(size)).unwrap());
  assert_eq!(found.media_type, MediaType::Rtf);
  assert_eq!(
    found.needs.map(|range| (range.offset, range.length)),
    Some((300, size - 300))
  );
  let facts = document(found);
  assert_eq!(facts.title.as_deref(), Some("A Harbor at Dusk"));
  assert!(
    facts
      .word_count
      .is_some_and(|words| words > 10 && words < 400)
  );
}

#[test]
fn a_file_whose_groups_do_not_close_or_close_too_often_is_still_read() {
  assert_eq!(
    text_of(br"{\rtf1 never {closed {at all"),
    "never closed at all"
  );
  assert_eq!(text_of(br"{\rtf1 closed}}}} too often"), "closed");
  // Groups nested deeper than any document has.
  let deep = [&br"{\rtf1 "[..], &b"{".repeat(100_000), b"deep"].concat();
  let found = specs(&deep);
  assert_eq!(
    found.warnings,
    ["its groups are nested deeper than is read"]
  );
}

#[test]
fn text_written_twice_for_older_readers_is_read_once_with_every_character() {
  // The plain copy first, then the copy in which each character is written by its number.
  let file = br"{\rtf1\ansi\ansicpg1252{\info{\title {\upr{Menu}{\*\ud{\uc0 Men\u252 }}}}}text}";
  let facts = document(specs(file));
  assert_eq!(facts.title.as_deref(), Some("Men\u{fc}"));
}

#[test]
fn a_fact_stated_twice_is_the_first_one() {
  // One writer names who made the document and then who last saved it, both as its author.
  let file = br"{\rtf1\ansi{\info{\author Ada Marsh}{\creatim\yr2026\mo11\dy17}{\author Someone Else}{\title One}{\title Two}}text}";
  let facts = document(specs(file));
  assert_eq!(facts.author.as_deref(), Some("Ada Marsh"));
  assert_eq!(facts.title.as_deref(), Some("One"));
  assert_eq!(facts.created_date.as_deref(), Some("2026-11-17T00:00:00"));
}

#[test]
fn a_moment_of_all_zeros_is_one_the_writer_did_not_know() {
  let file = br"{\rtf1\ansi{\info{\creatim\yr0\mo0\dy0\hr0\min0}{\revtim\yr2026\mo13\dy40}}text}";
  let facts = document(specs(file));
  assert_eq!((facts.created_date, facts.modified_date), (None, None));
}

#[test]
fn a_language_stated_on_the_text_is_the_one_most_of_the_text_is_in() {
  // No language for the document as a whole; one named in a style, which is not text; and
  // then the text, most of it in one language and a few words in another.
  let file = br"{\rtf1\ansi{\stylesheet{\s0\lang1036 Normal;}}\pard\lang1031 Der schnelle braune Fuchs springt {\lang1033 over the} faulen Hund.\par}";
  assert_eq!(document(specs(file)).language.as_deref(), Some("de-DE"));
  // Text that names no language of its own is in the document's.
  let file = br"{\rtf1\ansi\deflang2057 Most of the words are here {\lang1036 et deux ici}.}";
  assert_eq!(document(specs(file)).language.as_deref(), Some("en-GB"));
  // The number for text that is not to be checked names no language.
  let file = br"{\rtf1\ansi\deflang1033\lang1024 words that are not to be checked}";
  assert_eq!(document(specs(file)).language.as_deref(), Some("en-US"));
}

#[test]
fn a_picture_placed_as_a_shape_is_counted_and_a_shape_s_own_words_are_text() {
  // A shape: its properties by name and value, one of which is its picture; the words of a
  // text box; and after it what an older reader is to show in its place.
  let file = br"{\rtf1\ansi one {\shp{\*\shpinst\shpleft0{\sp{\sn shapeType}{\sv 75}}{\sp{\sn pib}{\sv {\pict\pngblip 00ff}}}{\shptxt in a box}}{\shprslt{\pict\wmetafile8 0100} in its place}} two}";
  let found = specs(file);
  assert_eq!(found.warnings, Vec::<String>::new());
  let facts = document(found);
  assert_eq!(facts.image_count, Some(1));
  assert_eq!(facts.text_preview.as_deref(), Some("one in a box two"));
}

#[test]
fn what_is_set_aside_stays_aside_whatever_it_holds() {
  // A page's header that holds a shape with a text box and a picture, and a field's result.
  let file = br"{\rtf1\ansi body {\header top {\shp{\*\shpinst{\shptxt page one}{\sp{\sn pib}{\sv {\pict\pngblip 00}}}}}{\field{\fldrslt of five}}}text}";
  let found = specs(file);
  let facts = document(found);
  assert_eq!(facts.text_preview.as_deref(), Some("body text"));
  assert_eq!(facts.image_count, Some(0));
}

#[test]
fn a_document_behind_a_byte_order_mark_or_a_blank_line_is_read_as_one() {
  let plain = b"{\\rtf1\\ansi{\\info{\\title Dusk}}Hello there}".to_vec();
  let marked = [&b"\xEF\xBB\xBF\r\n"[..], &plain].concat();
  for file in [plain, marked] {
    let found = crate::specs(&file);
    assert_eq!(found.media_type, crate::MediaType::Rtf);
    let crate::Facts::Document(facts) = found.facts else {
      panic!("not a document's facts");
    };
    assert!(matches!(facts.detail, crate::DocumentDetail::Rtf(_)));
    assert_eq!(facts.title.as_deref(), Some("Dusk"));
    assert_eq!(facts.text_preview.as_deref(), Some("Hello there"));
  }
}
