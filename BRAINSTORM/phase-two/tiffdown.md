# tiffdown: notes

Status: **steps 1 to 5 of section 6 are built** (2026-11-19; step 3, `ocrdown`, is recorded in [ocrdown.md](ocrdown.md)): the directory reader with each page's facts; a page's pixels from Group 4, LZW, and uncompressed strips, with a page as PNG; with the `ocr` feature, each page's words on the page, its text, and the invoice read from them; and the Node binding with the route in `mimedown`. The binding's OCR package still needs its manifests from you: see "What the OCR package still needs" in section 7. The "as built" notes under section 6 say what exists. Rows marked decided are decided; the rest are recommendations waiting for an answer.

Many invoices, especially older ones, are TIFF files, and they should be read. `tiffdown` reads them. Its two companions have notes of their own: [rtfdown.md](rtfdown.md) and [jpegdown.md](jpegdown.md).

## 1. Decisions

| # | Question | Decided or recommended | Why |
| --- | --- | --- | --- |
| 1 | A crate of its own | **Decided** (2026-11-18). `tiffdown`, with `tiffdown-node` | Your call |
| 2 | Scope | Bytes in, pages out. Each directory of the file is a page: a size, a resolution, and a raster. With recognition, each page's words, and the invoice read from them | As the other parsers: its own format and nothing else |
| 3 | Decoder | Ours: a directory reader and a strip and tile reader in the crate, over codecs the workspace already has (section 3). Not the `tiff` crate | The codings a scanner or a fax machine writes are where a general decoder stops. `hayro-ccitt` already decodes the same fax coding for `pdfdown`, so the workspace keeps one such decoder and not two |
| 4 | Recognition | **Decided** (2026-11-18). `ocrdown`: a crate carved out of `pdfdown-ocr` that reads a raster and knows no format (section 5). `pdfdown-ocr`, `tiffdown`, and `jpegdown` use it | Otherwise the Tesseract half is written three times. `ooxmldown` is the same arrangement for the three Office crates |
| 5 | How recognition is switched on | A cargo feature `ocr` on `tiffdown`, off by default. The binding as `pdfdown-node` has it: one package without, one with | The library builds anywhere; only the `ocr` build needs Tesseract on the machine |
| 6 | Verbs without Tesseract | The page count, each page's facts, and a page as PNG | A model that reads pictures takes no TIFF, and a file of several pages has to be split for it |
| 7 | Invoice | `contentdown::invoice::extract_invoice` over the recognized pages | No new extraction code. It is what every other parser calls |
| 8 | New dependency | `weezl`, for LZW, as a direct dependency. It is already in the lock file by way of `lopdf`. Nothing else is added | `hayro-ccitt`, `flate2`, and `image` are in the workspace now |
| 9 | `mimedown` | `image/tiff` routes to `tiffdown` (`Parser::Tiffdown`). Neither crate depends on the other | `mimedown` reads a TIFF's header by ranges; `tiffdown` reads whole files. Camera raw types stay unrouted |
| 10 | Fixtures | None added by me. Tests build their files in code. Real files are made in scratch with `magick` and `tiffcp` and checked against `tiffinfo` | Your standing rule |

## 2. What exists to build on

- **`hayro-ccitt`** (workspace dependency). `pdfdown` uses it for `CCITTFaxDecode` (`crates/pdfdown/src/images.rs`, the `CcittRows` sink). A TIFF's Group 4 strip is the same bit stream as a PDF's with `K = -1`.
- **`flate2`** for Deflate, and **`image`** (`png`, `jpeg`) for a JPEG strip and for writing a page as PNG.
- **`pdfdown-ocr`** (`src/ocr.rs`): Tesseract over an `image::DynamicImage`, with the data path lookup, the thread cap, the resolution it tells Tesseract, orientation detection, and the retry over the turns. All of it is private to that crate and shares a file with the PDF cascade.
- **`contentdown`**: the page model (`PageLayout`, `Word`), the line builder, and invoice extraction.
- **`mimedown`** names a TIFF and reads its header facts: size, page count, coding, bits, resolution, software, date. Its directory reader (`src/container/ifd.rs`) is written for ranges and stays where it is.

## 3. Codings

In the order I would write them.

