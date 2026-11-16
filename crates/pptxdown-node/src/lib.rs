//! Node bindings for pptxdown.
//!
//! `PptxDown` holds a parsed deck. Build one with the constructor, or with `openAsync` to parse
//! off the JavaScript thread, then read from it as often as needed; nothing is parsed twice. The
//! free functions take a buffer and parse it for that one call. Every reader has an `...Async`
//! form that does its work on the libuv thread pool: the reading there, and the making of
//! JavaScript values back on the JavaScript thread.

// Public so its enums keep the spellings JavaScript sees (`UBL`, `DMY`) without a lint exception.
pub mod invoice;
mod layout;
mod types;

use std::convert::identity;
use std::sync::Arc;

use napi::bindgen_prelude::{AsyncTask, Buffer, Error, Result};
use napi::{Env, Task};
use napi_derive::napi;
use pptxdown::{Options, Presentation, PresentationError};

use crate::invoice::{InvoiceOptions, InvoiceResult};
use crate::layout::{PptxPage, pages_of};
use crate::types::{
  DocumentRead, ImageRead, MediaRead, PageText, PptxAnnotation, PptxChart, PptxDocument, PptxImage,
  PptxMedia, PptxMeta, PptxNote, PptxOptions, PptxSlide, PptxTable, StructuredPageText,
  annotations_of, charts_of, document_js, document_read, images_js, images_read, media_js,
  media_read, metadata_of, notes_of, options_of, slides_of, structured_text_of, tables_of,
  text_per_page_of,
};

// A parsed deck is handed to worker threads behind an `Arc`.
const _: () = {
  fn assert_send_sync<T: Send + Sync>() {}
  let _ = assert_send_sync::<Presentation>;
};

/// A deck error as the JavaScript error it is thrown or rejected with. The message is the
/// error's own text: `unsupported format: …`, `limit exceeded: …` or `malformed presentation: …`.
fn js_error(error: PresentationError) -> Error {
  Error::from_reason(error.to_string())
}

fn open(bytes: Vec<u8>, options: Options) -> Result<Presentation> {
  Presentation::open_with(bytes, options).map_err(js_error)
}

/// Parses a deck on a worker thread, for `PptxDown.openAsync`.
pub struct OpenPresentationTask {
  bytes: Vec<u8>,
  options: Options,
}

#[napi]
impl Task for OpenPresentationTask {
  type Output = Presentation;
  type JsValue = PptxDown;

  fn compute(&mut self) -> Result<Self::Output> {
    open(std::mem::take(&mut self.bytes), self.options)
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(PptxDown {
      inner: Arc::new(output),
    })
  }
}

fn text_of(deck: &Presentation) -> String {
  deck.text()
}

fn markdown_of(deck: &Presentation) -> String {
  deck.markdown()
}

/// Declares the two worker-thread tasks of one reader: the one that reads the deck a `PptxDown`
/// holds, and the one that parses a buffer first, for the free function. A reader is a function
/// from a deck to what it reads. One whose result carries bytes names two more things: what it
/// reads on the worker thread, and the function that makes the JavaScript value from that once
/// the work is back on the JavaScript thread.
macro_rules! tasks {
  ($shared:ident, $alone:ident, $value:ty, $read:path) => {
    tasks!($shared, $alone, $value, $value, $read, identity);
  };
  ($shared:ident, $alone:ident, $raw:ty, $value:ty, $read:path, $convert:path) => {
    pub struct $shared {
      inner: Arc<Presentation>,
    }

    #[napi]
    impl Task for $shared {
      type Output = $raw;
      type JsValue = $value;

      fn compute(&mut self) -> Result<Self::Output> {
        Ok($read(&self.inner))
      }

      fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok($convert(output))
      }
    }

    pub struct $alone {
      bytes: Vec<u8>,
    }

    #[napi]
    impl Task for $alone {
      type Output = $raw;
      type JsValue = $value;

      fn compute(&mut self) -> Result<Self::Output> {
        let deck = open(std::mem::take(&mut self.bytes), Options::default())?;
        Ok($read(&deck))
      }

      fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok($convert(output))
      }
    }
  };
}

