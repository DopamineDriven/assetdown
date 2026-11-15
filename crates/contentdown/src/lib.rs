//! Structured content from pages of positioned text.
//!
//! A page here is a set of words with their boxes and the lines they form. A format crate turns
//! its own document into such pages (a PDF's glyphs, recognized text, a sheet laid out as a page)
//! and this crate reads them: `words` builds lines from words, `invoice` extracts invoice
//! fields from the pages or from e-invoice XML that travelled with the document, and `paper`
//! names the standard paper size a page is nearest to.
//!
//! Nothing here opens a file or knows a document format. It reads pages and returns data.

pub mod invoice;
mod layout;
mod paper;
mod words;

pub use layout::{PageLayout, PlacedImage, TextLine, TextSource, TextWord};
pub use paper::{PaperSize, paper_neighbors, paper_size};
pub use words::{FontMeta, Word, words_to_lines, words_to_lines_indexed};
