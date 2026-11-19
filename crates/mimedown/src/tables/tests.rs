use super::*;

/// Each extension of the tables in `@d0paminedriven/metadata` 6.0.2, with the type that package
/// gives for it first. Three rows differ from that package on purpose; each says why.
const EXTENSIONS: [(&str, &str); 133] = [
  ("aac", "audio/aac"),
  ("abw", "application/x-abiword"),
  ("aces", "image/aces"),
  ("apng", "image/apng"),
  ("arc", "application/x-freearc"),
  ("avci", "image/avci"),
  ("avif", "image/avif"),
  ("avi", "video/x-msvideo"),
  ("azw", "application/vnd.amazon.ebook"),
  ("bin", "application/octet-stream"),
  ("bmp", "image/bmp"),
  ("bz", "application/x-bzip"),
  ("bz2", "application/x-bzip2"),
  ("c", "text/x-c"),
  ("cda", "application/x-cdf"),
  ("cjs", "text/javascript"),
  ("cpp", "text/x-c++"),
  ("cs", "text/x-csharp"),
  ("csh", "application/x-csh"),
  ("css", "text/css"),
  ("csv", "text/csv"),
  ("cur", "image/x-icon"),
  ("doc", "application/msword"),
  (
    "docx",
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
  ),
  ("dpx", "image/dpx"),
  ("emf", "image/emf"),
  ("eot", "application/vnd.ms-fontobject"),
  ("epub", "application/epub+zip"),
  ("gif", "image/gif"),
  ("glb", "model/gltf-binary"),
  ("gltf", "model/gltf+json"),
  ("gz", "application/gzip"),
  ("heic", "image/heic"),
  ("hjif", "haptics/hjif"),
  ("hmpg", "haptics/hmpg"),
  ("htm", "text/html"),
  ("html", "text/html"),
  ("ico", "image/x-icon"),
  ("ics", "text/calendar"),
  ("ivs", "haptics/ivs"),
  ("ivt", "haptics/ivs"),
  ("jar", "application/java-archive"),
  ("java", "text/x-java"),
  ("jfif", "image/jpeg"),
  ("jif", "image/jpeg"),
  ("jpe", "image/jpeg"),
  ("jpeg", "image/jpeg"),
  ("jpg", "image/jpeg"),
  ("js", "text/javascript"),
  ("json", "application/json"),
  ("jsonl", "application/jsonl"),
  ("jsonld", "application/ld+json"),
  ("jsonlines", "application/jsonl"),
  ("jsonc", "application/json"),
  ("json5", "application/json"),
  ("ktx", "image/ktx"),
  ("ktx2", "image/ktx2"),
  ("m3u8", "application/vnd.apple.mpegurl"),
  ("m4a", "audio/mp4"),
  ("m4s", "video/mp4"),
  ("m4v", "video/mp4"),
  ("md", "text/markdown"),
  ("mdx", "application/x-mdx"),
  ("mid", "audio/midi"),
  ("midi", "audio/midi"),
  ("mjs", "text/javascript"),
  ("mp3", "audio/mpeg"),
  ("mp4", "video/mp4"),
  ("mpd", "application/dash+xml"),
  ("mpeg", "video/mpeg"),
  ("mpkg", "application/vnd.apple.installer+xml"),
  ("mtlx", "application/xml"),
  ("ndjson", "application/x-ndjson"),
  ("node", "application/node"),
  ("obj", "model/obj"),
  ("odp", "application/vnd.oasis.opendocument.presentation"),
  ("ods", "application/vnd.oasis.opendocument.spreadsheet"),
  ("odt", "application/vnd.oasis.opendocument.text"),
  ("oga", "audio/ogg"),
  ("ogg", "audio/ogg"),
  ("ogv", "video/ogg"),
  ("ogx", "application/ogg"),
  ("opus", "audio/ogg"),
  ("otf", "font/otf"),
  ("pk1", "application/octet-stream"),
  ("png", "image/png"),
  ("pdf", "application/pdf"),
  ("php", "text/x-php"),
  ("pjp", "image/jpeg"),
  ("pjpeg", "image/jpeg"),
  ("pkpass", "application/vnd.apple.pkpass"),
  ("ppt", "application/vnd.ms-powerpoint"),
  (
    "pptx",
    "application/vnd.openxmlformats-officedocument.presentationml.presentation",
  ),
  ("py", "text/x-python"),
  ("pyc", "application/x-python-code"),
  ("rar", "application/vnd.rar"),
  ("rb", "text/x-ruby"),
  ("rs", "text/rust"),
  ("rtf", "application/rtf"),
  ("sh", "application/x-sh"),
  ("sql", "application/sql"),
  ("sse", "text/event-stream"),
  ("svg", "image/svg+xml"),
  ("tar", "application/x-tar"),
  ("tex", "text/x-tex"),
  ("tif", "image/tiff"),
  ("tiff", "image/tiff"),
  ("toml", "application/toml"),
  ("ts", "text/typescript"),
  ("ttf", "font/ttf"),
  ("txt", "text/plain"),
  ("usdz", "model/vnd.usdz+zip"),
  ("vsd", "application/vnd.visio"),
  ("vtt", "text/vtt"),
  ("wasm", "application/wasm"),
  ("wav", "audio/wav"),
  ("weba", "audio/webm"),
  ("webm", "video/webm"),
  ("webmanifest", "application/manifest+json"),
  ("webp", "image/webp"),
  ("woff", "font/woff"),
  ("woff2", "font/woff2"),
  ("xhtml", "application/xhtml+xml"),
  ("xls", "application/vnd.ms-excel"),
  (
    "xlsx",
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
  ),
  ("xml", "application/xml"),
  ("xul", "application/vnd.mozilla.xul+xml"),
  ("yaml", "application/yaml"),
  ("yml", "application/yaml"),
  ("zip", "application/zip"),
  ("3gp", "video/3gpp"),
  ("3g2", "video/3gpp2"),
  ("7z", "application/x-7z-compressed"),
];

