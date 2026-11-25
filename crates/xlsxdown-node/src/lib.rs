//! Node bindings for xlsxdown.
//!
//! `XlsxDown` holds a parsed workbook. Build one with the constructor, or with `openAsync` to
//! parse off the JavaScript thread, then read from it as often as needed; nothing is parsed
//! twice. The free functions take a buffer and parse it for that one call. Every reader has an
//! `...Async` form that does its work on the libuv thread pool.

mod cells;
// Public so its enums keep the spellings JavaScript sees (`UBL`, `DMY`) without a lint exception.
pub mod invoice;
mod layout;
mod meta;
mod types;

use std::sync::Arc;

use napi::bindgen_prelude::{AsyncTask, Buffer, Error, Result};
use napi::{Env, Task};
use napi_derive::napi;
use xlsxdown::{SpreadsheetError, Workbook};

use crate::cells::{cells_of, grid_of};
use crate::invoice::{InvoiceOptions, InvoiceResult};
use crate::layout::{SheetPage, pages_of};
use crate::meta::{XlsxMeta, metadata_of};
use crate::types::{SheetCells, SheetGrid, SpreadsheetOptions};

// A parsed workbook is handed to worker threads behind an `Arc`.
const _: () = {
  fn assert_send_sync<T: Send + Sync>() {}
  let _ = assert_send_sync::<Workbook>;
};

/// A workbook error as the JavaScript error it is thrown or rejected with. The message is the
/// error's own text: `unsupported format: …`, `limit exceeded: …` or `malformed workbook: …`.
fn js_error(error: SpreadsheetError) -> Error {
  Error::from_reason(error.to_string())
}

fn open(bytes: Vec<u8>) -> Result<Workbook> {
  Workbook::open(bytes).map_err(js_error)
}

/// Parses a workbook on a worker thread, for `XlsxDown.openAsync`.
pub struct OpenWorkbookTask(Vec<u8>);

#[napi]
impl Task for OpenWorkbookTask {
  type Output = Workbook;
  type JsValue = XlsxDown;

  fn compute(&mut self) -> Result<Self::Output> {
    open(std::mem::take(&mut self.0))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(XlsxDown {
      inner: Arc::new(output),
    })
  }
}

/// Reads cells from the workbook an `XlsxDown` holds, for its `spreadsheetCellsAsync`.
pub struct SharedCellsTask {
  inner: Arc<Workbook>,
  opts: SpreadsheetOptions,
}

#[napi]
impl Task for SharedCellsTask {
  type Output = Vec<SheetCells>;
  type JsValue = Vec<SheetCells>;

  fn compute(&mut self) -> Result<Self::Output> {
    Ok(cells_of(&self.inner, &self.opts))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}

/// Parses a buffer and reads its cells, for the standalone `spreadsheetCellsAsync`.
pub struct SpreadsheetCellsTask {
  bytes: Vec<u8>,
  opts: SpreadsheetOptions,
}

#[napi]
impl Task for SpreadsheetCellsTask {
  type Output = Vec<SheetCells>;
  type JsValue = Vec<SheetCells>;

  fn compute(&mut self) -> Result<Self::Output> {
    let workbook = open(std::mem::take(&mut self.bytes))?;
    Ok(cells_of(&workbook, &self.opts))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}

/// Keys the workbook an `XlsxDown` holds by row and column, for its `spreadsheetGridAsync`.
pub struct SharedGridTask {
  inner: Arc<Workbook>,
  opts: SpreadsheetOptions,
}

#[napi]
impl Task for SharedGridTask {
  type Output = Vec<SheetGrid>;
  type JsValue = Vec<SheetGrid>;

  fn compute(&mut self) -> Result<Self::Output> {
    Ok(grid_of(&self.inner, &self.opts))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}

/// Lays out the sheets of the workbook an `XlsxDown` holds, for its `layoutAsync`.
pub struct SharedLayoutTask {
  inner: Arc<Workbook>,
  opts: SpreadsheetOptions,
}

#[napi]
impl Task for SharedLayoutTask {
  type Output = Vec<SheetPage>;
  type JsValue = Vec<SheetPage>;

  fn compute(&mut self) -> Result<Self::Output> {
    Ok(pages_of(&self.inner, &self.opts))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}

/// Reads invoice fields from the workbook an `XlsxDown` holds, for its `invoiceAsync`.
pub struct SharedInvoiceTask {
  inner: Arc<Workbook>,
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

/// Counts what the workbook an `XlsxDown` holds, for its `metadataAsync`.
pub struct SharedMetadataTask {
  inner: Arc<Workbook>,
}

#[napi]
impl Task for SharedMetadataTask {
  type Output = XlsxMeta;
  type JsValue = XlsxMeta;

  fn compute(&mut self) -> Result<Self::Output> {
    Ok(metadata_of(&self.inner))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}

/// A parsed workbook. Parsing happens once, in the constructor or in `openAsync`; every method
/// reads from the parsed workbook.
#[napi]
pub struct XlsxDown {
  inner: Arc<Workbook>,
}

#[napi]
impl XlsxDown {
  /// Parse a workbook from the bytes of an `.xlsx` file. Throws `unsupported format: …` for a
  /// package that is not a plain workbook, `limit exceeded: …` for one past the size limits and
  /// `malformed workbook: …` for anything else that cannot be read.
  #[napi(constructor)]
  pub fn new(buffer: Buffer) -> Result<Self> {
    Ok(XlsxDown {
      inner: Arc::new(open(buffer.to_vec())?),
    })
  }

