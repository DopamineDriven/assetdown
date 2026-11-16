//! Node bindings for docxdown.
//!
//! `DocxDown` holds a parsed document. Build one with the constructor, or with `openAsync` to
//! parse off the JavaScript thread, then read from it as often as needed; nothing is parsed
//! twice. The free functions take a buffer and parse it for that one call. Every reader has an
//! `...Async` form that does its work on the libuv thread pool.

// Public so its enums keep the spellings JavaScript sees (`UBL`, `DMY`) without a lint exception.
pub mod invoice;
mod layout;
mod types;

use std::sync::Arc;

use docxdown::{Document, DocumentError, Options};
use napi::bindgen_prelude::{AsyncTask, Buffer, Error, Result};
use napi::{Env, Task};
use napi_derive::napi;

use crate::invoice::{InvoiceOptions, InvoiceResult};
use crate::layout::{DocxPage, pages_of};
use crate::types::{
  Block, DocxAnnotation, DocxDocument, DocxFormField, DocxImage, DocxMeta, DocxNote, DocxOptions,
  DocxTable, PageText, annotations_of, blocks_of, document_of, form_fields_of, images_of,
  metadata_of, notes_of, options_of, tables_of, text_per_page_of,
};

// A parsed document is handed to worker threads behind an `Arc`.
const _: () = {
  fn assert_send_sync<T: Send + Sync>() {}
  let _ = assert_send_sync::<Document>;
};

/// A document error as the JavaScript error it is thrown or rejected with. The message is the
/// error's own text: `unsupported format: …`, `limit exceeded: …` or `malformed document: …`.
fn js_error(error: DocumentError) -> Error {
  Error::from_reason(error.to_string())
}

fn open(bytes: Vec<u8>, options: Options) -> Result<Document> {
  Document::open_with(bytes, options).map_err(js_error)
}

/// Parses a document on a worker thread, for `DocxDown.openAsync`.
pub struct OpenDocumentTask {
  bytes: Vec<u8>,
  options: Options,
}

#[napi]
impl Task for OpenDocumentTask {
  type Output = Document;
  type JsValue = DocxDown;

  fn compute(&mut self) -> Result<Self::Output> {
    open(std::mem::take(&mut self.bytes), self.options)
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(DocxDown {
      inner: Arc::new(output),
    })
  }
}

fn text_of(document: &Document) -> String {
  document.text()
}

fn markdown_of(document: &Document) -> String {
  document.markdown()
}

/// Declares the two worker-thread tasks of one reader: the one that reads the document a
/// `DocxDown` holds, and the one that parses a buffer first, for the free function.
macro_rules! tasks {
  ($shared:ident, $alone:ident, $value:ty, $read:path) => {
    pub struct $shared {
      inner: Arc<Document>,
    }

    #[napi]
    impl Task for $shared {
      type Output = $value;
      type JsValue = $value;

      fn compute(&mut self) -> Result<Self::Output> {
        Ok($read(&self.inner))
      }

      fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output)
      }
    }

    pub struct $alone {
      bytes: Vec<u8>,
    }

    #[napi]
    impl Task for $alone {
      type Output = $value;
      type JsValue = $value;

      fn compute(&mut self) -> Result<Self::Output> {
        let document = open(std::mem::take(&mut self.bytes), Options::default())?;
        Ok($read(&document))
      }

      fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output)
      }
    }
  };
}

tasks!(SharedMetadataTask, MetadataTask, DocxMeta, metadata_of);
tasks!(SharedTextTask, TextTask, String, text_of);
tasks!(
  SharedTextPerPageTask,
  TextPerPageTask,
  Vec<PageText>,
  text_per_page_of
);
tasks!(SharedBlocksTask, BlocksTask, Vec<Block>, blocks_of);
tasks!(SharedMarkdownTask, MarkdownTask, String, markdown_of);
tasks!(SharedTablesTask, TablesTask, Vec<DocxTable>, tables_of);
tasks!(SharedImagesTask, ImagesTask, Vec<DocxImage>, images_of);
tasks!(
  SharedAnnotationsTask,
  AnnotationsTask,
  Vec<DocxAnnotation>,
  annotations_of
);
tasks!(
  SharedFormFieldsTask,
  FormFieldsTask,
  Vec<DocxFormField>,
  form_fields_of
);
tasks!(SharedNotesTask, NotesTask, Vec<DocxNote>, notes_of);
tasks!(SharedLayoutTask, LayoutTask, Vec<DocxPage>, pages_of);
tasks!(SharedDocumentTask, DocumentTask, DocxDocument, document_of);

/// What a document buffer says about itself, its sections and their paper, and counts of what it
/// holds. Parses the buffer for this call; hold a `DocxDown` to read the same document more than
/// once.
#[napi]
pub fn docx_metadata(buffer: Buffer) -> Result<DocxMeta> {
  let document = open(buffer.to_vec(), Options::default())?;
  Ok(metadata_of(&document))
}

