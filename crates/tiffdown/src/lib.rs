//! TIFF files, read: a scanned or faxed document is often one, with a page to a directory.
//!
//! [`Tiff::open`] takes the whole file and lists its pages with what each says of itself.
//! Nothing here opens a file, fetches one, or depends on Node.

mod directory;
mod error;
#[cfg(feature = "ocr")]
mod ocr;
mod raster;
#[cfg(test)]
mod testing;
mod tiff;

pub use error::TiffError;
#[cfg(feature = "ocr")]
pub use ocr::Recognized;
pub use tiff::{Compression, Page, Photometric, Resolution, Tiff};