tasks!(SharedMetadataTask, MetadataTask, PptxMeta, metadata_of);
tasks!(SharedTextTask, TextTask, String, text_of);
tasks!(
  SharedTextPerPageTask,
  TextPerPageTask,
  Vec<PageText>,
  text_per_page_of
);
tasks!(
  SharedStructuredTextTask,
  StructuredTextTask,
  Vec<StructuredPageText>,
  structured_text_of
);
tasks!(SharedSlidesTask, SlidesTask, Vec<PptxSlide>, slides_of);
tasks!(SharedMarkdownTask, MarkdownTask, String, markdown_of);
tasks!(SharedTablesTask, TablesTask, Vec<PptxTable>, tables_of);
tasks!(
  SharedImagesTask,
  ImagesTask,
  Vec<ImageRead>,
  Vec<PptxImage>,
  images_read,
  images_js
);
tasks!(
  SharedAnnotationsTask,
  AnnotationsTask,
  Vec<PptxAnnotation>,
  annotations_of
);
tasks!(SharedNotesTask, NotesTask, Vec<PptxNote>, notes_of);
tasks!(SharedChartsTask, ChartsTask, Vec<PptxChart>, charts_of);
tasks!(
  SharedMediaTask,
  MediaTask,
  Vec<MediaRead>,
  Vec<PptxMedia>,
  media_read,
  media_js
);
tasks!(SharedLayoutTask, LayoutTask, Vec<PptxPage>, pages_of);
tasks!(
  SharedDocumentTask,
  DocumentTask,
  DocumentRead,
  PptxDocument,
  document_read,
  document_js
);

/// What a deck buffer says about itself, the size of its slides, and counts of what it holds.
/// Parses the buffer for this call; hold a `PptxDown` to read the same deck more than once.
#[napi]
pub fn pptx_metadata(buffer: Buffer) -> Result<PptxMeta> {
  let deck = open(buffer.to_vec(), Options::default())?;
  Ok(metadata_of(&deck))
}

/// `pptxMetadata` on a worker thread: the buffer is copied, then parsed and read there.
#[napi]
pub fn pptx_metadata_async(buffer: Buffer) -> AsyncTask<MetadataTask> {
  AsyncTask::new(MetadataTask {
    bytes: buffer.to_vec(),
  })
}

/// The slides of a deck buffer as plain text, slide by slide, in reading order.
#[napi]
pub fn pptx_text(buffer: Buffer) -> Result<String> {
  let deck = open(buffer.to_vec(), Options::default())?;
  Ok(text_of(&deck))
}

/// `pptxText` on a worker thread: the buffer is copied, then parsed and read there.
#[napi]
pub fn pptx_text_async(buffer: Buffer) -> AsyncTask<TextTask> {
  AsyncTask::new(TextTask {
    bytes: buffer.to_vec(),
  })
}

/// The text of each slide of a deck buffer.
#[napi]
pub fn pptx_text_per_page(buffer: Buffer) -> Result<Vec<PageText>> {
  let deck = open(buffer.to_vec(), Options::default())?;
  Ok(text_per_page_of(&deck))
}

/// `pptxTextPerPage` on a worker thread: the buffer is copied, then parsed and read there.
#[napi]
pub fn pptx_text_per_page_async(buffer: Buffer) -> AsyncTask<TextPerPageTask> {
  AsyncTask::new(TextPerPageTask {
    bytes: buffer.to_vec(),
  })
}

/// The text of each slide of a deck buffer, with its dates, footers and slide numbers set apart
/// from its body.
#[napi]
pub fn pptx_structured_text(buffer: Buffer) -> Result<Vec<StructuredPageText>> {
  let deck = open(buffer.to_vec(), Options::default())?;
  Ok(structured_text_of(&deck))
}

/// `pptxStructuredText` on a worker thread: the buffer is copied, then parsed and read there.
#[napi]
pub fn pptx_structured_text_async(buffer: Buffer) -> AsyncTask<StructuredTextTask> {
  AsyncTask::new(StructuredTextTask {
    bytes: buffer.to_vec(),
  })
}

/// The slides of a deck buffer, each with the shapes on it.
#[napi]
pub fn pptx_slides(buffer: Buffer) -> Result<Vec<PptxSlide>> {
  let deck = open(buffer.to_vec(), Options::default())?;
  Ok(slides_of(&deck))
}