/// `docxMetadata` on a worker thread: the buffer is copied, then parsed and read there.
#[napi]
pub fn docx_metadata_async(buffer: Buffer) -> AsyncTask<MetadataTask> {
  AsyncTask::new(MetadataTask {
    bytes: buffer.to_vec(),
  })
}

/// The body of a document buffer as plain text, in reading order.
#[napi]
pub fn docx_text(buffer: Buffer) -> Result<String> {
  let document = open(buffer.to_vec(), Options::default())?;
  Ok(text_of(&document))
}

/// `docxText` on a worker thread: the buffer is copied, then parsed and read there.
#[napi]
pub fn docx_text_async(buffer: Buffer) -> AsyncTask<TextTask> {
  AsyncTask::new(TextTask {
    bytes: buffer.to_vec(),
  })
}

/// The body's text of a document buffer, page by page.
#[napi]
pub fn docx_text_per_page(buffer: Buffer) -> Result<Vec<PageText>> {
  let document = open(buffer.to_vec(), Options::default())?;
  Ok(text_per_page_of(&document))
}

/// `docxTextPerPage` on a worker thread: the buffer is copied, then parsed and read there.
#[napi]
pub fn docx_text_per_page_async(buffer: Buffer) -> AsyncTask<TextPerPageTask> {
  AsyncTask::new(TextPerPageTask {
    bytes: buffer.to_vec(),
  })
}

/// The body of a document buffer as a tree of paragraphs and tables.
#[napi]
pub fn docx_blocks(buffer: Buffer) -> Result<Vec<Block>> {
  let document = open(buffer.to_vec(), Options::default())?;
  Ok(blocks_of(&document))
}

/// `docxBlocks` on a worker thread: the buffer is copied, then parsed and read there.
#[napi]
pub fn docx_blocks_async(buffer: Buffer) -> AsyncTask<BlocksTask> {
  AsyncTask::new(BlocksTask {
    bytes: buffer.to_vec(),
  })
}

/// The body of a document buffer as Markdown, with its notes after it.
#[napi]
pub fn docx_markdown(buffer: Buffer) -> Result<String> {
  let document = open(buffer.to_vec(), Options::default())?;
  Ok(markdown_of(&document))
}

/// `docxMarkdown` on a worker thread: the buffer is copied, then parsed and read there.
#[napi]
pub fn docx_markdown_async(buffer: Buffer) -> AsyncTask<MarkdownTask> {
  AsyncTask::new(MarkdownTask {
    bytes: buffer.to_vec(),
  })
}

/// The tables of a document buffer, each keyed by row and then column.
#[napi]
pub fn docx_tables(buffer: Buffer) -> Result<Vec<DocxTable>> {
  let document = open(buffer.to_vec(), Options::default())?;
  Ok(tables_of(&document))
}

/// `docxTables` on a worker thread: the buffer is copied, then parsed and read there.
#[napi]
pub fn docx_tables_async(buffer: Buffer) -> AsyncTask<TablesTask> {
  AsyncTask::new(TablesTask {
    bytes: buffer.to_vec(),
  })
}

/// The pictures of a document buffer, one entry for each place a picture is used.
#[napi]
pub fn docx_images(buffer: Buffer) -> Result<Vec<DocxImage>> {
  let document = open(buffer.to_vec(), Options::default())?;
  Ok(images_of(&document))
}

/// `docxImages` on a worker thread: the buffer is copied, then parsed and read there.
#[napi]
pub fn docx_images_async(buffer: Buffer) -> AsyncTask<ImagesTask> {
  AsyncTask::new(ImagesTask {
    bytes: buffer.to_vec(),
  })
}

/// The hyperlinks and comments of a document buffer.
#[napi]
pub fn docx_annotations(buffer: Buffer) -> Result<Vec<DocxAnnotation>> {
  let document = open(buffer.to_vec(), Options::default())?;
  Ok(annotations_of(&document))
}

/// `docxAnnotations` on a worker thread: the buffer is copied, then parsed and read there.
#[napi]
pub fn docx_annotations_async(buffer: Buffer) -> AsyncTask<AnnotationsTask> {
  AsyncTask::new(AnnotationsTask {
    bytes: buffer.to_vec(),
  })
}

/// The content controls, form fields and merge fields of a document buffer, with their values.
#[napi]
pub fn docx_form_fields(buffer: Buffer) -> Result<Vec<DocxFormField>> {
  let document = open(buffer.to_vec(), Options::default())?;
  Ok(form_fields_of(&document))
}

