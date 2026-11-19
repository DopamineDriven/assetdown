//! What a file is, told from its bytes.
//!
//! [`sniff`] names a file's type from as few of its bytes as that takes: a signature at a fixed
//! place, or what a container says it holds. It is never told the type and it never guesses one: bytes that say nothing are [`Sniffed::Unknown`], which is an
//! answer and not an error. [`Source`] is how the bytes are handed over: the whole file, or
//! pieces of it with their offsets, in which case an answer can name the further range that
//! would settle it. `media_type` holds every type that can be named, `tables` goes between a
//! type and the file extensions in use for it, and `container` reads the boxes, chunks, entries
//! and streams that several types are kept in. Nothing here opens a file, fetches one or
//! depends on Node.

mod audio;
mod container;
mod document;
mod error;
mod image;
mod media_type;
mod sniff;
mod source;
mod specs;
mod tables;
#[cfg(test)]
mod testing;
mod text;
mod verbs;
mod video;

pub use audio::AudioFacts;
pub use container::icc::IccProfile;
pub use container::xmp::Xmp;
pub use document::{DocumentDetail, DocumentFacts, OfficeDetail, PdfDetail, RtfDetail, TextDetail};
pub use error::MimeError;
pub use image::{
  BmpDetail, ColorModel, ColorSpace, GifDetail, HdrDetail, HdrEncoding, HeifDetail, IcoDetail,
  IcoEntry, IcoKind, ImageDetail, ImageFacts, Jp2Detail, JpegDetail, JxlDetail, JxrDetail,
  PngDetail, PngText, PngTextKind, SvgDetail, TiffDetail, WebpDetail,
};
pub use media_type::{MediaType, Parser, TopLevel};
pub use sniff::{Evidence, Options, Retry, Sniffed, sniff, sniff_source, sniff_with};
pub use source::{ByteRange, Piece, Source};
pub use specs::{Facts, Specs, specs, specs_source, specs_with};
pub use tables::{extensions_of, media_type_of_path, media_types_of};
pub use verbs::{
  ChannelPeaks, Keyframes, Outcome, Poster, PosterFormat, PosterOptions, Resolution, Waveform,
  WaveformOptions, WhyNot, keyframes, poster, waveform,
};
pub use video::VideoFacts;

/// Which of the parsers the crate was built with. A type whose parser is not built in is still
/// named; its fuller facts, and the verbs that decode, are what is missing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltWith {
  pub pdf: bool,
  pub office: bool,
  pub audio: bool,
  pub video: bool,
}

pub fn built_with() -> BuiltWith {
  BuiltWith {
    pdf: cfg!(feature = "pdf"),
    office: cfg!(feature = "office"),
    audio: cfg!(feature = "audio"),
    video: cfg!(feature = "video"),
  }
}