#[test]
fn every_extension_of_the_typescript_tables_names_the_same_type_first() {
  for (extension, mime) in EXTENSIONS {
    let first = media_types_of(extension).first().map(|found| found.mime());
    assert_eq!(first, Some(mime), "{extension}");
  }
}

#[test]
fn three_extensions_differ_from_the_typescript_tables_on_purpose() {
  // The string the workspace already emits, where that package put the registered one first.
  assert_eq!(media_types_of("ico"), [MediaType::Ico]);
  assert_eq!(MediaType::Ico.aliases(), ["image/vnd.microsoft.icon"]);
  // The registered string, where that package put the one it replaced first.
  assert_eq!(media_types_of("ttf"), [MediaType::Ttf]);
  assert!(MediaType::Ttf.aliases().contains(&"application/font-sfnt"));
  // `.mid` and `.midi` are one type, where that package gave each its own string.
  assert_eq!(media_types_of("mid"), media_types_of("midi"));
  assert_eq!(MediaType::Midi.aliases(), ["audio/x-midi"]);
}

#[test]
fn an_extension_in_use_for_two_types_lists_both_with_the_usual_one_first() {
  assert_eq!(
    media_types_of("ts"),
    [MediaType::TypeScript, MediaType::MpegTs]
  );
  assert_eq!(
    media_types_of("js"),
    [MediaType::JavaScript, MediaType::Node]
  );
  assert_eq!(media_types_of("json5"), [MediaType::Json, MediaType::Json5]);
}

#[test]
fn the_extensions_of_models_name_their_types() {
  let expected = [
    ("glb", MediaType::Glb),
    ("gltf", MediaType::Gltf),
    ("obj", MediaType::ModelObj),
    ("usdz", MediaType::Usdz),
    ("usd", MediaType::Usd),
    ("usdc", MediaType::Usd),
    ("usda", MediaType::Usda),
    ("igs", MediaType::Iges),
    ("iges", MediaType::Iges),
    ("msh", MediaType::Mesh),
    ("mesh", MediaType::Mesh),
    ("silo", MediaType::Mesh),
    ("stl", MediaType::Stl),
    ("3mf", MediaType::ThreeMf),
    ("mtl", MediaType::Mtl),
    ("wrl", MediaType::Vrml),
    ("vrml", MediaType::Vrml),
    ("step", MediaType::Step),
    ("stp", MediaType::Step),
    ("p21", MediaType::Step),
    ("dae", MediaType::Collada),
    ("x3d", MediaType::X3d),
  ];
  for (extension, media_type) in expected {
    assert_eq!(media_types_of(extension), [media_type], "{extension}");
  }
  assert_eq!(extensions_of("model/vnd.pixar.usd"), ["usd", "usdc"]);
  assert_eq!(
    media_type_of_path("models/chair.usdz"),
    Some(MediaType::Usdz)
  );
}

#[test]
fn case_and_a_leading_dot_do_not_change_what_an_extension_names() {
  assert_eq!(media_types_of(".PNG"), [MediaType::Png]);
  assert_eq!(media_types_of("Jpeg"), [MediaType::Jpeg]);
}

#[test]
fn an_extension_in_no_table_names_nothing() {
  assert_eq!(media_types_of("nope"), []);
  assert_eq!(media_types_of(""), []);
  assert_eq!(media_types_of("."), []);
}

#[test]
fn a_path_is_read_for_its_last_extension() {
  let expected = [
    ("photo.JPG", Some(MediaType::Jpeg)),
    ("a/b/report.final.pdf", Some(MediaType::Pdf)),
    ("backup.tar.gz", Some(MediaType::Gzip)),
    (r"C:\scans\invoice.TIFF", Some(MediaType::Tiff)),
    // What follows `?` or `#` in an address is not part of the name.
    (
      "https://cdn.example.com/media/clip.mp4?sig=a.b#t=1.5",
      Some(MediaType::Mp4),
    ),
    ("https://example.com/download?file=a.png", None),
    // A dot in a folder's name is not the file's.
    ("release.d/notes", None),
    ("notes", None),
    ("notes.", None),
    ("archive.nope", None),
    ("", None),
  ];
  for (path, media_type) in expected {
    assert_eq!(media_type_of_path(path), media_type, "{path}");
  }
}

#[test]
fn the_extensions_of_a_type_are_found_from_any_string_in_use_for_it() {
  assert_eq!(
    extensions_of("image/jpeg"),
    ["jpg", "jpeg", "jpe", "jif", "jfif", "pjp", "pjpeg"]
  );
  assert_eq!(extensions_of("audio/ogg"), ["ogg", "oga", "opus"]);
  assert_eq!(extensions_of("application/x-gzip"), ["gz"]);
  assert_eq!(extensions_of("Video/WebM"), ["webm"]);
  let none: [&str; 0] = [];
  assert_eq!(extensions_of("application/x-nothing"), none);
}

#[test]
fn a_local_name_may_hold_the_marks_an_address_ends_at() {
  assert_eq!(media_type_of_path("report #3.pdf"), Some(MediaType::Pdf));
  assert_eq!(media_type_of_path("why?.txt"), Some(MediaType::Txt));
  assert_eq!(
    media_type_of_path("drafts #2/letter.docx"),
    Some(MediaType::Docx)
  );
  // An address is still read up to its query, whatever the query holds.
  assert_eq!(
    media_type_of_path("https://example.com/a/b.png?next=/c/d.zip#top"),
    Some(MediaType::Png)
  );
}
