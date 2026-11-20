//! Every type that can be named: its string, the other strings in use for it, its file
//! extensions, and the workspace package that reads it.
//!
//! A type is described by what the registry of media types says of it and by nothing else.
//! Whether a file counts as a document, an archive or neither is for whoever handles it to
//! say: `application` holds a PDF, a zip and WebAssembly alike.
//!
//! The types are a closed set, so they are an enum. The table below is the one place a type is
//! described; the lookups in `tables` are all derived from it.

/// The first half of a type's string, as the registry of media types has it: `application` in
/// `application/pdf`. It is read off the string and is never a judgment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TopLevel {
  Application,
  Audio,
  Font,
  Haptics,
  Image,
  Model,
  Text,
  Video,
}

impl TopLevel {
  pub fn as_str(self) -> &'static str {
    match self {
      TopLevel::Application => "application",
      TopLevel::Audio => "audio",
      TopLevel::Font => "font",
      TopLevel::Haptics => "haptics",
      TopLevel::Image => "image",
      TopLevel::Model => "model",
      TopLevel::Text => "text",
      TopLevel::Video => "video",
    }
  }
}

/// A workspace package that reads a type in full.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Parser {
  Pdfdown,
  Xlsxdown,
  Docxdown,
  Pptxdown,
  Audiodown,
  Videodown,
  Tiffdown,
  Jpegdown,
  Rtfdown,
}

impl Parser {
  pub fn as_str(self) -> &'static str {
    match self {
      Parser::Pdfdown => "pdfdown",
      Parser::Xlsxdown => "xlsxdown",
      Parser::Docxdown => "docxdown",
      Parser::Pptxdown => "pptxdown",
      Parser::Audiodown => "audiodown",
      Parser::Videodown => "videodown",
      Parser::Tiffdown => "tiffdown",
      Parser::Jpegdown => "jpegdown",
      Parser::Rtfdown => "rtfdown",
    }
  }
}

/// One row for each type: `Name => "its string", ["others in use"], ["extensions"]`, and the
/// package that reads it when there is one. The first extension is the usual one.
macro_rules! media_types {
  ($(
    $variant:ident => $mime:literal, [$($alias:literal),*], [$($ext:literal),*]
      $(, $parser:ident)?;
  )*) => {
    /// A type a file can be named as.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub enum MediaType {
      $($variant),*
    }

    impl MediaType {
      /// Every type, in the order of the table. Where an extension is used for two types, the
      /// one that stands first here is the usual one.
      pub const ALL: &'static [MediaType] = &[$(MediaType::$variant),*];

      /// The type's string: the one the workspace emits where several are in use.
      pub fn mime(self) -> &'static str {
        match self {
          $(MediaType::$variant => $mime),*
        }
      }

      /// The other strings in use for the type. They are accepted, never returned.
      pub fn aliases(self) -> &'static [&'static str] {
        match self {
          $(MediaType::$variant => &[$($alias),*]),*
        }
      }

      /// The file extensions in use for the type, the usual one first, in lower case and
      /// without the dot.
      pub fn extensions(self) -> &'static [&'static str] {
        match self {
          $(MediaType::$variant => &[$($ext),*]),*
        }
      }

      /// The workspace package that reads the type in full.
      pub fn parser(self) -> Option<Parser> {
        match self {
          $(MediaType::$variant => media_types!(@parser $($parser)?)),*
        }
      }
    }
  };
  (@parser) => { None };
  (@parser $parser:ident) => { Some(Parser::$parser) };
}