/// `pptxSlides` on a worker thread: the buffer is copied, then parsed and read there.
#[napi]
pub fn pptx_slides_async(buffer: Buffer) -> AsyncTask<SlidesTask> {
  AsyncTask::new(SlidesTask {
    bytes: buffer.to_vec(),
  })
}

/// A deck buffer as Markdown: a heading for each slide, then what the slide holds.
#[napi]
pub fn pptx_markdown(buffer: Buffer) -> Result<String> {
  let deck = open(buffer.to_vec(), Options::default())?;
  Ok(markdown_of(&deck))
}

/// `pptxMarkdown` on a worker thread: the buffer is copied, then parsed and read there.
#[napi]
pub fn pptx_markdown_async(buffer: Buffer) -> AsyncTask<MarkdownTask> {
  AsyncTask::new(MarkdownTask {
    bytes: buffer.to_vec(),
  })
}

/// The tables of a deck buffer, each keyed by row and then column.
#[napi]
pub fn pptx_tables(buffer: Buffer) -> Result<Vec<PptxTable>> {
  let deck = open(buffer.to_vec(), Options::default())?;
  Ok(tables_of(&deck))
}

/// `pptxTables` on a worker thread: the buffer is copied, then parsed and read there.
#[napi]
pub fn pptx_tables_async(buffer: Buffer) -> AsyncTask<TablesTask> {
  AsyncTask::new(TablesTask {
    bytes: buffer.to_vec(),
  })
}

/// The pictures of a deck buffer, one entry for each place a picture is used.
#[napi]
pub fn pptx_images(buffer: Buffer) -> Result<Vec<PptxImage>> {
  let deck = open(buffer.to_vec(), Options::default())?;
  Ok(images_js(images_read(&deck)))
}

/// `pptxImages` on a worker thread: the buffer is copied, then parsed and read there.
#[napi]
pub fn pptx_images_async(buffer: Buffer) -> AsyncTask<ImagesTask> {
  AsyncTask::new(ImagesTask {
    bytes: buffer.to_vec(),
  })
}

/// The hyperlinks and comments of a deck buffer.
#[napi]
pub fn pptx_annotations(buffer: Buffer) -> Result<Vec<PptxAnnotation>> {
  let deck = open(buffer.to_vec(), Options::default())?;
  Ok(annotations_of(&deck))
}

/// `pptxAnnotations` on a worker thread: the buffer is copied, then parsed and read there.
#[napi]
pub fn pptx_annotations_async(buffer: Buffer) -> AsyncTask<AnnotationsTask> {
  AsyncTask::new(AnnotationsTask {
    bytes: buffer.to_vec(),
  })
}

/// The speaker notes of a deck buffer, for each slide that has any.
#[napi]
pub fn pptx_notes(buffer: Buffer) -> Result<Vec<PptxNote>> {
  let deck = open(buffer.to_vec(), Options::default())?;
  Ok(notes_of(&deck))
}

/// `pptxNotes` on a worker thread: the buffer is copied, then parsed and read there.
#[napi]
pub fn pptx_notes_async(buffer: Buffer) -> AsyncTask<NotesTask> {
  AsyncTask::new(NotesTask {
    bytes: buffer.to_vec(),
  })
}

/// The charts of a deck buffer, as data.
#[napi]
pub fn pptx_charts(buffer: Buffer) -> Result<Vec<PptxChart>> {
  let deck = open(buffer.to_vec(), Options::default())?;
  Ok(charts_of(&deck))
}

/// `pptxCharts` on a worker thread: the buffer is copied, then parsed and read there.
#[napi]
pub fn pptx_charts_async(buffer: Buffer) -> AsyncTask<ChartsTask> {
  AsyncTask::new(ChartsTask {
    bytes: buffer.to_vec(),
  })
}

/// The audio and video clips of a deck buffer, one entry for each place a clip is used.
#[napi]
pub fn pptx_media(buffer: Buffer) -> Result<Vec<PptxMedia>> {
  let deck = open(buffer.to_vec(), Options::default())?;
  Ok(media_js(media_read(&deck)))
}

