//! What recognition runs on: how many pictures are read at once, and where Tesseract's
//! language data is.

use std::sync::Arc;

/// How many pictures may be read at once: what was asked for, or four, and no more than the
/// machine has cores.
pub fn normalize_max_threads(v: Option<u32>) -> u32 {
  let default = 4u32;
  let max = std::thread::available_parallelism()
    .map(|n| n.get() as u32)
    .unwrap_or(default);
  v.unwrap_or(default).clamp(1, max)
}

/// The pool that reads at most `threads` pictures at once, made once for each size and kept.
pub fn pool(threads: usize) -> Arc<rayon::ThreadPool> {
  use std::collections::HashMap;
  use std::sync::{Mutex, OnceLock};

  static POOLS: OnceLock<Mutex<HashMap<usize, Arc<rayon::ThreadPool>>>> = OnceLock::new();
  let map = POOLS.get_or_init(|| Mutex::new(HashMap::new()));
  let mut guard = map.lock().unwrap();
  Arc::clone(guard.entry(threads).or_insert_with(|| {
    Arc::new(
      rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .expect("failed to build OCR thread pool"),
    )
  }))
}

/// Auto-detect tessdata path, cached for the lifetime of the process.
pub(crate) fn tessdata_prefix() -> Option<&'static str> {
  use std::sync::OnceLock;
  static TESSDATA_PATH: OnceLock<Option<String>> = OnceLock::new();

  TESSDATA_PATH
    .get_or_init(|| {
      if let Ok(path) = std::env::var("TESSDATA_PREFIX") {
        return Some(path);
      }

      let output = std::process::Command::new("tesseract")
        .arg("--list-langs")
        .output()
        .ok()?;

      let stderr = String::from_utf8_lossy(&output.stderr);
      let text = if stderr.contains('"') {
        stderr
      } else {
        String::from_utf8_lossy(&output.stdout)
      };

      let start = text.find('"')?;
      let end = text[start + 1..].find('"')?;
      Some(text[start + 1..start + 1 + end].to_string())
    })
    .as_deref()
}

/// Whether Tesseract starts here with this language, which may be several joined with `+`.
/// The reading functions give no words when it does not, and a page with no words looks the
/// same as a blank one; this is how the two are told apart. A language that has started once
/// is not tried again.
pub fn ready(lang: &str) -> Result<(), String> {
  use std::collections::HashSet;
  use std::sync::{Mutex, OnceLock};

  static STARTED: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
  let started = STARTED.get_or_init(|| Mutex::new(HashSet::new()));
  if started.lock().is_ok_and(|known| known.contains(lang)) {
    return Ok(());
  }
  let tess = tesseract_rs::TesseractAPI::new();
  if tess.init(tessdata_prefix().unwrap_or(""), lang).is_err() {
    return Err(format!(
      "Tesseract has no data here for the language `{lang}`"
    ));
  }
  if let Ok(mut known) = started.lock() {
    known.insert(lang.to_string());
  }
  Ok(())
}

/// What `read` makes of each of `items`, in the order of the items, with no more of them being
/// read at once than the cap allows: what was asked for, or four, and never more than the
/// machine has cores.
pub fn in_order<T, R>(
  max_threads: Option<u32>,
  items: Vec<T>,
  read: impl Fn(T) -> R + Send + Sync,
) -> Vec<R>
where
  T: Send,
  R: Send,
{
  use rayon::prelude::*;

  let threads = normalize_max_threads(max_threads) as usize;
  pool(threads).install(|| items.into_par_iter().map(read).collect())
}

#[cfg(test)]
mod tests;