media_types! {
  // Images.
  Png => "image/png", [], ["png"];
  Apng => "image/apng", [], ["apng"];
  Jpeg => "image/jpeg", [], ["jpg", "jpeg", "jpe", "jif", "jfif", "pjp", "pjpeg"], Jpegdown;
  Gif => "image/gif", [], ["gif"];
  Bmp => "image/bmp", [], ["bmp"];
  Webp => "image/webp", [], ["webp"];
  Avif => "image/avif", [], ["avif"];
  Heic => "image/heic", [], ["heic"];
  Heif => "image/heif", [], ["heif"];
  Avci => "image/avci", [], ["avci"];
  Svg => "image/svg+xml", [], ["svg"];
  Ico => "image/x-icon", ["image/vnd.microsoft.icon"], ["ico", "cur"];
  Tiff => "image/tiff", [], ["tiff", "tif"], Tiffdown;
  Hdr => "image/vnd.radiance", [], ["hdr", "rgbe", "xyze", "pic"];
  Aces => "image/aces", [], ["aces"];
  Dpx => "image/dpx", [], ["dpx"];
  Emf => "image/emf", [], ["emf"];
  Ktx => "image/ktx", [], ["ktx"];
  Ktx2 => "image/ktx2", [], ["ktx2"];
  Jxl => "image/jxl", [], ["jxl"];
  // JPEG 2000: the file format, its extended form, the compound form, the one that holds the
  // high-throughput coding, and the codestream with no boxes around it.
  Jp2 => "image/jp2", [], ["jp2", "jpg2"];
  Jpx => "image/jpx", [], ["jpx", "jpf"];
  Jpm => "image/jpm", [], ["jpm"];
  Jph => "image/jph", [], ["jph"];
  J2c => "image/j2c", ["image/x-jp2-codestream"], ["j2c", "j2k", "jpc"];
  Jxr => "image/jxr", ["image/vnd.ms-photo"], ["jxr", "wdp", "hdp"];
  Jls => "image/jls", [], ["jls"];
  // What a camera's sensor recorded. None of these strings is registered; they are the ones
  // the desktop's shared database of types uses.
  Dng => "image/x-adobe-dng", [], ["dng"];
  Cr2 => "image/x-canon-cr2", [], ["cr2"];
  Cr3 => "image/x-canon-cr3", [], ["cr3"];
  Nef => "image/x-nikon-nef", [], ["nef", "nrw"];
  Arw => "image/x-sony-arw", [], ["arw"];
  Raf => "image/x-fuji-raf", [], ["raf"];
  Orf => "image/x-olympus-orf", [], ["orf"];
  Rw2 => "image/x-panasonic-rw2", ["image/x-panasonic-raw2"], ["rw2"];
  // A sensor's recording in a TIFF by a maker that is not told apart.
  Raw => "image/x-dcraw", [], ["raw"];

  // What has pages, sheets or slides, and text of every kind, wherever the registry files it.
  Pdf => "application/pdf", [], ["pdf"], Pdfdown;
  Docx => "application/vnd.openxmlformats-officedocument.wordprocessingml.document", [], ["docx"], Docxdown;
  Xlsx => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet", [], ["xlsx"], Xlsxdown;
  Pptx => "application/vnd.openxmlformats-officedocument.presentationml.presentation", [], ["pptx"], Pptxdown;
  Docm => "application/vnd.ms-word.document.macroenabled.12", [], ["docm"], Docxdown;
  Dotx => "application/vnd.openxmlformats-officedocument.wordprocessingml.template", [], ["dotx"], Docxdown;
  Xlsm => "application/vnd.ms-excel.sheet.macroenabled.12", [], ["xlsm"], Xlsxdown;
  Xltx => "application/vnd.openxmlformats-officedocument.spreadsheetml.template", [], ["xltx"], Xlsxdown;
  Pptm => "application/vnd.ms-powerpoint.presentation.macroenabled.12", [], ["pptm"], Pptxdown;
  Potx => "application/vnd.openxmlformats-officedocument.presentationml.template", [], ["potx"], Pptxdown;
  Ppsx => "application/vnd.openxmlformats-officedocument.presentationml.slideshow", [], ["ppsx"], Pptxdown;
  Doc => "application/msword", [], ["doc"];
  Xls => "application/vnd.ms-excel", [], ["xls"];
  Ppt => "application/vnd.ms-powerpoint", [], ["ppt"];
  Vsd => "application/vnd.visio", [], ["vsd"];
  Odt => "application/vnd.oasis.opendocument.text", [], ["odt"];
  Ods => "application/vnd.oasis.opendocument.spreadsheet", [], ["ods"];
  Odp => "application/vnd.oasis.opendocument.presentation", [], ["odp"];
  Epub => "application/epub+zip", [], ["epub"];
  Azw => "application/vnd.amazon.ebook", [], ["azw"];
  Abw => "application/x-abiword", [], ["abw"];
  Rtf => "application/rtf", ["text/rtf"], ["rtf"], Rtfdown;
  Postscript => "application/postscript", [], ["ps", "eps"];
  Txt => "text/plain", [], ["txt"];
  Markdown => "text/markdown", ["application/text"], ["md"];
  Mdx => "application/x-mdx", [], ["mdx"];
  Csv => "text/csv", [], ["csv"];
  Html => "text/html", [], ["html", "htm"];
  Xhtml => "application/xhtml+xml", [], ["xhtml"];
  Xml => "application/xml", ["text/xml"], ["xml", "mtlx"];
  Css => "text/css", [], ["css"];
  JavaScript => "text/javascript", [], ["js", "mjs", "cjs"];
  Node => "application/node", [], ["node", "js", "cjs"];
  TypeScript => "text/typescript", ["application/typescript"], ["ts"];
  Json => "application/json", [], ["json", "jsonc", "json5"];
  Jsonc => "application/jsonc", [], ["jsonc"];
  Json5 => "application/json5", ["application/vnd.json5"], ["json5"];
  Jsonl => "application/jsonl", ["text/x-jsonl"], ["jsonl", "jsonlines"];
  Ndjson => "application/x-ndjson", [], ["ndjson"];
  JsonLd => "application/ld+json", [], ["jsonld"];
  WebManifest => "application/manifest+json", [], ["webmanifest"];
  Yaml => "application/yaml", [], ["yml", "yaml"];
  Toml => "application/toml", [], ["toml"];
  Sql => "application/sql", [], ["sql"];
  Shell => "application/x-sh", [], ["sh"];
  CShell => "application/x-csh", [], ["csh"];
  C => "text/x-c", [], ["c"];
  Cpp => "text/x-c++", [], ["cpp"];
  CSharp => "text/x-csharp", [], ["cs"];
  Java => "text/x-java", [], ["java"];
  Php => "text/x-php", ["application/x-httpd-php"], ["php"];
  Python => "text/x-python", ["text/x-script.python"], ["py"];
  Ruby => "text/x-ruby", [], ["rb"];
  Rust => "text/rust", [], ["rs"];
  Tex => "text/x-tex", [], ["tex"];
  Calendar => "text/calendar", [], ["ics"];
  Vcard => "text/vcard", ["text/x-vcard"], ["vcf"];
  Vtt => "text/vtt", [], ["vtt"];
  EventStream => "text/event-stream", [], ["sse"];
  HlsPlaylist => "application/vnd.apple.mpegurl", [], ["m3u8"];
  DashManifest => "application/dash+xml", [], ["mpd"];
  Xul => "application/vnd.mozilla.xul+xml", [], ["xul"];

  // Audio.
  Mp3 => "audio/mpeg", ["audio/mp3"], ["mp3"], Audiodown;
  Wav => "audio/wav", ["audio/x-wav", "audio/wave", "audio/vnd.wave"], ["wav"], Audiodown;
  Flac => "audio/flac", ["audio/x-flac"], ["flac"];
  OggAudio => "audio/ogg", [], ["ogg", "oga", "opus"];
  Aac => "audio/aac", [], ["aac"];
  M4a => "audio/mp4", ["audio/x-m4a"], ["m4a"];
  WebmAudio => "audio/webm", [], ["weba"];
  Midi => "audio/midi", ["audio/x-midi"], ["mid", "midi"];

  // Video.
  Mp4 => "video/mp4", [], ["mp4", "m4v", "m4s"], Videodown;
  Mov => "video/quicktime", [], ["mov", "qt"], Videodown;
  Webm => "video/webm", [], ["webm"], Videodown;
  Mkv => "video/x-matroska", ["video/matroska"], ["mkv"], Videodown;
  Avi => "video/x-msvideo", [], ["avi"];
  Mpeg => "video/mpeg", [], ["mpeg", "mpg"];
  MpegTs => "video/mp2t", ["video/vnd.dlna.mpeg-tts"], ["ts"];
  OggVideo => "video/ogg", [], ["ogv"];
  ThreeGp => "video/3gpp", [], ["3gp"];
  ThreeG2 => "video/3gpp2", [], ["3g2"];

  // Files that hold other files.
  Zip => "application/zip", ["application/x-zip-compressed"], ["zip"];
  Gzip => "application/gzip", ["application/x-gzip"], ["gz"];
  Bzip => "application/x-bzip", [], ["bz"];
  Bzip2 => "application/x-bzip2", [], ["bz2"];
  Tar => "application/x-tar", [], ["tar"];
  SevenZip => "application/x-7z-compressed", [], ["7z"];
  Rar => "application/vnd.rar", ["application/x-rar-compressed"], ["rar"];
  Jar => "application/java-archive", [], ["jar"];
  Pkpass => "application/vnd.apple.pkpass", [], ["pkpass"];
  FreeArc => "application/x-freearc", [], ["arc"];
  // A compound file that is none of the Office documents kept in one.
  Cfb => "application/x-cfb", ["application/x-ole-storage"], ["cfb"];

  // Fonts. The second strings are what they went by before `font/` was registered.
  Woff => "font/woff", ["application/font-woff"], ["woff"];
  Woff2 => "font/woff2", [], ["woff2"];
  Otf => "font/otf", ["application/vnd.ms-opentype", "application/x-font-otf"], ["otf"];
  Ttf => "font/ttf", ["application/font-sfnt", "application/x-font-ttf"], ["ttf"];
  Ttc => "font/collection", [], ["ttc"];
  Eot => "application/vnd.ms-fontobject", [], ["eot"];

  // 3D models and scenes.
  Glb => "model/gltf-binary", [], ["glb"];
  Gltf => "model/gltf+json", [], ["gltf"];
  ModelObj => "model/obj", [], ["obj"];
  Mtl => "model/mtl", [], ["mtl"];
  Usdz => "model/vnd.usdz+zip", [], ["usdz"];
  Usd => "model/vnd.pixar.usd", [], ["usd", "usdc"];
  Usda => "model/vnd.usda", [], ["usda"];
  Stl => "model/stl", [], ["stl"];
  ThreeMf => "model/3mf", [], ["3mf"];
  Iges => "model/iges", [], ["igs", "iges"];
  Step => "model/step", [], ["step", "stp", "p21", "stpnc"];
  Mesh => "model/mesh", [], ["msh", "mesh", "silo"];
  Vrml => "model/vrml", [], ["wrl", "vrml"];
  X3d => "model/x3d+xml", [], ["x3d"];
  Collada => "model/vnd.collada+xml", [], ["dae"];

  // Haptics.
  Hjif => "haptics/hjif", [], ["hjif"];
  Hmpg => "haptics/hmpg", [], ["hmpg"];
  Ivs => "haptics/ivs", [], ["ivs", "ivt"];

  // The rest of `application`.
  Ogg => "application/ogg", [], ["ogx"];
  Wasm => "application/wasm", [], ["wasm"];
  Cda => "application/x-cdf", [], ["cda"];
  PythonBytecode => "application/x-python-code", [], ["pyc"];
  InstallerPackage => "application/vnd.apple.installer+xml", [], ["mpkg"];
  Msi => "application/x-msi", [], ["msi"];

  // What bytes are called when they name no type.
  OctetStream => "application/octet-stream", [], ["bin", "pk1"];
}

