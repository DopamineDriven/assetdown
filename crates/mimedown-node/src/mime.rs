//! Every type the library can name, as the string it is known by. The list mirrors the
//! library's own table: a type added there does not compile here until it is added too.

use napi_derive::napi;

/// A media type, as its string: `image/png`.
#[napi(string_enum)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mime {
  #[napi(value = "image/png")]
  Png,
  #[napi(value = "image/apng")]
  Apng,
  #[napi(value = "image/jpeg")]
  Jpeg,
  #[napi(value = "image/gif")]
  Gif,
  #[napi(value = "image/bmp")]
  Bmp,
  #[napi(value = "image/webp")]
  Webp,
  #[napi(value = "image/avif")]
  Avif,
  #[napi(value = "image/heic")]
  Heic,
  #[napi(value = "image/heif")]
  Heif,
  #[napi(value = "image/avci")]
  Avci,
  #[napi(value = "image/svg+xml")]
  Svg,
  #[napi(value = "image/x-icon")]
  Ico,
  #[napi(value = "image/tiff")]
  Tiff,
  #[napi(value = "image/vnd.radiance")]
  Hdr,
  #[napi(value = "image/aces")]
  Aces,
  #[napi(value = "image/dpx")]
  Dpx,
  #[napi(value = "image/emf")]
  Emf,
  #[napi(value = "image/ktx")]
  Ktx,
  #[napi(value = "image/ktx2")]
  Ktx2,
  #[napi(value = "image/jxl")]
  Jxl,
  #[napi(value = "image/jp2")]
  Jp2,
  #[napi(value = "image/jpx")]
  Jpx,
  #[napi(value = "image/jpm")]
  Jpm,
  #[napi(value = "image/jph")]
  Jph,
  #[napi(value = "image/j2c")]
  J2c,
  #[napi(value = "image/jxr")]
  Jxr,
  #[napi(value = "image/jls")]
  Jls,
  #[napi(value = "image/x-adobe-dng")]
  Dng,
  #[napi(value = "image/x-canon-cr2")]
  Cr2,
  #[napi(value = "image/x-canon-cr3")]
  Cr3,
  #[napi(value = "image/x-nikon-nef")]
  Nef,
  #[napi(value = "image/x-sony-arw")]
  Arw,
  #[napi(value = "image/x-fuji-raf")]
  Raf,
  #[napi(value = "image/x-olympus-orf")]
  Orf,
  #[napi(value = "image/x-panasonic-rw2")]
  Rw2,
  #[napi(value = "image/x-dcraw")]
  Raw,
  #[napi(value = "application/pdf")]
  Pdf,
  #[napi(value = "application/vnd.openxmlformats-officedocument.wordprocessingml.document")]
  Docx,
  #[napi(value = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet")]
  Xlsx,
  #[napi(value = "application/vnd.openxmlformats-officedocument.presentationml.presentation")]
  Pptx,
  #[napi(value = "application/vnd.ms-word.document.macroenabled.12")]
  Docm,
  #[napi(value = "application/vnd.openxmlformats-officedocument.wordprocessingml.template")]
  Dotx,
  #[napi(value = "application/vnd.ms-excel.sheet.macroenabled.12")]
  Xlsm,
  #[napi(value = "application/vnd.openxmlformats-officedocument.spreadsheetml.template")]
  Xltx,
  #[napi(value = "application/vnd.ms-powerpoint.presentation.macroenabled.12")]
  Pptm,
  #[napi(value = "application/vnd.openxmlformats-officedocument.presentationml.template")]
  Potx,
  #[napi(value = "application/vnd.openxmlformats-officedocument.presentationml.slideshow")]
  Ppsx,
  #[napi(value = "application/msword")]
  Doc,
  #[napi(value = "application/vnd.ms-excel")]
  Xls,
  #[napi(value = "application/vnd.ms-powerpoint")]
  Ppt,
  #[napi(value = "application/vnd.visio")]
  Vsd,
  #[napi(value = "application/vnd.oasis.opendocument.text")]
  Odt,
  #[napi(value = "application/vnd.oasis.opendocument.spreadsheet")]
  Ods,
  #[napi(value = "application/vnd.oasis.opendocument.presentation")]
  Odp,
  #[napi(value = "application/epub+zip")]
  Epub,
  #[napi(value = "application/vnd.amazon.ebook")]
  Azw,
  #[napi(value = "application/x-abiword")]
  Abw,
  #[napi(value = "application/rtf")]
  Rtf,
  #[napi(value = "application/postscript")]
  Postscript,
  #[napi(value = "text/plain")]
  Txt,
  #[napi(value = "text/markdown")]
  Markdown,
  #[napi(value = "application/x-mdx")]
  Mdx,
  #[napi(value = "text/csv")]
  Csv,
  #[napi(value = "text/html")]
  Html,
  #[napi(value = "application/xhtml+xml")]
  Xhtml,
  #[napi(value = "application/xml")]
  Xml,
  #[napi(value = "text/css")]
  Css,
  #[napi(value = "text/javascript")]
  JavaScript,
  #[napi(value = "application/node")]
  Node,
  #[napi(value = "text/typescript")]
  TypeScript,
  #[napi(value = "application/json")]
  Json,
  #[napi(value = "application/jsonc")]
  Jsonc,
  #[napi(value = "application/json5")]
  Json5,
  #[napi(value = "application/jsonl")]
  Jsonl,
  #[napi(value = "application/x-ndjson")]
  Ndjson,
  #[napi(value = "application/ld+json")]
  JsonLd,
  #[napi(value = "application/manifest+json")]
  WebManifest,
  #[napi(value = "application/yaml")]
  Yaml,
  #[napi(value = "application/toml")]
  Toml,
  #[napi(value = "application/sql")]
  Sql,
  #[napi(value = "application/x-sh")]
  Shell,
  #[napi(value = "application/x-csh")]
  CShell,
  #[napi(value = "text/x-c")]
  C,
  #[napi(value = "text/x-c++")]
  Cpp,
  #[napi(value = "text/x-csharp")]
  CSharp,
  #[napi(value = "text/x-java")]
  Java,
  #[napi(value = "text/x-php")]
  Php,
  #[napi(value = "text/x-python")]
  Python,
  #[napi(value = "text/x-ruby")]
  Ruby,
  #[napi(value = "text/rust")]
  Rust,
  #[napi(value = "text/x-tex")]
  Tex,
  #[napi(value = "text/calendar")]
  Calendar,
  #[napi(value = "text/vcard")]
  Vcard,
  #[napi(value = "text/vtt")]
  Vtt,
  #[napi(value = "text/event-stream")]
  EventStream,
  #[napi(value = "application/vnd.apple.mpegurl")]
  HlsPlaylist,
  #[napi(value = "application/dash+xml")]
  DashManifest,
  #[napi(value = "application/vnd.mozilla.xul+xml")]
  Xul,
  #[napi(value = "audio/mpeg")]
  Mp3,
  #[napi(value = "audio/wav")]
  Wav,
  #[napi(value = "audio/flac")]
  Flac,
  #[napi(value = "audio/ogg")]
  OggAudio,
  #[napi(value = "audio/aac")]
  Aac,
  #[napi(value = "audio/mp4")]
  M4a,
  #[napi(value = "audio/webm")]
  WebmAudio,
  #[napi(value = "audio/midi")]
  Midi,
  #[napi(value = "video/mp4")]
  Mp4,
  #[napi(value = "video/quicktime")]
  Mov,
  #[napi(value = "video/webm")]
  Webm,
  #[napi(value = "video/x-matroska")]
  Mkv,
  #[napi(value = "video/x-msvideo")]
  Avi,
  #[napi(value = "video/mpeg")]
  Mpeg,
  #[napi(value = "video/mp2t")]
  MpegTs,
  #[napi(value = "video/ogg")]
  OggVideo,
  #[napi(value = "video/3gpp")]
  ThreeGp,
  #[napi(value = "video/3gpp2")]
  ThreeG2,
  #[napi(value = "application/zip")]
  Zip,
  #[napi(value = "application/gzip")]
  Gzip,
  #[napi(value = "application/x-bzip")]
  Bzip,
  #[napi(value = "application/x-bzip2")]
  Bzip2,
  #[napi(value = "application/x-tar")]
  Tar,
  #[napi(value = "application/x-7z-compressed")]
  SevenZip,
  #[napi(value = "application/vnd.rar")]
  Rar,
  #[napi(value = "application/java-archive")]
  Jar,
  #[napi(value = "application/vnd.apple.pkpass")]
  Pkpass,
  #[napi(value = "application/x-freearc")]
  FreeArc,
  #[napi(value = "application/x-cfb")]
  Cfb,
  #[napi(value = "font/woff")]
  Woff,
  #[napi(value = "font/woff2")]
  Woff2,
  #[napi(value = "font/otf")]
  Otf,
  #[napi(value = "font/ttf")]
  Ttf,
  #[napi(value = "font/collection")]
  Ttc,
  #[napi(value = "application/vnd.ms-fontobject")]
  Eot,
  #[napi(value = "model/gltf-binary")]
  Glb,
  #[napi(value = "model/gltf+json")]
  Gltf,
  #[napi(value = "model/obj")]
  ModelObj,
  #[napi(value = "model/mtl")]
  Mtl,
  #[napi(value = "model/vnd.usdz+zip")]
  Usdz,
  #[napi(value = "model/vnd.pixar.usd")]
  Usd,
  #[napi(value = "model/vnd.usda")]
  Usda,
  #[napi(value = "model/stl")]
  Stl,
  #[napi(value = "model/3mf")]
  ThreeMf,
  #[napi(value = "model/iges")]
  Iges,
  #[napi(value = "model/step")]
  Step,
  #[napi(value = "model/mesh")]
  Mesh,
  #[napi(value = "model/vrml")]
  Vrml,
  #[napi(value = "model/x3d+xml")]
  X3d,
  #[napi(value = "model/vnd.collada+xml")]
  Collada,
  #[napi(value = "haptics/hjif")]
  Hjif,
  #[napi(value = "haptics/hmpg")]
  Hmpg,
  #[napi(value = "haptics/ivs")]
  Ivs,
  #[napi(value = "application/ogg")]
  Ogg,
  #[napi(value = "application/wasm")]
  Wasm,
  #[napi(value = "application/x-cdf")]
  Cda,
  #[napi(value = "application/x-python-code")]
  PythonBytecode,
  #[napi(value = "application/vnd.apple.installer+xml")]
  InstallerPackage,
  #[napi(value = "application/x-msi")]
  Msi,
  #[napi(value = "application/octet-stream")]
  OctetStream,
}