  /// Parse a workbook off the JavaScript thread; resolves to an `XlsxDown`, or rejects with the
  /// errors the constructor throws.
  #[napi]
  pub fn open_async(buffer: Buffer) -> AsyncTask<OpenWorkbookTask> {
    AsyncTask::new(OpenWorkbookTask(buffer.to_vec()))
  }

  /// The cells of the selected sheets, typed, with their display text, formulas and geometry.
  #[napi]
  pub fn spreadsheet_cells(&self, opts: Option<SpreadsheetOptions>) -> Vec<SheetCells> {
    cells_of(&self.inner, &opts.unwrap_or_default())
  }

  /// `spreadsheetCells` on a worker thread.
  #[napi]
  pub fn spreadsheet_cells_async(
    &self,
    opts: Option<SpreadsheetOptions>,
  ) -> AsyncTask<SharedCellsTask> {
    AsyncTask::new(SharedCellsTask {
      inner: Arc::clone(&self.inner),
      opts: opts.unwrap_or_default(),
    })
  }

  /// The cells of the selected sheets keyed by row and then column, so a cell, or a whole row,
  /// is one lookup: `rows["12"]["C"]`.
  #[napi]
  pub fn spreadsheet_grid(&self, opts: Option<SpreadsheetOptions>) -> Vec<SheetGrid> {
    grid_of(&self.inner, &opts.unwrap_or_default())
  }

  /// `spreadsheetGrid` on a worker thread.
  #[napi]
  pub fn spreadsheet_grid_async(
    &self,
    opts: Option<SpreadsheetOptions>,
  ) -> AsyncTask<SharedGridTask> {
    AsyncTask::new(SharedGridTask {
      inner: Arc::clone(&self.inner),
      opts: opts.unwrap_or_default(),
    })
  }

  /// The selected sheets laid out as pages: each cell's text placed in its rectangle, broken
  /// into words and grouped into lines, every word naming its cell. Cells in hidden rows and
  /// columns are left off the page unless `hiddenCells` is set.
  #[napi]
  pub fn layout(&self, opts: Option<SpreadsheetOptions>) -> Vec<SheetPage> {
    pages_of(&self.inner, &opts.unwrap_or_default())
  }

  /// `layout` on a worker thread.
  #[napi]
  pub fn layout_async(&self, opts: Option<SpreadsheetOptions>) -> AsyncTask<SharedLayoutTask> {
    AsyncTask::new(SharedLayoutTask {
      inner: Arc::clone(&self.inner),
      opts: opts.unwrap_or_default(),
    })
  }

  /// Invoice fields read from the workbook's visible sheets, each laid out as a page. Where
  /// the workbook's formulas give a total, it is `fields.invoiceTotal`: of the formulas
  /// nothing else reads, one shown as money before one that is not, and then the one that
  /// reaches the most cells. The lines it adds up are `fields.items`, each with its
  /// description, its quantity and unit price where one cell is multiplied by another, and
  /// where its cells are on the page. Amounts are given as their cells show them, where a
  /// cell shows cents. A warning says when the labels on the page gave another total, and
  /// when the lines do not come to the total. A formula total of nought, and one shown as a
  /// plain number beside a labeled amount shown as money, leave the labels' reading as it
  /// was.
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

  /// What the workbook says about itself (title, author, dates) and counts of what it holds,
  /// sheet by sheet and in total: cells, formulas, merged ranges, tables, pictures, charts,
  /// pivot tables, links and comments. Hidden sheets are counted like any other.
  #[napi]
  pub fn metadata(&self) -> XlsxMeta {
    metadata_of(&self.inner)
  }

  /// `metadata` on a worker thread.
  #[napi]
  pub fn metadata_async(&self) -> AsyncTask<SharedMetadataTask> {
    AsyncTask::new(SharedMetadataTask {
      inner: Arc::clone(&self.inner),
    })
  }
}

/// The cells of a workbook buffer's selected sheets. Parses the buffer for this call; hold an
/// `XlsxDown` to read the same workbook more than once.
#[napi]
pub fn spreadsheet_cells(
  buffer: Buffer,
  opts: Option<SpreadsheetOptions>,
) -> Result<Vec<SheetCells>> {
  let workbook = open(buffer.to_vec())?;
  Ok(cells_of(&workbook, &opts.unwrap_or_default()))
}

/// `spreadsheetCells` on a worker thread: the buffer is copied, then parsed and read there.
#[napi]
pub fn spreadsheet_cells_async(
  buffer: Buffer,
  opts: Option<SpreadsheetOptions>,
) -> AsyncTask<SpreadsheetCellsTask> {
  AsyncTask::new(SpreadsheetCellsTask {
    bytes: buffer.to_vec(),
    opts: opts.unwrap_or_default(),
  })
}
