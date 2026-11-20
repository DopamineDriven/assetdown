use crate::layout::page_layout;
use crate::types::{PageText, StructuredPageText};
use contentdown::PageLayout;
use lopdf::{Document, ObjectId};
use rayon::prelude::*;

/// The text of a page that has been laid out: its lines from the top of the page down, a line
/// of text to each, and the words of a line from the left.
pub fn text_of_layout(layout: &PageLayout) -> String {
  let lines: Vec<&str> = layout.lines.iter().map(|line| line.text.as_str()).collect();
  lines.join("\n")
}

/// The text of one page, read from where its glyphs are drawn, as [`page_layout`] reads them:
/// through the forms the page draws, with each font's own map and encoding, and with words and
/// lines told apart by position. `lopdf`'s own `extract_text` is not used: it reads no form,
/// drops the codes a font's map gives outside the range the map declares, and breaks words by
/// drawing operation.
pub fn page_text(doc: &Document, page_num: u32, page_id: ObjectId) -> String {
  text_of_layout(&page_layout(doc, page_num, page_id))
}

pub fn extract_text(doc: &Document) -> Vec<PageText> {
  let pages = doc.get_pages();
  let page_count_str = pages.len().to_string();
  let page_entries: Vec<(u32, ObjectId)> = pages.iter().map(|(&k, &v)| (k, v)).collect();
  let mut results: Vec<PageText> = page_entries
    .par_iter()
    .map(|&(page_num, page_id)| {
      let raw = page_text(doc, page_num, page_id);
      let text = strip_footer_artifacts(&raw, &page_count_str);
      PageText {
        page: page_num,
        text,
      }
    })
    .collect();
  results.sort_unstable_by_key(|p| p.page);
  results
}

/// Normalize a line for header/footer comparison: trim whitespace and replace
/// contiguous digit sequences with `<NUM>` so "Page 1" matches "Page 42".
fn normalize_header_footer_line(line: &str) -> String {
  let trimmed = line.trim();
  let mut out = String::with_capacity(trimmed.len());
  let mut in_digits = false;
  for ch in trimmed.chars() {
    if ch.is_ascii_digit() {
      if !in_digits {
        out.push_str("<NUM>");
        in_digits = true;
      }
    } else {
      in_digits = false;
      out.push(ch);
    }
  }
  out
}

/// Strip Chromium footer artifacts from extracted text.
///
/// Chromium's Skia PDF renderer writes page footers (e.g., `1 / 38`) as 2-3
/// separate text operations. `lopdf::extract_text` concatenates these in
/// content-stream order, causing orphaned fragments like ` / \n38\n` to appear
/// mid-text on every page. This function removes the known pattern: a line
/// containing just `/` followed by a line containing just the total page count.
pub fn strip_footer_artifacts(text: &str, page_count_str: &str) -> String {
  let lines: Vec<&str> = text.lines().collect();
  if lines.len() < 2 {
    return text.to_string();
  }
  let mut skip = vec![false; lines.len()];
  for i in 0..lines.len() - 1 {
    if lines[i].trim() == "/" && lines[i + 1].trim() == page_count_str {
      skip[i] = true;
      skip[i + 1] = true;
    }
  }
  if !skip.iter().any(|&s| s) {
    return text.to_string();
  }
  lines
    .iter()
    .zip(skip.iter())
    .filter(|&(_, &s)| !s)
    .map(|(&line, _)| line)
    .collect::<Vec<_>>()
    .join("\n")
}

/// Detect repeated header/footer lines across pages and split each page's text
/// into header, body, and footer sections.
pub fn detect_headers_footers(pages: &[PageText]) -> Vec<StructuredPageText> {
  // For fewer than 3 pages, no meaningful detection — return everything as body
  if pages.len() < 3 {
    return pages
      .iter()
      .map(|p| StructuredPageText {
        page: p.page,
        header: String::new(),
        body: p.text.clone(),
        footer: String::new(),
      })
      .collect();
  }

  let threshold = (pages.len() as f64 * 0.6).ceil() as usize;
  let max_check = 3usize; // check up to 3 lines from top/bottom

  // Split each page into lines
  let page_lines: Vec<Vec<&str>> = pages.iter().map(|p| p.text.lines().collect()).collect();

  // Detect header line count: for each position 0..max_check, check if the
  // normalized line at that position appears on >= threshold pages
  let mut header_count = 0usize;
  for pos in 0..max_check {
    let mut freq = std::collections::HashMap::<String, usize>::new();
    for lines in &page_lines {
      if let Some(&line) = lines.get(pos) {
        let norm = normalize_header_footer_line(line);
        if !norm.is_empty() {
          *freq.entry(norm).or_insert(0) += 1;
        }
      }
    }
    if freq.values().any(|&c| c >= threshold) {
      header_count = pos + 1;
    } else {
      break;
    }
  }

  // Detect footer line count (from the bottom)
  let mut footer_count = 0usize;
  for pos in 0..max_check {
    let mut freq = std::collections::HashMap::<String, usize>::new();
    for lines in &page_lines {
      if lines.len() > pos {
        let idx = lines.len() - 1 - pos;
        // Don't overlap with headers
        if idx >= header_count {
          let norm = normalize_header_footer_line(lines[idx]);
          if !norm.is_empty() {
            *freq.entry(norm).or_insert(0) += 1;
          }
        }
      }
    }
    if freq.values().any(|&c| c >= threshold) {
      footer_count = pos + 1;
    } else {
      break;
    }
  }

  pages
    .iter()
    .zip(page_lines.iter())
    .map(|(p, lines)| {
      let total = lines.len();
      let h_end = header_count.min(total);
      let f_start = if footer_count > 0 {
        total.saturating_sub(footer_count).max(h_end)
      } else {
        total
      };

      let header = lines[..h_end].join("\n");
      let body = lines[h_end..f_start].join("\n");
      let footer = lines[f_start..].join("\n");

      StructuredPageText {
        page: p.page,
        header,
        body,
        footer,
      }
    })
    .collect()
}

pub fn extract_structured_text(doc: &Document) -> Vec<StructuredPageText> {
  detect_headers_footers(&extract_text(doc))
}

#[cfg(test)]
mod tests;
