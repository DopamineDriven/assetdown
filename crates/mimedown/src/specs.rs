//! What a file is, and the facts its own header states.
//!
//! `specs` sniffs the file and hands it to the reader for its type. A type that has no reader
//! here is named and nothing more.

use crate::audio::AudioFacts;
use crate::document::{self, DocumentFacts};
use crate::image::{self, ImageFacts, Reading};
use crate::media_type::MediaType;
use crate::sniff::{Evidence, Options, Retry, Sniffed, located};
use crate::source::{ByteRange, Source};
use crate::video::VideoFacts;

/// The facts of a file, by the kind of facts its type has.
#[derive(Debug, Clone, PartialEq)]
pub enum Facts {
  /// The type is named and none of its facts are read here.
  None,
  Image(Box<ImageFacts>),
  Document(Box<DocumentFacts>),
  Audio(Box<AudioFacts>),
  Video(Box<VideoFacts>),
}

/// What a file is and what it says of itself.
#[derive(Debug, Clone, PartialEq)]
pub struct Specs {
  /// The type, which is `application/octet-stream` for bytes that name none.
  pub media_type: MediaType,
  /// What named the type. None for bytes that name none.
  pub evidence: Option<Evidence>,
  /// The second look that named it, when the first pass could not and one was asked for.
  pub retry: Option<Retry>,
  /// A range that would complete the answer, when the bytes given stop short of it.
  pub needs: Option<ByteRange>,
  /// What was wrong with the file, when it could still be read.
  pub warnings: Vec<String>,
  pub facts: Facts,
}

/// What the whole file in `bytes` is, and its facts.
pub fn specs(bytes: &[u8]) -> Specs {
  specs_source(&Source::whole(bytes))
}

/// The same, for a file of which `source` holds all or part.
pub fn specs_source(source: &Source<'_>) -> Specs {
  specs_with(source, Options::default())
}

/// The same, with what the caller knows of the file beside its bytes.
pub fn specs_with(source: &Source<'_>, options: Options<'_>) -> Specs {
  let (sniffed, leading) = located(source, options);
  let (media_type, evidence, needs, retry) = match sniffed {
    Sniffed::Named {
      media_type,
      evidence,
      needs,
      retry,
    } => (media_type, Some(evidence), needs, retry),
    Sniffed::Unknown { needs } => (MediaType::OctetStream, None, needs, None),
  };
  let mut reading = Reading::default();
  // A file found behind leading bytes is read from where it starts, and what its reader then
  // asks for is asked for by its place in the bytes that were given.
  let view = source.shifted(leading);
  let leniently = retry == Some(Retry::LenientText);
  let facts = read(media_type, evidence, leniently, &view, &mut reading);
  // What a reader lacked is asked for a block at a time: a header is followed by what it
  // heads, and asking for each few bytes in turn takes a fetch for every one.
  reading.needs = reading.needs.map(|range| view.block(range));
  reading.needs = reading.needs.map(|range| match range.from_end {
    true => range,
    false => ByteRange::forward(range.offset.saturating_add(leading), range.length),
  });
  Specs {
    media_type,
    evidence,
    retry,
    // What the name still needs and what the facts do are asked for in one range.
    needs: match (needs, reading.needs) {
      (Some(name), Some(facts)) => Some(name.spanning(facts)),
      (name, facts) => name.or(facts),
    },
    warnings: reading.warnings,
    facts,
  }
}

/// The facts of a file of this type, from the reader that knows the type.
fn read(
  media_type: MediaType,
  evidence: Option<Evidence>,
  leniently: bool,
  source: &Source<'_>,
  reading: &mut Reading,
) -> Facts {
  let image = match media_type {
    MediaType::Svg => image::svg::read(source, reading),
    // A document in RTF is one however it was found. Behind a byte-order mark or a blank
    // line it is the text step that names it, and it is read from where it opens.
    MediaType::Rtf => {
      let lead = document::rtf::lead(source);
      let document = document::rtf::read(&source.shifted(lead), reading);
      reading.needs = reading.needs.map(|range| match range.from_end {
        true => range,
        false => ByteRange::forward(range.offset.saturating_add(lead), range.length),
      });
      return document.map_or(Facts::None, |facts| Facts::Document(Box::new(facts)));
    }
    // Whatever else the text step named is text, and has a text's facts.
    _ if matches!(evidence, Some(Evidence::Content | Evidence::FileName)) => {
      let text = document::text::read(source, leniently, reading);
      return text.map_or(Facts::None, |facts| Facts::Document(Box::new(facts)));
    }
    MediaType::Png | MediaType::Apng => image::png::read(source, reading),
    MediaType::Gif => image::gif::read(source, reading),
    MediaType::Bmp => image::bmp::read(source, reading),
    MediaType::Ico => image::ico::read(source, reading),
    MediaType::Jpeg | MediaType::Jls => image::jpeg::read(source, reading),
    MediaType::Tiff => image::tiff::read(source, false, reading),
    MediaType::Dng | MediaType::Cr2 | MediaType::Nef | MediaType::Arw | MediaType::Raw => {
      image::tiff::read(source, true, reading)
    }
    MediaType::Webp => image::webp::read(source, reading),
    MediaType::Hdr => image::hdr::read(source, reading),
    MediaType::Jxl => image::jxl::read(source, reading),
    MediaType::Jxr => image::jxr::read(source, reading),
    MediaType::Jp2 | MediaType::Jpx | MediaType::Jpm | MediaType::Jph | MediaType::J2c => {
      image::jp2::read(source, reading)
    }
    MediaType::Avif | MediaType::Heic | MediaType::Heif | MediaType::Avci => {
      image::heif::read(source, reading)
    }
    MediaType::Pdf => {
      let document = document::pdf::read(source, reading);
      return document.map_or(Facts::None, |facts| Facts::Document(Box::new(facts)));
    }
    MediaType::Docx
    | MediaType::Docm
    | MediaType::Dotx
    | MediaType::Xlsx
    | MediaType::Xlsm
    | MediaType::Xltx
    | MediaType::Pptx
    | MediaType::Pptm
    | MediaType::Potx
    | MediaType::Ppsx => {
      let document = document::office::read(media_type, source, reading);
      return document.map_or(Facts::None, |facts| Facts::Document(Box::new(facts)));
    }
    #[cfg(feature = "video")]
    MediaType::Mp4 | MediaType::Mov | MediaType::Webm | MediaType::Mkv => {
      let video = crate::video::read(source, reading);
      return video.map_or(Facts::None, |facts| Facts::Video(Box::new(facts)));
    }
    #[cfg(feature = "audio")]
    MediaType::Mp3 | MediaType::Wav => {
      let audio = crate::audio::read(source, reading);
      return audio.map_or(Facts::None, |facts| Facts::Audio(Box::new(facts)));
    }
    _ => None,
  };
  image.map_or(Facts::None, |facts| Facts::Image(Box::new(facts)))
}

#[cfg(test)]
mod exchange_tests;
#[cfg(test)]
mod tests;
