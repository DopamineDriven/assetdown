//! The document set on pages. `contentdown` does the setting, for this crate and for any other
//! whose format records a flow of text and not where anything lands.

use contentdown::PageLayout;
use contentdown::document::{self, Body};

const ESTIMATED: &str =
  "word positions are estimated from the flow of the text; an RTF file records no layout";

/// The document's pages, in order. There is always at least one.
pub(crate) fn pages(body: &Body) -> Vec<PageLayout> {
  document::pages(body, ESTIMATED)
}

#[cfg(test)]
mod tests;