impl MediaType {
  /// How the registry files the type: the word in front of the slash in its string.
  pub fn top_level(self) -> TopLevel {
    match self.mime().split_once('/') {
      Some(("audio", _)) => TopLevel::Audio,
      Some(("font", _)) => TopLevel::Font,
      Some(("haptics", _)) => TopLevel::Haptics,
      Some(("image", _)) => TopLevel::Image,
      Some(("model", _)) => TopLevel::Model,
      Some(("text", _)) => TopLevel::Text,
      Some(("video", _)) => TopLevel::Video,
      // Every other string in the table is filed under `application`, which a test holds it to.
      _ => TopLevel::Application,
    }
  }

  /// The usual extension for the type, in lower case and without the dot.
  pub fn ext(self) -> &'static str {
    // Every type has an extension; `bin` is what bytes without a name are called.
    self.extensions().first().copied().unwrap_or("bin")
  }

  /// The type a string names: its own, or another in use for it. Letter case, the space around
  /// it and any parameters after a `;` are ignored.
  pub fn from_mime(text: &str) -> Option<MediaType> {
    let essence = text.split(';').next().unwrap_or(text).trim();
    MediaType::ALL.iter().copied().find(|media_type| {
      std::iter::once(media_type.mime())
        .chain(media_type.aliases().iter().copied())
        .any(|string| string.eq_ignore_ascii_case(essence))
    })
  }
}

impl std::fmt::Display for MediaType {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(self.mime())
  }
}

#[cfg(test)]
mod tests;
