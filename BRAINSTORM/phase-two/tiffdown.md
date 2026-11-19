# tiffdown: notes

Status: **not started** (2026-11-18). These are notes toward a plan. Rows marked decided are decided; the rest are recommendations waiting for an answer. The crate and its binding are yours to create.

Many invoices, especially older ones, are TIFF files, and they should be read. `tiffdown` reads them. Its two companions have notes of their own: [rtfdown.md](rtfdown.md) and [jpegdown.md](jpegdown.md).

## 1. Decisions

| # | Question | Decided or recommended | Why |
| --- | --- | --- | --- |
| 1 | A crate of its own | **Decided** (2026-11-18). `tiffdown`, with `tiffdown-node` | Your call |
| 2 | Scope | Bytes in, pages out. Each directory of the file is a page: a size, a resolution and a raster. With recognition, each page's words, and the invoice read from them | As the other parsers: its own format and nothing else |
| 3 | Decoder | Ours: a directory reader and a strip and tile reader in the crate, over codecs the workspace already has (section 3). Not the `tiff` crate | The codings a scanner or a fax machine writes are where a general decoder stops. `hayro-ccitt` already decodes the same fax coding for `pdfdown`, so the workspace keeps one such decoder and not two |
| 4 | Recognition | **Decided** (2026-11-18). `ocrdown`: a crate carved out of `pdfdown-ocr` that reads a raster and knows no format (section 5). `pdfdown-ocr`, `tiffdown` and `jpegdown` use it | Otherwise the Tesseract half is written three times. `ooxmldown` is the same arrangement for the three Office crates |
| 5 | How recognition is switched on | A cargo feature `ocr` on `tiffdown`, off by default. The binding as `pdfdown-node` has it: one package without, one with | The library builds anywhere; only the `ocr` build needs Tesseract on the machine |
| 6 | Verbs without Tesseract | The page count, each page's facts, and a page as PNG | A model that reads pictures takes no TIFF, and a file of several pages has to be split for it |
| 7 | Invoice | `contentdown::invoice::extract_invoice` over the recognized pages | No new extraction code. It is what every other parser calls |
| 8 | New dependency | `weezl`, for LZW, as a direct dependency. It is already in the lock file by way of `lopdf`. Nothing else is added | `hayro-ccitt`, `flate2` and `image` are in the workspace now |
| 9 | `mimedown` | `image/tiff` routes to `tiffdown` (`Parser::Tiffdown`). Neither crate depends on the other | `mimedown` reads a TIFF's header by ranges; `tiffdown` reads whole files. Camera raw types stay unrouted |
| 10 | Fixtures | None added by me. Tests build their files in code. Real files are made in scratch with `magick` and `tiffcp` and checked against `tiffinfo` | Your standing rule |

## 2. What exists to build on

- **`hayro-ccitt`** (workspace dependency). `pdfdown` uses it for `CCITTFaxDecode` (`crates/pdfdown/src/images.rs`, the `CcittRows` sink). A TIFF's Group 4 strip is the same bit stream as a PDF's with `K = -1`.
- **`flate2`** for Deflate, and **`image`** (`png`, `jpeg`) for a JPEG strip and for writing a page as PNG.
- **`pdfdown-ocr`** (`src/ocr.rs`): Tesseract over an `image::DynamicImage`, with the data path lookup, the thread cap, the resolution it tells Tesseract, orientation detection and the retry over the turns. All of it is private to that crate and shares a file with the PDF cascade.
- **`contentdown`**: the page model (`PageLayout`, `Word`), the line builder and invoice extraction.
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

Samples: one bit, and 8 bits gray, palette and RGB first; 16 bits and CMYK after. A page the crate cannot decode is answered for with its facts and a warning, not an error for the whole file.

For reference, the `tiff` crate at 0.11.3 (what `image` 0.25.10 takes behind its `tiff` feature): its README lists uncompressed, LZW, Deflate, PackBits, Group 4 (through the `fax` crate) and JPEG, and says nothing of Group 3 or old-style JPEG. Read on 2026-11-18, not run.

## 4. How the hard parts work

- **Pages.** Directories are chained through the file. A directory marked as a reduced copy or a mask of another is not a page. Pages of one file may differ in size and coding. The walk stops at a directory it has seen and at a cap.
- **Strips and tiles.** Each strip is coded on its own. Old writers leave out the strip byte counts, which then come from the offsets. Tiles are padded to their full size and cropped to the page.
- **One-bit pages.** The fill order tag says which end of a byte comes first, and the photometric tag says whether 0 is white or black. The raster handed on is gray with white as 255, whatever the file says.
- **Resolution.** The unit is inches, centimeters or none. A fax can state a different resolution across than down (pixels that are not square); such a page is stretched to square pixels before it is recognized or written as PNG.
- **Orientation.** The tag's eight values are applied on decode. The recognition crate's own detection of the turn still runs after it.
- **Memory.** One page is decoded at a time and a one-bit page stays packed until it is needed. A file of many pages is never held decoded in full.
- **Limits.** A stated size is checked against the bytes that are there before anything is allocated for it.

## 5. Recognition: `ocrdown`

What moves out of `pdfdown-ocr/src/ocr.rs`: the data path lookup, the thread pool and its cap, the resolution setting, orientation detection, the upright pass and the retry over the turns, word recognition with boxes and confidence, and turning the boxes back. What stays: everything that knows a PDF (which pages need it, the one-bit composites, the render tier, merging with the page's own words).

The first step is a move with no change in behavior: `pdfdown-ocr`'s tests pass before and after.

What it takes and gives: a raster, its resolution and a language in; the words with their boxes in pixels, their confidence, and the turn that made them upright out.

From a recognized page to `contentdown::PageLayout`: the page is `pixels × 72 / resolution` points each way; each word's box is scaled the same and flipped, since the page model counts y upward; `source` is `Ocr`; `content_rotation` is the turn.

One addition comes from the JPEG side: bringing a small raster up to a working resolution before it is read ([jpegdown.md](jpegdown.md), section 3).

## 6. Order of work

1. The directory reader: pages and their facts.
2. Group 4, uncompressed and LZW to a raster; a page as PNG. Checked against `magick` on files made in scratch.
3. `ocrdown`, carved out of `pdfdown-ocr`.
4. The `ocr` feature: text, layout and the invoice.
5. `tiffdown-node`, and the route in `mimedown`.
6. The rest of section 3.

## Parked

- Old-style JPEG, JBIG, and BigTIFF.
- Writing TIFF.
- `mimedown` counts a TIFF's pages by following the chain, which from ranges is one fetch for each directory. A cap on that walk ("at least N pages") is a `mimedown` question, not this crate's.

## Not verified

- `hayro-ccitt` on a TIFF strip. It is proven here on PDF streams only.
- What the `tiff` crate does with a fax file. Its README and changelog were read; nothing was run.