/// `docxFormFields` on a worker thread: the buffer is copied, then parsed and read there.
#[napi]
pub fn docx_form_fields_async(buffer: Buffer) -> AsyncTask<FormFieldsTask> {
  AsyncTask::new(FormFieldsTask {
    bytes: buffer.to_vec(),
  })
}

/// The footnotes and endnotes of a document buffer.
#[napi]
pub fn docx_notes(buffer: Buffer) -> Result<Vec<DocxNote>> {
  let document = open(buffer.to_vec(), Options::default())?;
  Ok(notes_of(&document))
}

/// `docxNotes` on a worker thread: the buffer is copied, then parsed and read there.
#[napi]
pub fn docx_notes_async(buffer: Buffer) -> AsyncTask<NotesTask> {
  AsyncTask::new(NotesTask {
    bytes: buffer.to_vec(),
  })
}

/// A document buffer laid out as pages of positioned words, by estimate.
#[napi]
pub fn docx_layout(buffer: Buffer) -> Result<Vec<DocxPage>> {
  let document = open(buffer.to_vec(), Options::default())?;
  Ok(pages_of(&document))
}

/// `docxLayout` on a worker thread: the buffer is copied, then parsed and read there.
#[napi]
pub fn docx_layout_async(buffer: Buffer) -> AsyncTask<LayoutTask> {
  AsyncTask::new(LayoutTask {
    bytes: buffer.to_vec(),
  })
}

/// Everything read from a document buffer, in one object.
#[napi]
pub fn docx_document(buffer: Buffer) -> Result<DocxDocument> {
  let document = open(buffer.to_vec(), Options::default())?;
  Ok(document_of(&document))
}

/// `docxDocument` on a worker thread: the buffer is copied, then parsed and read there.
#[napi]
pub fn docx_document_async(buffer: Buffer) -> AsyncTask<DocumentTask> {
  AsyncTask::new(DocumentTask {
    bytes: buffer.to_vec(),
  })
}

/// Reads invoice fields from the document a `DocxDown` holds, for its `invoiceAsync`.
pub struct SharedInvoiceTask {
  inner: Arc<Document>,
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

/// Parses a buffer and reads its invoice fields, for the standalone `docxInvoiceAsync`.
pub struct InvoiceTask {
  bytes: Vec<u8>,
  opts: Option<contentdown::invoice::InvoiceOptions>,
}

#[napi]
impl Task for InvoiceTask {
  type Output = contentdown::invoice::InvoiceResult;
  type JsValue = InvoiceResult;

  fn compute(&mut self) -> Result<Self::Output> {
    let document = open(std::mem::take(&mut self.bytes), Options::default())?;
    Ok(document.invoice(self.opts.as_ref()))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into())
  }
}

/// Invoice fields read from a document buffer's pages as `docxLayout` sets them.
#[napi]
pub fn docx_invoice(buffer: Buffer, opts: Option<InvoiceOptions>) -> Result<InvoiceResult> {
  let document = open(buffer.to_vec(), Options::default())?;
  Ok(document.invoice(opts.map(Into::into).as_ref()).into())
}

/// `docxInvoice` on a worker thread.
#[napi]
pub fn docx_invoice_async(buffer: Buffer, opts: Option<InvoiceOptions>) -> AsyncTask<InvoiceTask> {
  AsyncTask::new(InvoiceTask {
    bytes: buffer.to_vec(),
    opts: opts.map(Into::into),
  })
}

/// A parsed document. Parsing happens once, in the constructor or in `openAsync`; every method
/// reads from the parsed document.
#[napi]
pub struct DocxDown {
  inner: Arc<Document>,
}

#[napi]
impl DocxDown {
  /// Parse a document from the bytes of a `.docx`, `.docm` or `.dotx` file. Throws
  /// `unsupported format: …` for a file that is not a Word document (a workbook, a legacy `.doc`,
  /// a document protected with a password), `limit exceeded: …` for one past the size limits and
  /// `malformed document: …` for anything else that cannot be read.
  #[napi(constructor)]
  pub fn new(buffer: Buffer, opts: Option<DocxOptions>) -> Result<Self> {
    Ok(DocxDown {
      inner: Arc::new(open(buffer.to_vec(), options_of(opts))?),
    })
  }

  /// Parse a document off the JavaScript thread; resolves to a `DocxDown`, or rejects with the
  /// errors the constructor throws.
  #[napi]
  pub fn open_async(buffer: Buffer, opts: Option<DocxOptions>) -> AsyncTask<OpenDocumentTask> {
    AsyncTask::new(OpenDocumentTask {
      bytes: buffer.to_vec(),
      options: options_of(opts),
    })
  }

  /// What the document says about itself (title, author, dates), its sections with the paper
  /// each is nearest to, and counts of what it holds.
  #[napi]
  pub fn metadata(&self) -> DocxMeta {
    metadata_of(&self.inner)
  }