impl From<mimedown::MediaType> for Mime {
  fn from(media_type: mimedown::MediaType) -> Self {
    match media_type {
      mimedown::MediaType::Png => Mime::Png,
      mimedown::MediaType::Apng => Mime::Apng,
      mimedown::MediaType::Jpeg => Mime::Jpeg,
      mimedown::MediaType::Gif => Mime::Gif,
      mimedown::MediaType::Bmp => Mime::Bmp,
      mimedown::MediaType::Webp => Mime::Webp,
      mimedown::MediaType::Avif => Mime::Avif,
      mimedown::MediaType::Heic => Mime::Heic,
      mimedown::MediaType::Heif => Mime::Heif,
      mimedown::MediaType::Avci => Mime::Avci,
      mimedown::MediaType::Svg => Mime::Svg,
      mimedown::MediaType::Ico => Mime::Ico,
      mimedown::MediaType::Tiff => Mime::Tiff,
      mimedown::MediaType::Hdr => Mime::Hdr,
      mimedown::MediaType::Aces => Mime::Aces,
      mimedown::MediaType::Dpx => Mime::Dpx,
      mimedown::MediaType::Emf => Mime::Emf,
      mimedown::MediaType::Ktx => Mime::Ktx,
      mimedown::MediaType::Ktx2 => Mime::Ktx2,
      mimedown::MediaType::Jxl => Mime::Jxl,
      mimedown::MediaType::Jp2 => Mime::Jp2,
      mimedown::MediaType::Jpx => Mime::Jpx,
      mimedown::MediaType::Jpm => Mime::Jpm,
      mimedown::MediaType::Jph => Mime::Jph,
      mimedown::MediaType::J2c => Mime::J2c,
      mimedown::MediaType::Jxr => Mime::Jxr,
      mimedown::MediaType::Jls => Mime::Jls,
      mimedown::MediaType::Dng => Mime::Dng,
      mimedown::MediaType::Cr2 => Mime::Cr2,
      mimedown::MediaType::Cr3 => Mime::Cr3,
      mimedown::MediaType::Nef => Mime::Nef,
      mimedown::MediaType::Arw => Mime::Arw,
      mimedown::MediaType::Raf => Mime::Raf,
      mimedown::MediaType::Orf => Mime::Orf,
      mimedown::MediaType::Rw2 => Mime::Rw2,
      mimedown::MediaType::Raw => Mime::Raw,
      mimedown::MediaType::Pdf => Mime::Pdf,
      mimedown::MediaType::Docx => Mime::Docx,
      mimedown::MediaType::Xlsx => Mime::Xlsx,
      mimedown::MediaType::Pptx => Mime::Pptx,
      mimedown::MediaType::Docm => Mime::Docm,
      mimedown::MediaType::Dotx => Mime::Dotx,
      mimedown::MediaType::Xlsm => Mime::Xlsm,
      mimedown::MediaType::Xltx => Mime::Xltx,
      mimedown::MediaType::Pptm => Mime::Pptm,
      mimedown::MediaType::Potx => Mime::Potx,
      mimedown::MediaType::Ppsx => Mime::Ppsx,
      mimedown::MediaType::Doc => Mime::Doc,
      mimedown::MediaType::Xls => Mime::Xls,
      mimedown::MediaType::Ppt => Mime::Ppt,
      mimedown::MediaType::Vsd => Mime::Vsd,
      mimedown::MediaType::Odt => Mime::Odt,
      mimedown::MediaType::Ods => Mime::Ods,
      mimedown::MediaType::Odp => Mime::Odp,
      mimedown::MediaType::Epub => Mime::Epub,
      mimedown::MediaType::Azw => Mime::Azw,
      mimedown::MediaType::Abw => Mime::Abw,
      mimedown::MediaType::Rtf => Mime::Rtf,
      mimedown::MediaType::Postscript => Mime::Postscript,
      mimedown::MediaType::Txt => Mime::Txt,
      mimedown::MediaType::Markdown => Mime::Markdown,
      mimedown::MediaType::Mdx => Mime::Mdx,
      mimedown::MediaType::Csv => Mime::Csv,
      mimedown::MediaType::Html => Mime::Html,
      mimedown::MediaType::Xhtml => Mime::Xhtml,
      mimedown::MediaType::Xml => Mime::Xml,
      mimedown::MediaType::Css => Mime::Css,
      mimedown::MediaType::JavaScript => Mime::JavaScript,
      mimedown::MediaType::Node => Mime::Node,
      mimedown::MediaType::TypeScript => Mime::TypeScript,
      mimedown::MediaType::Json => Mime::Json,
      mimedown::MediaType::Jsonc => Mime::Jsonc,
      mimedown::MediaType::Json5 => Mime::Json5,
      mimedown::MediaType::Jsonl => Mime::Jsonl,
      mimedown::MediaType::Ndjson => Mime::Ndjson,
      mimedown::MediaType::JsonLd => Mime::JsonLd,
      mimedown::MediaType::WebManifest => Mime::WebManifest,
      mimedown::MediaType::Yaml => Mime::Yaml,
      mimedown::MediaType::Toml => Mime::Toml,
      mimedown::MediaType::Sql => Mime::Sql,
      mimedown::MediaType::Shell => Mime::Shell,
      mimedown::MediaType::CShell => Mime::CShell,
      mimedown::MediaType::C => Mime::C,
      mimedown::MediaType::Cpp => Mime::Cpp,
      mimedown::MediaType::CSharp => Mime::CSharp,
      mimedown::MediaType::Java => Mime::Java,
      mimedown::MediaType::Php => Mime::Php,
      mimedown::MediaType::Python => Mime::Python,
      mimedown::MediaType::Ruby => Mime::Ruby,
      mimedown::MediaType::Rust => Mime::Rust,
      mimedown::MediaType::Tex => Mime::Tex,
      mimedown::MediaType::Calendar => Mime::Calendar,
      mimedown::MediaType::Vcard => Mime::Vcard,
      mimedown::MediaType::Vtt => Mime::Vtt,
      mimedown::MediaType::EventStream => Mime::EventStream,
      mimedown::MediaType::HlsPlaylist => Mime::HlsPlaylist,
      mimedown::MediaType::DashManifest => Mime::DashManifest,
      mimedown::MediaType::Xul => Mime::Xul,
      mimedown::MediaType::Mp3 => Mime::Mp3,
      mimedown::MediaType::Wav => Mime::Wav,
      mimedown::MediaType::Flac => Mime::Flac,
      mimedown::MediaType::OggAudio => Mime::OggAudio,
      mimedown::MediaType::Aac => Mime::Aac,
      mimedown::MediaType::M4a => Mime::M4a,
      mimedown::MediaType::WebmAudio => Mime::WebmAudio,
      mimedown::MediaType::Midi => Mime::Midi,
      mimedown::MediaType::Mp4 => Mime::Mp4,
      mimedown::MediaType::Mov => Mime::Mov,
      mimedown::MediaType::Webm => Mime::Webm,
      mimedown::MediaType::Mkv => Mime::Mkv,
      mimedown::MediaType::Avi => Mime::Avi,
      mimedown::MediaType::Mpeg => Mime::Mpeg,
      mimedown::MediaType::MpegTs => Mime::MpegTs,
      mimedown::MediaType::OggVideo => Mime::OggVideo,
      mimedown::MediaType::ThreeGp => Mime::ThreeGp,
      mimedown::MediaType::ThreeG2 => Mime::ThreeG2,
      mimedown::MediaType::Zip => Mime::Zip,
      mimedown::MediaType::Gzip => Mime::Gzip,
      mimedown::MediaType::Bzip => Mime::Bzip,
      mimedown::MediaType::Bzip2 => Mime::Bzip2,
      mimedown::MediaType::Tar => Mime::Tar,
      mimedown::MediaType::SevenZip => Mime::SevenZip,
      mimedown::MediaType::Rar => Mime::Rar,
      mimedown::MediaType::Jar => Mime::Jar,
      mimedown::MediaType::Pkpass => Mime::Pkpass,
      mimedown::MediaType::FreeArc => Mime::FreeArc,
      mimedown::MediaType::Cfb => Mime::Cfb,
      mimedown::MediaType::Woff => Mime::Woff,
      mimedown::MediaType::Woff2 => Mime::Woff2,
      mimedown::MediaType::Otf => Mime::Otf,
      mimedown::MediaType::Ttf => Mime::Ttf,
      mimedown::MediaType::Ttc => Mime::Ttc,
      mimedown::MediaType::Eot => Mime::Eot,
      mimedown::MediaType::Glb => Mime::Glb,
      mimedown::MediaType::Gltf => Mime::Gltf,
      mimedown::MediaType::ModelObj => Mime::ModelObj,
      mimedown::MediaType::Mtl => Mime::Mtl,
      mimedown::MediaType::Usdz => Mime::Usdz,
      mimedown::MediaType::Usd => Mime::Usd,
      mimedown::MediaType::Usda => Mime::Usda,
      mimedown::MediaType::Stl => Mime::Stl,
      mimedown::MediaType::ThreeMf => Mime::ThreeMf,
      mimedown::MediaType::Iges => Mime::Iges,
      mimedown::MediaType::Step => Mime::Step,
      mimedown::MediaType::Mesh => Mime::Mesh,
      mimedown::MediaType::Vrml => Mime::Vrml,
      mimedown::MediaType::X3d => Mime::X3d,
      mimedown::MediaType::Collada => Mime::Collada,
      mimedown::MediaType::Hjif => Mime::Hjif,
      mimedown::MediaType::Hmpg => Mime::Hmpg,
      mimedown::MediaType::Ivs => Mime::Ivs,
      mimedown::MediaType::Ogg => Mime::Ogg,
      mimedown::MediaType::Wasm => Mime::Wasm,
      mimedown::MediaType::Cda => Mime::Cda,
      mimedown::MediaType::PythonBytecode => Mime::PythonBytecode,
      mimedown::MediaType::InstallerPackage => Mime::InstallerPackage,
      mimedown::MediaType::Msi => Mime::Msi,
      mimedown::MediaType::OctetStream => Mime::OctetStream,
    }
  }
}