/// `pptxMedia` on a worker thread: the buffer is copied, then parsed and read there.
#[napi]
pub fn pptx_media_async(buffer: Buffer) -> AsyncTask<MediaTask> {
  AsyncTask::new(MediaTask {
    bytes: buffer.to_vec(),
  })
}

/// The slides of a deck buffer as pages of positioned words.
#[napi]
pub fn pptx_layout(buffer: Buffer) -> Result<Vec<PptxPage>> {
  let deck = open(buffer.to_vec(), Options::default())?;
  Ok(pages_of(&deck))
}

/// `pptxLayout` on a worker thread: the buffer is copied, then parsed and read there.
#[napi]
pub fn pptx_layout_async(buffer: Buffer) -> AsyncTask<LayoutTask> {
  AsyncTask::new(LayoutTask {
    bytes: buffer.to_vec(),
  })
}

/// Everything read from a deck buffer, in one object.
#[napi]
pub fn pptx_document(buffer: Buffer) -> Result<PptxDocument> {
  let deck = open(buffer.to_vec(), Options::default())?;
  Ok(document_js(document_read(&deck)))
}

/// `pptxDocument` on a worker thread: the buffer is copied, then parsed and read there.
#[napi]
pub fn pptx_document_async(buffer: Buffer) -> AsyncTask<DocumentTask> {
  AsyncTask::new(DocumentTask {
    bytes: buffer.to_vec(),
  })
}

/// Reads invoice fields from the deck a `PptxDown` holds, for its `invoiceAsync`.
pub struct SharedInvoiceTask {
  inner: Arc<Presentation>,
  opts: Option<contentdown::invoice::InvoiceOptions>,
}

#[napi]
impl Task for SharedInvoiceTask {
  type Output = contentdown::invoice::InvoiceResult;
  type JsValue = InvoiceResult;

  fn compute(&mut self) -> Result<Self::Output> {
    Ok(self.inner.invoice(self.opts.as_ref()))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into())
  }
}

/// Parses a buffer and reads its invoice fields, for the standalone `pptxInvoiceAsync`.
pub struct InvoiceTask {
  bytes: Vec<u8>,
  opts: Option<contentdown::invoice::InvoiceOptions>,
}

#[napi]
impl Task for InvoiceTask {
  type Output = contentdown::invoice::InvoiceResult;
  type JsValue = InvoiceResult;

  fn compute(&mut self) -> Result<Self::Output> {
    let deck = open(std::mem::take(&mut self.bytes), Options::default())?;
    Ok(deck.invoice(self.opts.as_ref()))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into())
  }
}

/// Invoice fields read from a deck buffer's slides as `pptxLayout` sets them.
#[napi]
pub fn pptx_invoice(buffer: Buffer, opts: Option<InvoiceOptions>) -> Result<InvoiceResult> {
  let deck = open(buffer.to_vec(), Options::default())?;
  Ok(deck.invoice(opts.map(Into::into).as_ref()).into())
}

/// `pptxInvoice` on a worker thread.
#[napi]
pub fn pptx_invoice_async(buffer: Buffer, opts: Option<InvoiceOptions>) -> AsyncTask<InvoiceTask> {
  AsyncTask::new(InvoiceTask {
    bytes: buffer.to_vec(),
    opts: opts.map(Into::into),
  })
}

/// A parsed deck. Parsing happens once, in the constructor or in `openAsync`; every method reads
/// from the parsed deck. A slide is a page: the `page` a reader names is the slide's number, from
/// 1, in the order of the deck.
#[napi]
pub struct PptxDown {
  inner: Arc<Presentation>,
}

#[napi]
impl PptxDown {
  /// Parse a deck from the bytes of a `.pptx`, `.pptm`, `.potx` or `.ppsx` file. Throws
  /// `unsupported format: …` for a file that is not a presentation (a workbook, a legacy `.ppt`,
  /// a deck protected with a password), `limit exceeded: …` for one past the size limits and
  /// `malformed presentation: …` for anything else that cannot be read.
  #[napi(constructor)]
  pub fn new(buffer: Buffer, opts: Option<PptxOptions>) -> Result<Self> {
    Ok(PptxDown {
      inner: Arc::new(open(buffer.to_vec(), options_of(opts))?),
    })
  }

