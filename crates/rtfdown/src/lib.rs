//! RTF: a word processor's export, or a mail message saved with what was attached to it.

mod body;
mod error;
mod layout;
mod rtf;
#[cfg(test)]
mod testing;
mod token;

pub use body::{Align, Block, DownFrom, Inline, Paper, Paragraph, Placing, Row, Run, Running};
pub use error::RtfError;
pub use rtf::{Embedded, Picture, PictureFormat, Rtf};