  /// `metadata` on a worker thread.
  #[napi]
  pub fn metadata_async(&self) -> AsyncTask<SharedMetadataTask> {
    AsyncTask::new(SharedMetadataTask {
      inner: Arc::clone(&self.inner),
    })
  }

  /// The body as plain text, in reading order: one line per paragraph and per table row, a list
  /// item with its marker in front, the text of text boxes after the paragraph they are
  /// anchored in.
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

  /// The body's text for each page the reader counted. A paragraph belongs to the page it starts
  /// on; a table is split between its rows. Joined by line breaks, the pages are `text()`.
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

  /// The body as a tree: paragraphs and tables in reading order, a table's cells holding blocks
  /// of their own. Each block is a union keyed on `type`.
  #[napi]
  pub fn blocks(&self) -> Vec<Block> {
    blocks_of(&self.inner)
  }

  /// `blocks` on a worker thread.
  #[napi]
  pub fn blocks_async(&self) -> AsyncTask<SharedBlocksTask> {
    AsyncTask::new(SharedBlocksTask {
      inner: Arc::clone(&self.inner),
    })
  }

  /// The body as Markdown: headings, lists, emphasis, links, pipe tables, and the footnotes and
  /// endnotes after it.
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

  /// The tables of the body, the ones inside cells included, each keyed by row and then column,
  /// so a cell, or a whole row, is one lookup: `rows["2"]["B"]`.
  #[napi]
  pub fn tables(&self) -> Vec<DocxTable> {
    tables_of(&self.inner)
  }

  /// `tables` on a worker thread.
  #[napi]
  pub fn tables_async(&self) -> AsyncTask<SharedTablesTask> {
    AsyncTask::new(SharedTablesTask {
      inner: Arc::clone(&self.inner),
    })
  }

  /// Every place a picture is used, page by page, with the bytes as the document stores them,
  /// their format, the picture's size in pixels and the size it is shown at.
  #[napi]
  pub fn images(&self) -> Vec<DocxImage> {
    images_of(&self.inner)
  }

  /// `images` on a worker thread.
  #[napi]
  pub fn images_async(&self) -> AsyncTask<SharedImagesTask> {
    AsyncTask::new(SharedImagesTask {
      inner: Arc::clone(&self.inner),
    })
  }

  /// Every hyperlink and comment, page by page: a link with where it leads and the text it
  /// covers, a comment with what it says, who wrote it and the text it is attached to.
  #[napi]
  pub fn annotations(&self) -> Vec<DocxAnnotation> {
    annotations_of(&self.inner)
  }

  /// `annotations` on a worker thread.
  #[napi]
  pub fn annotations_async(&self) -> AsyncTask<SharedAnnotationsTask> {
    AsyncTask::new(SharedAnnotationsTask {
      inner: Arc::clone(&self.inner),
    })
  }

  /// The places made to hold a value, with what each holds: content controls, legacy form
  /// fields and mail-merge fields.
  #[napi]
  pub fn form_fields(&self) -> Vec<DocxFormField> {
    form_fields_of(&self.inner)
  }

  /// `formFields` on a worker thread.
  #[napi]
  pub fn form_fields_async(&self) -> AsyncTask<SharedFormFieldsTask> {
    AsyncTask::new(SharedFormFieldsTask {
      inner: Arc::clone(&self.inner),
    })
  }

  /// The footnotes and endnotes, in the order a `noteRef` inline counts them.
  #[napi]
  pub fn notes(&self) -> Vec<DocxNote> {
    notes_of(&self.inner)
  }

  /// `notes` on a worker thread.
  #[napi]
  pub fn notes_async(&self) -> AsyncTask<SharedNotesTask> {
    AsyncTask::new(SharedNotesTask {
      inner: Arc::clone(&self.inner),
    })
  }

  /// The document as pages of positioned words: one page for each the reader counted, with the
  /// header and footer its section gives it. A docx records no positions, so these are
  /// estimates; see `DocxPage`.
  #[napi]
  pub fn layout(&self) -> Vec<DocxPage> {
    pages_of(&self.inner)
  }

  /// `layout` on a worker thread.
  #[napi]
  pub fn layout_async(&self) -> AsyncTask<SharedLayoutTask> {
    AsyncTask::new(SharedLayoutTask {
      inner: Arc::clone(&self.inner),
    })
  }

  /// Invoice fields read from the document's pages as `layout()` sets them.
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

  /// Everything read from the document in one object: what each reader above returns under the
  /// reader's own name, and the headers and footers.
  #[napi]
  pub fn document(&self) -> DocxDocument {
    document_of(&self.inner)
  }

  /// `document` on a worker thread.
  #[napi]
  pub fn document_async(&self) -> AsyncTask<SharedDocumentTask> {
    AsyncTask::new(SharedDocumentTask {
      inner: Arc::clone(&self.inner),
    })
  }
}