  /// Parse a deck off the JavaScript thread; resolves to a `PptxDown`, or rejects with the errors
  /// the constructor throws.
  #[napi]
  pub fn open_async(buffer: Buffer, opts: Option<PptxOptions>) -> AsyncTask<OpenPresentationTask> {
    AsyncTask::new(OpenPresentationTask {
      bytes: buffer.to_vec(),
      options: options_of(opts),
    })
  }

  /// What the deck says about itself (title, author, dates), the size of its slides with the
  /// paper that is nearest to, and counts of what it holds.
  #[napi]
  pub fn metadata(&self) -> PptxMeta {
    metadata_of(&self.inner)
  }

  /// `metadata` on a worker thread.
  #[napi]
  pub fn metadata_async(&self) -> AsyncTask<SharedMetadataTask> {
    AsyncTask::new(SharedMetadataTask {
      inner: Arc::clone(&self.inner),
    })
  }

  /// The deck as plain text, slide by slide with a blank line between slides. A slide is read
  /// title first, then top to bottom and left to right; a paragraph is a line with its bullet in
  /// front, a table row a line with tabs between its cells. Dates, footers and slide numbers,
  /// which repeat on every slide, are left out, and so are speaker notes.
  #[napi]
  pub fn text(&self) -> String {
    self.inner.text()
  }

  /// `text` on a worker thread.
  #[napi]
  pub fn text_async(&self) -> AsyncTask<SharedTextTask> {
    AsyncTask::new(SharedTextTask {
      inner: Arc::clone(&self.inner),
    })
  }

  /// The text of each slide. Joined by blank lines, the slides are `text()`.
  #[napi]
  pub fn text_per_page(&self) -> Vec<PageText> {
    text_per_page_of(&self.inner)
  }

  /// `textPerPage` on a worker thread.
  #[napi]
  pub fn text_per_page_async(&self) -> AsyncTask<SharedTextPerPageTask> {
    AsyncTask::new(SharedTextPerPageTask {
      inner: Arc::clone(&self.inner),
    })
  }

  /// The text of each slide with what repeats on every slide set apart from the body: a date,
  /// a footer or a slide number is `header` when it sits in the top half of the slide and
  /// `footer` when it sits in the bottom half. A deck of one slide has nothing that repeats, so
  /// all of it is `body`.
  #[napi]
  pub fn structured_text(&self) -> Vec<StructuredPageText> {
    structured_text_of(&self.inner)
  }

  /// `structuredText` on a worker thread.
  #[napi]
  pub fn structured_text_async(&self) -> AsyncTask<SharedStructuredTextTask> {
    AsyncTask::new(SharedStructuredTextTask {
      inner: Arc::clone(&self.inner),
    })
  }

  /// The slides, each with its title, the layout it is built on and the shapes on it in reading
  /// order. A shape is a union keyed on `type`; a group and a diagram hold shapes of their own.
  #[napi]
  pub fn slides(&self) -> Vec<PptxSlide> {
    slides_of(&self.inner)
  }

  /// `slides` on a worker thread.
  #[napi]
  pub fn slides_async(&self) -> AsyncTask<SharedSlidesTask> {
    AsyncTask::new(SharedSlidesTask {
      inner: Arc::clone(&self.inner),
    })
  }

  /// The deck as Markdown: a heading for each slide (its title, or `Slide 3`), then lists,
  /// emphasis, links, pipe tables, a chart as a table of its data, pictures, and the speaker
  /// notes as a quote.
  #[napi]
  pub fn markdown(&self) -> String {
    self.inner.markdown()
  }

  /// `markdown` on a worker thread.
  #[napi]
  pub fn markdown_async(&self) -> AsyncTask<SharedMarkdownTask> {
    AsyncTask::new(SharedMarkdownTask {
      inner: Arc::clone(&self.inner),
    })
  }

  /// The tables of the deck, each keyed by row and then column, so a cell, or a whole row, is
  /// one lookup: `rows["2"]["B"]`.
  #[napi]
  pub fn tables(&self) -> Vec<PptxTable> {
    tables_of(&self.inner)
  }

