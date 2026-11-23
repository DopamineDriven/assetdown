use std::collections::HashSet;

use super::*;

#[test]
fn no_string_names_two_types() {
  let mut seen = HashSet::new();
  for media_type in MediaType::ALL {
    let strings = std::iter::once(media_type.mime()).chain(media_type.aliases().iter().copied());
    for string in strings {
      assert_eq!(
        string,
        string.to_ascii_lowercase(),
        "{string} is not lower case"
      );
      assert_eq!(
        string.matches('/').count(),
        1,
        "{string} is not a type and a subtype"
      );
      assert!(seen.insert(string), "{string} is listed twice");
    }
  }
}

#[test]
fn every_type_has_an_extension_and_the_first_is_its_usual_one() {
  for media_type in MediaType::ALL {
    let extensions = media_type.extensions();
    assert!(!extensions.is_empty(), "{media_type} has no extension");
    assert_eq!(media_type.ext(), extensions[0]);
    for extension in extensions {
      let plain = extension
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit());
      assert!(plain, "{extension} is not lower-case letters and digits");
    }
  }
  assert_eq!(MediaType::Jpeg.ext(), "jpg");
  assert_eq!(MediaType::OggAudio.ext(), "ogg");
  assert_eq!(MediaType::OctetStream.ext(), "bin");
}

#[test]
fn a_type_is_found_by_its_own_string() {
  for media_type in MediaType::ALL {
    assert_eq!(MediaType::from_mime(media_type.mime()), Some(*media_type));
  }
}

#[test]
fn a_type_is_found_by_another_string_in_use_for_it() {
  for media_type in MediaType::ALL {
    for alias in media_type.aliases() {
      assert_eq!(MediaType::from_mime(alias), Some(*media_type), "{alias}");
    }
  }
  assert_eq!(
    MediaType::from_mime("image/vnd.microsoft.icon"),
    Some(MediaType::Ico)
  );
  assert_eq!(MediaType::from_mime("audio/x-wav"), Some(MediaType::Wav));
  assert_eq!(MediaType::from_mime("text/xml"), Some(MediaType::Xml));
}

#[test]
fn case_spaces_and_parameters_do_not_change_what_a_string_names() {
  assert_eq!(
    MediaType::from_mime(" Text/HTML; charset=UTF-8 "),
    Some(MediaType::Html)
  );
  assert_eq!(MediaType::from_mime("IMAGE/PNG"), Some(MediaType::Png));
}

#[test]
fn a_string_that_names_nothing_here_is_no_type() {
  assert_eq!(MediaType::from_mime("application/x-nothing"), None);
  assert_eq!(MediaType::from_mime("png"), None);
  assert_eq!(MediaType::from_mime(""), None);
}

#[test]
fn the_string_returned_is_the_one_the_workspace_already_emits() {
  assert_eq!(MediaType::Wav.mime(), "audio/wav");
  assert_eq!(MediaType::Mkv.mime(), "video/x-matroska");
  assert_eq!(MediaType::Ico.mime(), "image/x-icon");
  assert_eq!(MediaType::Mov.to_string(), "video/quicktime");
}

#[test]
fn fonts_models_and_haptics_have_top_levels_of_their_own() {
  let filed_under = |top_level: TopLevel| {
    let mut strings: Vec<&str> = MediaType::ALL
      .iter()
      .filter(|media_type| media_type.top_level() == top_level)
      .map(|media_type| media_type.mime())
      .collect();
    strings.sort_unstable();
    strings
  };
  assert_eq!(
    filed_under(TopLevel::Font),
    [
      "font/collection",
      "font/otf",
      "font/ttf",
      "font/woff",
      "font/woff2"
    ]
  );
  assert_eq!(
    filed_under(TopLevel::Model),
    [
      "model/3mf",
      "model/gltf+json",
      "model/gltf-binary",
      "model/iges",
      "model/mesh",
      "model/mtl",
      "model/obj",
      "model/step",
      "model/stl",
      "model/vnd.collada+xml",
      "model/vnd.pixar.usd",
      "model/vnd.usda",
      "model/vnd.usdz+zip",
      "model/vrml",
      "model/x3d+xml",
    ]
  );
  assert_eq!(
    filed_under(TopLevel::Haptics),
    ["haptics/hjif", "haptics/hmpg", "haptics/ivs"]
  );
}

