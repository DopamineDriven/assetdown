// Bound Tesseract's OpenMP pool before libtesseract loads: libgomp reads the environment in its
// constructor, so this cannot be done from Rust once the native module is loaded. One OpenMP
// thread per page being read; parallelism across cores comes from OcrOptions.maxThreads, so
// nothing has to be configured to use them. Measured for `@d0paminedriven/pdfdown-ocr`, which
// reads pages the same way, on a 12-core box over 9 scanned pages: 13.7 s with the limit, 196 s
// without it. Whoever does set either OpenMP variable keeps their setting.
if (!process.env.OMP_THREAD_LIMIT && !process.env.OMP_NUM_THREADS) {
  process.env.OMP_THREAD_LIMIT = "1";
}