| Order | Coding (tag value) | With what |
| --- | --- | --- |
| 1 | CCITT Group 4 (4): one bit a sample, the scanned or faxed page | `hayro-ccitt` |
| 1 | Uncompressed (1) | nothing |
| 1 | LZW (5), with the horizontal predictor | `weezl` |
| 2 | PackBits (32773) | a short loop |
| 2 | Deflate (8 and 32946) | `flate2` |
| 2 | CCITT Group 3 (2 and 3) | `hayro-ccitt` |
| 3 | JPEG (7), with the tables the file keeps apart from the strips | `image` |
| Parked | Old-style JPEG (6) | Said to be unsupported, by name |

Samples: one bit, and 8 bits gray, palette, and RGB first; 16 bits and CMYK after. A page the crate cannot decode is answered for with its facts and a warning, not an error for the whole file.

For reference, the `tiff` crate at 0.11.3 (what `image` 0.25.10 takes behind its `tiff` feature): its README lists uncompressed, LZW, Deflate, PackBits, Group 4 (through the `fax` crate), and JPEG, and says nothing of Group 3 or old-style JPEG. Read on 2026-11-18, not run.

## 4. How the hard parts work

- **Pages.** Directories are chained through the file. A directory marked as a reduced copy or a mask of another is not a page. Pages of one file may differ in size and coding. The walk stops at a directory it has seen and at a cap.
- **Strips and tiles.** Each strip is coded on its own. Old writers leave out the strip byte counts, which then come from the offsets. Tiles are padded to their full size and cropped to the page.
- **One-bit pages.** The fill order tag says which end of a byte comes first, and the photometric tag says whether 0 is white or black. The raster handed on is gray with white as 255, whatever the file says.
- **Resolution.** The unit is inches, centimeters, or none. A fax can state a different resolution across than down (pixels that are not square); such a page is stretched to square pixels before it is recognized or written as PNG.
- **Orientation.** The tag's eight values are applied on decode. The recognition crate's own detection of the turn still runs after it.
- **Memory.** One page is decoded at a time and a one-bit page stays packed until it is needed. A file of many pages is never held decoded in full.
- **Limits.** A stated size is checked against the bytes that are there before anything is allocated for it.

## 5. Recognition: `ocrdown`