#[test]
fn the_strings_fonts_went_by_before_they_had_their_own_are_accepted() {
  let expected = [
    ("application/font-woff", MediaType::Woff),
    ("application/font-sfnt", MediaType::Ttf),
    ("application/x-font-ttf", MediaType::Ttf),
    ("application/x-font-otf", MediaType::Otf),
    ("application/vnd.ms-opentype", MediaType::Otf),
  ];
  for (string, media_type) in expected {
    assert_eq!(MediaType::from_mime(string), Some(media_type), "{string}");
  }
}

#[test]
fn a_type_names_the_package_that_reads_it_in_full() {
  let expected = [
    (MediaType::Pdf, Parser::Pdfdown),
    (MediaType::Xlsx, Parser::Xlsxdown),
    (MediaType::Docx, Parser::Docxdown),
    (MediaType::Pptx, Parser::Pptxdown),
    (MediaType::Docm, Parser::Docxdown),
    (MediaType::Dotx, Parser::Docxdown),
    (MediaType::Xlsm, Parser::Xlsxdown),
    (MediaType::Xltx, Parser::Xlsxdown),
    (MediaType::Pptm, Parser::Pptxdown),
    (MediaType::Potx, Parser::Pptxdown),
    (MediaType::Ppsx, Parser::Pptxdown),
    (MediaType::Mp3, Parser::Audiodown),
    (MediaType::Wav, Parser::Audiodown),
    (MediaType::Mp4, Parser::Videodown),
    (MediaType::Mov, Parser::Videodown),
    (MediaType::Webm, Parser::Videodown),
    (MediaType::Mkv, Parser::Videodown),
    (MediaType::Tiff, Parser::Tiffdown),
    (MediaType::Jpeg, Parser::Jpegdown),
    (MediaType::Rtf, Parser::Rtfdown),
    (MediaType::Xls, Parser::Xlsdown),
    (MediaType::Doc, Parser::Docdown),
  ];
  for (media_type, parser) in expected {
    assert_eq!(media_type.parser(), Some(parser), "{media_type}");
  }
  let read = MediaType::ALL
    .iter()
    .filter(|media_type| media_type.parser().is_some());
  assert_eq!(read.count(), expected.len());
  assert_eq!(MediaType::Png.parser(), None);
  // A camera's raw file is laid out as a TIFF is, and is not a page to read.
  assert_eq!(MediaType::Dng.parser(), None);
  assert_eq!(Parser::Pdfdown.as_str(), "pdfdown");
  assert_eq!(Parser::Tiffdown.as_str(), "tiffdown");
  assert_eq!(Parser::Jpegdown.as_str(), "jpegdown");
  assert_eq!(Parser::Rtfdown.as_str(), "rtfdown");
  assert_eq!(Parser::Xlsdown.as_str(), "xlsdown");
  assert_eq!(Parser::Docdown.as_str(), "docdown");
}

#[test]
fn the_top_level_is_the_word_in_front_of_the_slash() {
  for media_type in MediaType::ALL {
    let (word, _) = media_type.mime().split_once('/').unwrap();
    assert_eq!(media_type.top_level().as_str(), word, "{media_type}");
  }
  assert_eq!(MediaType::Pdf.top_level(), TopLevel::Application);
  assert_eq!(MediaType::Markdown.top_level(), TopLevel::Text);
  assert_eq!(MediaType::Woff2.top_level(), TopLevel::Font);
  assert_eq!(MediaType::Glb.top_level(), TopLevel::Model);
  assert_eq!(MediaType::Hjif.top_level(), TopLevel::Haptics);
  // The registry files a good deal that is text under `application`.
  assert_eq!(MediaType::Json.top_level(), TopLevel::Application);
  // One font was never given a string under `font`.
  assert_eq!(MediaType::Eot.top_level(), TopLevel::Application);
  // Bytes that name no type are filed under `application` too.
  assert_eq!(MediaType::OctetStream.top_level(), TopLevel::Application);
}