  /// `tables` on a worker thread.
  #[napi]
  pub fn tables_async(&self) -> AsyncTask<SharedTablesTask> {
    AsyncTask::new(SharedTablesTask {
      inner: Arc::clone(&self.inner),
    })
  }

  /// Every place a picture is used, slide by slide, with the bytes as the deck stores them,
  /// their format, the picture's size in pixels and the box it is shown in.
  #[napi]
  pub fn images(&self) -> Vec<PptxImage> {
    images_js(images_read(&self.inner))
  }

  /// `images` on a worker thread.
  #[napi]
  pub fn images_async(&self) -> AsyncTask<SharedImagesTask> {
    AsyncTask::new(SharedImagesTask {
      inner: Arc::clone(&self.inner),
    })
  }

  /// Every hyperlink and comment, slide by slide: a link with where it leads, the text it covers
  /// and the box of the shape it is in; a comment with what it says and who wrote it.
  #[napi]
  pub fn annotations(&self) -> Vec<PptxAnnotation> {
    annotations_of(&self.inner)
  }

  /// `annotations` on a worker thread.
  #[napi]
  pub fn annotations_async(&self) -> AsyncTask<SharedAnnotationsTask> {
    AsyncTask::new(SharedAnnotationsTask {
      inner: Arc::clone(&self.inner),
    })
  }

  /// The speaker notes, for each slide that has any.
  #[napi]
  pub fn notes(&self) -> Vec<PptxNote> {
    notes_of(&self.inner)
  }

  /// `notes` on a worker thread.
  #[napi]
  pub fn notes_async(&self) -> AsyncTask<SharedNotesTask> {
    AsyncTask::new(SharedNotesTask {
      inner: Arc::clone(&self.inner),
    })
  }

  /// The charts of the deck as data: each series with its name, its categories and the values
  /// the application last showed. Nothing is drawn.
  #[napi]
  pub fn charts(&self) -> Vec<PptxChart> {
    charts_of(&self.inner)
  }

  /// `charts` on a worker thread.
  #[napi]
  pub fn charts_async(&self) -> AsyncTask<SharedChartsTask> {
    AsyncTask::new(SharedChartsTask {
      inner: Arc::clone(&self.inner),
    })
  }

  /// Every place an audio or a video clip is used, with the bytes as the deck stores them.
  #[napi]
  pub fn media(&self) -> Vec<PptxMedia> {
    media_js(media_read(&self.inner))
  }

  /// `media` on a worker thread.
  #[napi]
  pub fn media_async(&self) -> AsyncTask<SharedMediaTask> {
    AsyncTask::new(SharedMediaTask {
      inner: Arc::clone(&self.inner),
    })
  }

  /// The deck as pages of positioned words, one page for each slide, with the boxes of its
  /// pictures. The boxes are the file's own; where a line wraps inside its box is estimated. See
  /// `PptxPage`.
  #[napi]
  pub fn layout(&self) -> Vec<PptxPage> {
    pages_of(&self.inner)
  }

  /// `layout` on a worker thread.
  #[napi]
  pub fn layout_async(&self) -> AsyncTask<SharedLayoutTask> {
    AsyncTask::new(SharedLayoutTask {
      inner: Arc::clone(&self.inner),
    })
  }

  /// Invoice fields read from the deck's slides as `layout()` sets them.
  #[napi]
  pub fn invoice(&self, opts: Option<InvoiceOptions>) -> InvoiceResult {
    self.inner.invoice(opts.map(Into::into).as_ref()).into()
  }

  /// `invoice` on a worker thread.
  #[napi]
  pub fn invoice_async(&self, opts: Option<InvoiceOptions>) -> AsyncTask<SharedInvoiceTask> {
    AsyncTask::new(SharedInvoiceTask {
      inner: Arc::clone(&self.inner),
      opts: opts.map(Into::into),
    })
  }

  /// Everything read from the deck in one object: what each reader above returns under the
  /// reader's own name.
  #[napi]
  pub fn document(&self) -> PptxDocument {
    document_js(document_read(&self.inner))
  }

  /// `document` on a worker thread.
  #[napi]
  pub fn document_async(&self) -> AsyncTask<SharedDocumentTask> {
    AsyncTask::new(SharedDocumentTask {
      inner: Arc::clone(&self.inner),
    })
  }
}
