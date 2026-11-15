//! The whole document in one pass, with its text read through the OCR cascade.

use std::collections::HashSet;

use lopdf::Document;
use pdfdown::document::extract_annotations;
use pdfdown::forms::{extract_forms, form_field_pages};
use pdfdown::images::extract_images_raw;
use pdfdown::meta::extract_metadata_with_form;

use crate::ocr::{detect_headers_footers_ocr, extract_text_with_ocr};
use crate::types::PdfDocumentOcr;

pub fn extract_all_with_ocr(
  doc: &Document,
  pdf_bytes: &[u8],
  lang: &str,
  min_len: u32,
  max_threads: u32,
  render_dpi: u32,
  render_mode: u8,
) -> PdfDocumentOcr {
  let (text, ((images, annotations), forms)) = rayon::join(
    || {
      extract_text_with_ocr(
        doc,
        pdf_bytes,
        lang,
        min_len,
        max_threads,
        render_dpi,
        render_mode,
      )
    },
    || {
      rayon::join(
        || rayon::join(|| extract_images_raw(doc), || extract_annotations(doc)),
        || extract_forms(doc),
      )
    },
  );
  let meta = extract_metadata_with_form(doc, forms.info);
  let structured_text = detect_headers_footers_ocr(&text);

  let mut image_pages: Vec<u32> = images
    .iter()
    .map(|i| i.page)
    .collect::<HashSet<_>>()
    .into_iter()
    .collect();
  image_pages.sort_unstable();

  let mut annotation_pages: Vec<u32> = annotations
    .iter()
    .map(|a| a.page)
    .collect::<HashSet<_>>()
    .into_iter()
    .collect();
  annotation_pages.sort_unstable();

  let form_field_pages = form_field_pages(&forms.fields);

  PdfDocumentOcr {
    meta,
    text,
    structured_text,
    images,
    annotations,
    image_pages,
    annotation_pages,
    form_fields: forms.fields,
    form_field_pages,
  }
}