What moves out of `pdfdown-ocr/src/ocr.rs`: the data path lookup, the thread pool and its cap, the resolution setting, orientation detection, the upright pass and the retry over the turns, word recognition with boxes and confidence, and turning the boxes back. What stays: everything that knows a PDF (which pages need it, the one-bit composites, the render tier, merging with the page's own words).

The first step is a move with no change in behavior: `pdfdown-ocr`'s tests pass before and after.

What it takes and gives: a raster, its resolution, and a language in; the words with their boxes in pixels, their confidence, and the turn that made them upright out.

From a recognized page to `contentdown::PageLayout`: the page is `pixels × 72 / resolution` points each way; each word's box is scaled the same and flipped, since the page model counts y upward; `source` is `Ocr`; `content_rotation` is the turn.

One addition comes from the JPEG side: bringing a small raster up to a working resolution before it is read ([jpegdown.md](jpegdown.md), section 3).

## 6. Order of work

1. The directory reader: pages and their facts.
2. Group 4, uncompressed, and LZW to a raster; a page as PNG. Checked against `magick` on files made in scratch.
3. `ocrdown`, carved out of `pdfdown-ocr`.
4. The `ocr` feature: text, layout, and the invoice.
5. `tiffdown-node`, and the route in `mimedown`.
6. The rest of section 3.

### Step 1, as built (2026-11-19)

`Tiff::open(bytes)` reads the header and follows the chain of directories; `pages()` lists the pages, `warnings()` says what was wrong with a file that could still be read, and `byte_size()` its length.

- **A page** (`Page`) carries its number from 1, its size in pixels, its resolution in pixels to the inch (`Resolution`, across and down), its bits per sample and samples per pixel, its coding (`Compression`) and what a sample means (`Photometric`), each an enum whose `Other(n)` keeps a number it has no name for, its orientation from 1 to 8, the program that wrote it, and when. `size_in_points()` and `paper_size()` come from the pixels and the resolution, each side by its own, so a fax's pixels that are not square measure the right sheet.
- **What a directory leaves out** is what the format says: one bit, one sample, no compression, as it lies, the inch as the unit. What a sample means and the resolution have no default and are `None`.
- **Not a page:** a reduced copy or a mask, in the newer marking or the older, and a directory that states no size.
- **A damaged file:** a chain that leads back on itself is followed once; no more than 65,536 directories are followed, and no more than 2^22 entries are read over all of them (added the same day: a megabyte of directories laid over one another, each claiming 65,535 entries, took 23 seconds to read without it); a later directory past the end of the file keeps the pages before it; each of these is a warning, said once. A first directory that is not in the file is the one error (`malformed file`). A count no page could need (over 2^20 numbers, 64 fractions, 64 KiB of text) is not read, and every offset is checked against the end of the file.
- **Refused:** bytes that are not a TIFF, and BigTIFF by name (`unsupported format`).

Errors are `TiffError`, with the three categories the other parsers have.

Checked against `tiffdump` on every page of files made here in seven codings (uncompressed, LZW, Deflate, PackBits, JPEG, Group 3, Group 4; strips and tiles; both byte orders; inches and centimeters; a turned page; several pages; a pyramid of reduced copies) and of the real scanned and faxed files on this machine: size, coding, bits, samples, meaning, resolution, orientation, and date agree on all of them, with no warning and no error.

Test-first held, with one exception: three tests of the directory reader's values passed on their first run, because the reader they describe had been written for the tests of pages before them.

Not in this step: strips and tiles (where a page's pixels are), the fill order, extra samples, and anything decoded.

### Step 2, as built (2026-11-19)

`Tiff::raster(number)` gives a page's pixels as an `image::DynamicImage`, and `Tiff::png(number)` the same pixels as a PNG. Rows 3 and 8 of section 1 are answered by the manifest you created: our own reader, over `hayro-ccitt` and `weezl`.

- **Codings:** uncompressed, LZW (with samples kept as differences from their neighbors summed back), and CCITT Group 4. Each strip is decoded on its own; a strip's lengths, when an old writer left them out, run to the next strip or the end of the file; bytes written low end first are turned before they are decoded.
- **Samples:** one bit, eight bits of gray, eight bits that pick a color from the page's table, and color of three samples or four, where a fourth is kept only when it says how much of the pixel shows. One bit and gray both come out as gray with white at 255, whichever way round the file keeps them.
- **As it is to be looked at:** the page is turned the way its orientation says, and a fax whose pixels are not square (204 across, 98 down) has its sparser side stretched to match, each row given again.
- **Refused by name** (`unsupported format`): the codings not yet read (Deflate, PackBits, JPEG, Group 3, Modified Huffman), old-style JPEG and codings with no name, samples of other depths, gray with a second sample, and pages kept in tiles.
- **A damaged page** is `malformed file` and says which strip: one that does not lie within the file, holds fewer bytes than its rows need, or does not unpack to the rows it should hold; strips that stop short of the page; a table of colors that is not there. A page of more than 2^28 pixels is `limit exceeded` before anything is made for it, and what LZW unpacks to is bounded by what the strip's rows need.

Checked against ImageMagick, pixel for pixel: every page of files made here (Group 4 in one strip and in many, low end first, zero as black; LZW with and without differences, in both byte orders; uncompressed in one bit and in gray, in one strip and in many; several pages; a turned page) and every page of the real scanned and faxed files on this machine, which are Group 4 in one bit and uncompressed or LZW in color. All identical. A real page decodes in about 30 ms.

The Group 4 tests decode bits written out by hand from the coding's tables, so `hayro-ccitt` is shown to read a TIFF strip by a test and not only by real files.

Test-first held throughout this step.

### Step 4, as built (2026-11-19)

With the `ocr` feature, `src/ocr.rs` adds two methods to `Tiff` and one type:

- **`Tiff::page_layout(number, lang)`** reads one page and gives a `contentdown::PageLayout`: the words where they lie on the page as it is to be looked at, in points counted up from its foot, and the lines they make. `source` is `Ocr` and `rotate` is 0, since the page's orientation is already applied to its pixels.
- **`Tiff::recognize(lang, max_threads)`** reads every page, no more than `max_threads` at once, and gives a `Recognized`.
- **`Recognized`** holds the pages: `pages()` and `into_pages()` for the layout, `text_per_page()` for each page's lines from the top down, and `invoice(options)` for `contentdown::invoice::extract_from_pages` over them. One reading serves all three, so nothing is recognized twice.

What it decides:

- **The page's size** is the size its resolution states, with the sides changed over when its orientation lays it on its side. A fax with fewer rows to the inch than pixels across has the size its two resolutions state, and its picture is squared before it is read.
- **A page that states no resolution** is sized at 300 pixels to the inch and says so in a warning.
- **A page whose text reads another way up** is turned by the shared crate; `content_rotation` is the turn, the page's sides change over for a quarter turn, and a warning says which turn and what decided it.
- **A page whose picture cannot be decoded** (a coding not read yet, a damaged strip) is still in the list `recognize` gives, with its size, no words, and a warning that gives the reason. `page_layout` gives the error itself.
- **A language Tesseract has no data for** is said in a warning on every page. Before this, such a page came back with no words and nothing said, which looks the same as a blank page.
- **The invoice's warnings** begin with the pages' own, so an invoice read from some of a file's pages names the pages that were not read.

Text is the lines the words make, and not Tesseract's own running text: the same words, with no second pass over the page.

Checked on files made here from the six public invoice PDFs in `crates/pdfdown-node/__test__/fixtures/invoices`, each rendered to gray, cut to black and white, and written as Group 4, at 300 and at 200 pixels to the inch (nine pages each):

| | 300 | 200 |
| --- | --- | --- |
| Words `tiffdown` read, against the `tesseract` command on the same file (`--psm 6`) | 597 of 597 the same | 598 of 598 the same |
| Of the PDFs' own 580 words (`pdftotext`), found | 92.2% | 91.9% |
| A word's center against where `pdftotext -bbox` has it: each file's middle case, and the worst of all | 0.7 to 1.1 pt, 2.6 pt | 0.8 to 1.1 pt, 2.7 pt |
| Each page's size against the PDF page's | within half a point | within half a point |

The invoice read from each of the twelve files was set beside what `pdfdown` reads from the PDF it was made from, over seven fields (the invoice's number, its date, its total, the amount due, the vendor, the currency, and how many items): 76 of 84 agree. The eight that differ are all the vendor's name, in four of the six documents, where the name is in a logo or a heading that the scan carries as a picture.

A first set of files, made with `pdftoppm -mono`, read badly at 300 pixels to the inch. That switch dithers gray and color into dots, which a scanner does not do; the figures above are from files cut at a threshold.

Tests draw their words in a block face of five dots by seven, which Tesseract reads, so the tests that go through Tesseract need no file: a word found where it was drawn, a page kept on its side, a fax with half the rows, a page with no resolution, three pages read in order with one that cannot be decoded, the text, and the invoice's warnings.

Test-first held, with two things to say. The test for a fax with half the rows passed as soon as the plain page did, since a page's stated size was already right for it; it never failed on its own. And in `ocrdown`, the test that answers come back in order passed against the first version of `in_order`, which ran everything at once; only its companion, the cap, was seen to fail.

Not in CI yet: the tests and the clippy pass behind `ocr` need Tesseract, and no workflow builds `tiffdown` with the feature. The workflow for `tiffdown-node` (step 5) has to install Tesseract as the `pdfdown-node` one does, and run `cargo clippy -p tiffdown --all-targets --features ocr -- -D warnings` and `cargo test -p tiffdown --features ocr`.

Tesseract is run as `pdfdown-ocr` runs it, with the page taken as one block of text (the library's default, the command's `--psm 6`). The command's own default (`--psm 3`, which looks for columns first) found the same share of the PDFs' words on these files, to within 0.3 of a percentage point.

### Step 5, as built (2026-11-19)

**The base package, `@d0paminedriven/tiffdown`** (`crates/tiffdown-node/src/lib.rs` and `types.rs`):

- Functions, each with an `Async` twin: `tiffPages(buffer)`, `tiffPagePng(buffer, page)`, and `tiffDocument(buffer)`.
- The class `TiffDown`: the constructor and `TiffDown.openAsync(buffer)`, then `pageCount()`, `pages()`, `warnings()`, `pagePng(page)`, and `document()`, the last two with `Async` twins. The file's directories are read once; a page's pixels are decoded when asked for.
- A page's facts (`TiffPage`): its number, its size in pixels, its resolution, its samples, `compression` and `photometric` as unions keyed on `type` (with `other` carrying the format's number), its orientation, the program and the date, its size in points, and the paper it is nearest to (the same `PaperSize` union the Word package has).
- A page number has to be a whole number from 1. A fraction, `NaN`, and `Infinity` throw before any work is done, and are not cut down to a page that was not asked for.
- Errors keep the library's three openings: `unsupported format: …`, `limit exceeded: …`, and `malformed file: …`.

**The OCR package** (`src/ocr.rs` and `src/invoice.rs`, behind the `ocr` feature) adds, each with an `Async` twin: `tiffLayout`, `tiffTextPerPage`, `tiffInvoice`, and `tiffDocumentOcr`, and on the class `layout`, `textPerPage`, `invoice`, and `documentOcr`. `OcrOptions` is `lang` and `maxThreads`. `documentOcr` gives the facts, the layout, the text, and the invoice from one reading of the pages; the three verbs called apart read the pages three times, and their comments say so. `invoice.rs` is the file every binding carries, byte for byte.

**`mimedown`** names `tiffdown` as the package that reads `image/tiff` (`Parser::Tiffdown`, `'tiffdown'` in the binding's `Parser` union). A camera's raw file stays unrouted, which a test now pins. Both of the binding's declaration files were regenerated.

**Specs.** The repository holds no TIFF, so every public spec writes its files out by hand (`__test__/tiff-test.ts`), and the OCR spec draws its words in the block face the Rust tests use:

- `__test__/index.spec.ts`: 20 pass.
- `__test__/wasi.spec.ts`: 6 pass on the WebAssembly build, and it skips itself on the native one.
- `ocr/__test__/index.spec.ts`: 11 pass.
- Specs that read real files are kept out of the repository, in the ignored folders: one for each package. Both pass.

The public specs were seen to fail only as a whole, for want of an addon, before the binding was written; none of their assertions was watched failing on its own. The route in `mimedown` was test-first in the usual way, in the library and in the binding.

Not in the tree by my hand, because they are yours: the OCR package's `package.json`, the scripts that build and test it, the two lines of `.gitignore` it needs, the `ci` folder, the workflow, and the READMEs. Section 7 has what each needs.

## 7. Manifests

As they stand for steps 1 and 2 of section 6: both crates build with these today. They follow rows 3 and 8 (our own decoder, and `weezl`), which are still recommendations; a different answer to either changes the dependency list of `tiffdown`. Checked on 2026-11-19: both manifests, with empty sources, resolve and build in a scratch workspace against this root's dependency table. The ones under "When recognition arrives" are not checked.

### The root `Cargo.toml`

Three additions, each in its alphabetical place.

```toml
# [workspace] members
  "crates/tiffdown",
  "crates/tiffdown-node",

# [workspace.dependencies], third-party
weezl = "0.2.1"

# [workspace.dependencies], the workspace's own
tiffdown       = { path = "crates/tiffdown" }
tiffdown-node  = { path = "crates/tiffdown-node" }
```

### `crates/tiffdown/Cargo.toml`

```toml
[package]
description          = "Convenient tiff extraction"
edition.workspace    = true
keywords             = ["tiff", "fax", "scan", "extraction", "images"]
license              = { workspace = true }
name                 = "tiffdown"
repository.workspace = true
version              = "0.1.0"

[dependencies]
contentdown.workspace = true
flate2.workspace      = true
hayro-ccitt.workspace = true
image.workspace       = true
thiserror.workspace   = true
weezl.workspace       = true
```

What each is for: `hayro-ccitt` the fax codings, `weezl` LZW, `flate2` Deflate, `image` a JPEG strip and a page written as PNG, and `contentdown` the paper a page is nearest to and, later, the page model. `flate2` is not used until the second group of codings; it is listed now so the manifest is not touched again for it.

### `crates/tiffdown-node/Cargo.toml`

```toml
[package]
description            = "Convenient tiff extraction for Node"
edition.workspace      = true
keywords               = ["tiff", "fax", "scan", "extraction", "node-bindings", "rust"]
license                = { workspace = true }
name                   = "tiffdown-node"
publish                = false
repository.workspace   = true
rust-version.workspace = true
version                = "0.1.0"

[lib]
crate-type = ["cdylib"]

[dependencies]
contentdown.workspace = true
napi.workspace        = true
napi-derive.workspace = true
tiffdown.workspace    = true

[build-dependencies]
napi-build.workspace = true
```

### When recognition arrives (steps 3 and 4)

These follow row 5, a feature named `ocr`. They cannot go in before `ocrdown` exists, since a manifest that names a crate the workspace does not have does not resolve.

`crates/ocrdown/Cargo.toml`, with `"crates/ocrdown"` among the members and `ocrdown = { path = "crates/ocrdown" }` among the workspace's own:

```toml
[package]
description          = "Text from a picture with Tesseract: the words, their boxes, and the turn that reads them upright"
edition.workspace    = true
keywords             = ["ocr", "tesseract", "raster", "extraction"]
license              = { workspace = true }
name                 = "ocrdown"
repository.workspace = true
version              = "0.1.0"

[dependencies]
contentdown.workspace  = true
image.workspace        = true
rayon.workspace        = true
tesseract-rs.workspace = true
```

`tiffdown` then gains one dependency and its feature:

```toml
# [dependencies]
ocrdown = { workspace = true, optional = true }

[features]
ocr = ["dep:ocrdown"]
```

`tiffdown-node` passes the feature through, and `pdfdown-ocr` takes `ocrdown.workspace = true`:

```toml
[features]
ocr = ["tiffdown/ocr"]
```

### What the OCR package still needs

I added one thing to a manifest of yours, so that the code behind the feature could be compiled and tested: in `crates/tiffdown-node/Cargo.toml`,

```toml
[features]
ocr = ["tiffdown/ocr"]
```

The rest I have not touched. The OCR addon was built and tested with a `package.json` kept outside the tree, whose contents are these; it mirrors `crates/pdfdown-node/ocr/package.json`.

`crates/tiffdown-node/ocr/package.json`:

```json
{
  "name": "@d0paminedriven/tiffdown-ocr",
  "version": "0.1.0",
  "description": "Rust powered TIFF extraction for Node with OCR (requires system tesseract).",
  "license": "MIT",
  "repository": {
    "type": "git",
    "url": "git+https://github.com/DopamineDriven/assetdown.git"
  },
  "files": ["index.d.ts", "index.js", "omp.js", "tiffdown_ocr.napi.js", "README.md"],
  "type": "module",
  "os": ["darwin", "linux"],
  "main": "index.js",
  "publishConfig": {
    "access": "public",
    "registry": "https://registry.npmjs.org/"
  },
  "napi": {
    "binaryName": "tiffdown-node-ocr",
    "packageName": "@d0paminedriven/tiffdown-ocr",
    "targets": ["aarch64-apple-darwin", "x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"],
    "constEnum": false,
    "dtsHeader": "/* auto-generated by @d0paminedriven/tiffdown-ocr */\n\n"
  },
  "engines": {
    "node": ">=24"
  }
}
```

`crates/tiffdown-node/package.json`, among its scripts:

```json
"artifacts:ocr": "napi artifacts --package-json-path ocr/package.json --npm-dir ocr/npm",
"build:superset": "napi build --platform --esm --release --features ocr --package-json-path ocr/package.json --output-dir ocr --js tiffdown_ocr.napi.js",
"create-npm-dirs:ocr": "napi create-npm-dirs --package-json-path ocr/package.json --npm-dir ocr/npm",
"test:ocr": "yarn tsx --test \"ocr/__test__/**/*.spec.ts\""
```

Its keywords are still the ones `mimedown` has (`mime`, `sniffing`, `identification`).

`crates/tiffdown-node/.gitignore`:

- `index.js` there matches at any depth, so it also ignores `ocr/index.js`, which is written by hand (it loads `omp.js` before the addon). `/index.js` ignores only the generated one.
- `ocr/tiffdown_ocr.napi.js` is generated and is not ignored yet.

Already in `ocr/`, by my hand: `index.js`, `omp.js` (one OpenMP thread to a page being read, as the PDF package has it), `tsconfig.json`, and the spec. Generated there: `index.d.ts`.

For CI: a `ci` folder and a workflow as `pdfdown-node` has them, since the OCR package needs Tesseract installed. Its lint job also has to run `cargo clippy -p tiffdown --all-targets --features ocr -- -D warnings`, the same for `tiffdown-node`, and `cargo test -p tiffdown --features ocr`, which no workflow runs today. `version.sh` needs the package's name.

## Parked

- Old-style JPEG, JBIG, and BigTIFF.
- Writing TIFF.
- `mimedown` counts a TIFF's pages by following the chain, which from ranges is one fetch for each directory. A cap on that walk ("at least N pages") is a `mimedown` question, not this crate's.

## Not verified

- Group 3 and Modified Huffman through `hayro-ccitt`. Group 4 is verified (step 2); these two are not written yet.
- What the `tiff` crate does with a fax file. Its README and changelog were read; nothing was run.
