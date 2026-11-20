//! A JPEG that is a page: a scan, a fax, or a photograph of a sheet of paper.

mod error;
mod header;
mod jpeg;
#[cfg(feature = "ocr")]
mod ocr;
#[cfg(test)]
mod testing;

pub use error::JpegError;
pub use header::{Coding, Resolution};
pub use jpeg::{Color, Jpeg, Page, PageSize};
#[cfg(feature = "ocr")]
pub use ocr::Recognized;
