//! Word documents in the format Word wrote from 1997 to 2003: a `.doc` read into the model
//! of a flowed document that `contentdown` holds, so that it is set on pages, and an invoice
//! is read from them, by the code that does both for an RTF file.
//!
//! The file is a compound file (`cfbdown`). Its `WordDocument` stream opens with a block that
//! says where everything is (`fib`), and holds the text; a table stream holds what describes
//! the text. `pieces` reads the text in the order the piece table gives, `props` finds the
//! properties stated for any place in it, with the styles, the fonts, and the sections,
//! `body` reads paragraphs, table rows, and page breaks out of both, and `boxes` finds the
//! text of text boxes and where each is anchored. Nothing here depends on Node.

mod body;
mod boxes;
mod doc;
mod embedded;
mod error;
mod fib;
mod pictures;
mod pieces;
mod props;
#[cfg(test)]
mod testing;

pub use contentdown::document::{
  Align, Block, DownFrom, Inline, Paper, Paragraph, Placing, Row, Run, Running,
};
pub use doc::Doc;
pub use embedded::Embedded;
pub use error::DocError;
pub use pictures::{Picture, PictureFormat};

#[cfg(test)]
mod tests;
