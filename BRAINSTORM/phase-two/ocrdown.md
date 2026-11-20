# ocrdown: notes

Status: **steps 1 and 2 of section 3 are built** (2026-11-19): the move out of `pdfdown-ocr`, with no change in what it reads, and what `tiffdown`'s `ocr` feature needed. The "as built" notes under section 3 say what exists.

`ocrdown` reads text from a picture with Tesseract and knows no file format. It is the half of `pdfdown-ocr` that never looks at a PDF, moved out so that `pdfdown-ocr`, `tiffdown`, and `jpegdown` share one copy. Decided on 2026-11-18 ([tiffdown.md](tiffdown.md), row 4).

## 1. Manifests

### The root `Cargo.toml`

Two additions, each in its alphabetical place. Nothing third-party is new: `tesseract-rs`, `image`, `rayon`, and `contentdown` are all in the workspace already.

```toml
# [workspace] members
  "crates/ocrdown",

# [workspace.dependencies], the workspace's own
ocrdown        = { path = "crates/ocrdown" }
```

### `crates/ocrdown/Cargo.toml`

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

What each is for: `tesseract-rs` the recognition, `image` the picture it is handed, `rayon` the pool that caps how many pages are read at once, and `contentdown` the word a recognized word becomes once it is placed on a page.

Checked on 2026-11-19: this manifest, with a source that touches each of the four, resolves and builds in a scratch workspace against this root's dependency table, on this machine, which has Tesseract and Leptonica installed.

### `crates/pdfdown-ocr/Cargo.toml`

One line is added when the move begins:

```toml
ocrdown.workspace = true
```

When the move is done, `pdfdown-ocr` no longer calls Tesseract itself, and its own `tesseract-rs.workspace = true` can go. I will say so when that is true and not before.

### `tiffdown` and `tiffdown-node`

The lines they gain for the `ocr` feature are in [tiffdown.md](tiffdown.md), section 7, under "When recognition arrives". They come after this crate exists.

## 2. What moves, and what stays

All of it is in `crates/pdfdown-ocr/src/ocr.rs` today, private to that crate.

Moves to `ocrdown`:

- Where Tesseract's language data is, found once.
- The thread cap and the pool it sizes.
- The resolution a picture is read at, and telling Tesseract so.
- Reading a picture: its text, or its words with each word's box in pixels and its confidence.
- Which way up the text is: the upright pass, Tesseract's own detector, and the retry over the other turns; and turning a box found in a turned picture back onto the picture as it was given.
- Placing recognized words on a page: from pixels, counted down from the top, to points counted up from the bottom.

Stays in `pdfdown-ocr`: everything that knows a PDF. Which pages need reading at all, the one-bit pictures of a page put together into one, the render tier, merging recognized words with a page's own, and the turn a whole page takes when its pictures disagree.

## 3. Order of work

1. **The move, with no change in behavior.** `pdfdown-ocr` calls `ocrdown` for what it used to do itself. Its Rust tests and the OCR package's spec pass before and after, and the text it reads from the public OCR fixtures is the same, word for word.
2. **`tiffdown`'s `ocr` feature**, on top of it: a page's raster in, its words on the page out, then the layout and the invoice.
3. **A working resolution** for a small picture ([jpegdown.md](jpegdown.md), section 3), as an option, when `jpegdown` needs it.

### Step 1, as built (2026-11-19)

Four modules, each with the tests of what it holds:

- **`engine`**: `normalize_max_threads`, `pool` (the pool for a given cap, made once and kept), and where Tesseract's language data is.
- **`resolution`**: `source_ppi` and `DEFAULT_SOURCE_PPI`, and telling Tesseract the resolution.
- **`turn`**: the detector, the thresholds, the order the turns are tried in, `OrientationSource` (who decided), and `unrotate_pixel_box`.
- **`read`**: `OcrWord`; `recognize_text` (a picture's text, the way up it reads); `recognize_words` (its words as it is given); `recognize_words_upright`, which returns `Upright` (the words, the turn, the turned picture's size, and who decided); and `place_words` (from pixels onto a rectangle in points).

The bodies are the ones `pdfdown-ocr` had, moved and not rewritten. What changed is names and one return type: `get_ocr_pool` is `pool`, `ocr_dynamic_image` is `recognize_text`, `ocr_dynamic_image_words` is `recognize_words`, `map_ocr_words` is `place_words`, and `ocr_words_best_orientation` is `recognize_words_upright`, whose four-part tuple became the struct `Upright`. `pdfdown-ocr` calls them and still exports `normalize_max_threads` under its own name.

Stayed in `pdfdown-ocr`, as section 2 has it, and also the turning of a box within a page (`turn_box`, `turn_layout`) and the vote among a page's pictures (`dominant_rotation`): those are about a page that holds several pictures, which only a PDF does.

Checked:

- The 25 tests `pdfdown-ocr` had are all still there and pass: 8 moved with what they test, 17 stayed.
- What the OCR package reads from eight public PDFs (30 pages, two of them turned a quarter by the detector) was written out before the move and after it: the text, the placed words, and the structured text are the same, byte for byte.
- The OCR package's public spec passes, 19 of 19.
- Clippy is clean with and without `render`.

`pdfdown-ocr` no longer calls Tesseract itself, so the line `tesseract-rs.workspace = true` in its manifest can go. I added `ocrdown.workspace = true` to that manifest for the move, as section 1 has it.

The six workflows whose lint job has no Tesseract now leave `ocrdown` out as they leave out `pdfdown-ocr`, and the `pdfdown-node` workflow, which installs Tesseract and covers the whole workspace, also starts when `crates/ocrdown` changes.

### Step 2, as built (2026-11-19)

`tiffdown`'s side is in [tiffdown.md](tiffdown.md), under "Step 4, as built". Three things were added here, because `jpegdown` will need the same three:

- **`recognize_page(PagePicture, lang)`**, in a new module `page`: a picture that is a whole page, its number, and its size in points go in; a `contentdown::PageLayout` comes out, with the words placed, the lines made, the turn recorded, and a warning when the page was turned. The format crates no longer place words themselves.
- **`in_order(max_threads, items, read)`**, in `engine`: what `read` makes of each item, in the items' order, with no more being read at once than the cap allows. A format crate reads its pages through it and needs no `rayon` of its own.
- **`ready(lang)`**, in `engine`: whether Tesseract starts here with a language. The reading functions give no words when it does not, which looks the same as a blank page; `recognize_page` asks first and says so in a warning. `pdfdown-ocr` does not call it, so nothing changes there.

`pdfdown-ocr` is untouched by this step: its 17 tests pass, and none of the functions it calls changed.

## 4. What it needs on a machine

Tesseract and Leptonica, linked from the system, exactly as `pdfdown-ocr` needs them now. Two consequences:

- **CI.** Every workflow's lint job runs clippy and the tests over the whole workspace and leaves out `pdfdown-ocr`, because the runner has no Tesseract. `ocrdown` has to be left out the same way (`--exclude ocrdown`) in each of them, or every package's lint job fails the day the crate is added. I will make that change in the workflows with the move.
- **Features stay off by default.** `tiffdown` takes `ocrdown` only behind its `ocr` feature, so a build of `tiffdown` or of the workspace's other crates still needs no Tesseract.

## Not verified

- The workflows have not run on GitHub since the change.
- `pdfdown-ocr` gives no words, and says nothing, for a language Tesseract has no data for. `ready` could tell its callers; that would be a change in what `pdfdown-ocr` answers, so it is left for you to ask for.
