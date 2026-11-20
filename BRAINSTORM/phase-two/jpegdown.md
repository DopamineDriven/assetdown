# jpegdown: notes

Status: **built** (2026-11-19): the library, its `ocr` feature, the Node binding with its OCR package, and the route in `mimedown`. Section 6 says what exists. The rows of section 1 that were recommendations were followed as written, since you were away; say so where you would have chosen otherwise.

Invoices also arrive as JPEG files: a picture of one page. `jpegdown` reads them. It shares its recognition with `tiffdown`; see [tiffdown.md](tiffdown.md), section 5. The third companion is [rtfdown.md](rtfdown.md).

## 1. Decisions

| # | Question | Decided or recommended | Why |
| --- | --- | --- | --- |
| 1 | A crate of its own | **Decided** (2026-11-18). `jpegdown`, with `jpegdown-node` | Your call |
| 2 | Scope | One picture is one page. Bytes in; the page's facts and the picture upright; with recognition, its words and the invoice read from them | As the other parsers |
| 3 | Decoder | `image`'s, which the workspace already builds with `jpeg`. The crate writes no JPEG decoding of its own | It reads baseline and progressive files, gray and color, today. CMYK is to be checked |
| 4 | Recognition | `ocrdown` (**decided** 2026-11-18), behind an `ocr` feature, as `tiffdown` has it | One Tesseract half for the three crates |
| 5 | Small pictures | A raster under a floor of resolution is enlarged before it is read (section 3). It lives in the shared crate, as an option this crate turns on | A page at screen resolution reads badly as it comes and well once enlarged |
| 6 | Page size | From the resolution the file states, when that makes a page of a believable size. Otherwise the picture is taken to be a sheet of the paper its shape is nearest to | The page model is in points and a picture states pixels. A stated 72 or 96 is often a default and not a measurement |
| 7 | Invoice | `contentdown::invoice::extract_invoice` over the recognized page | No new extraction code |
| 8 | `mimedown` | `image/jpeg` routes to `jpegdown` (`Parser::Jpegdown`). Neither crate depends on the other | `mimedown` already reads the header facts |
| 9 | New dependency | None | |
| 10 | Fixtures | None added by me. Real files are made in scratch by rendering the public invoice PDFs to JPEG, which also gives the right answer: the PDF's own words | Your standing rule |

Said plainly: without recognition this crate has little to give that `mimedown` does not already say. Its own part is small, and most of the work is in the shared crate.

## 2. What exists to build on

- **`image`** 0.25.10: the JPEG decoder, the EXIF orientation it reports (`ImageDecoder::orientation`) and `DynamicImage::apply_orientation`, and `imageops::resize`.
- **`mimedown`** reads a JPEG's size, stated resolution, orientation, coding, and color from its header.
- **`pdfdown-ocr`** and **`contentdown`**, as [tiffdown.md](tiffdown.md) section 2 has them.

## 3. Small pictures

Tesseract is built for pages near 300 pixels an inch. `pdfdown-ocr` tells it the resolution of what it is given and does not resample.

Measured on 2026-11-18 with Tesseract 5.5.1 on the six PDFs in `crates/pdfdown-node/__test__/fixtures/invoices`: the first page of each rendered at 100 pixels an inch as JPEG (`pdftoppm -r 100 -jpeg`), read as rendered and then enlarged with a Lanczos filter, and compared with the PDF's own words (`pdftotext`).

| Raster | Of the PDF's words, found | Of the words read, at confidence 80 or more |
| --- | --- | --- |
| As rendered | 88% | 76% |
| Enlarged twice | 98% | 94% |
| Enlarged three times | 91% | 93% |

So a small raster is brought to about 200 pixels an inch and no further. `image::imageops::resize` does it, with no new crate.

Open: where the floor sits, and which filter. Whether `pdfdown-ocr` should do the same for a small picture inside a PDF is a separate question, to be measured before its answers change.

## 4. The rest

- **Orientation.** The EXIF value is applied on decode. The shared crate's own detection of the turn still runs after it.
- **Words on the page.** As [tiffdown.md](tiffdown.md) section 5: boxes scaled from pixels to points and flipped.
- **What it does not read.** Lossless and 12-bit JPEG, and JPEG-LS, are said to be unsupported, by name.

## 5. Order of work

1. After `ocrdown` exists ([tiffdown.md](tiffdown.md), section 6, step 3): decode, the page's facts, the picture upright.
2. The working resolution in the shared crate.
3. The `ocr` feature: text, layout, and the invoice.
4. `jpegdown-node`, and the route in `mimedown`.

## 6. As built (2026-11-19)

**The library** (`crates/jpegdown`):

- `Jpeg::open(bytes)` reads the header: every segment up to the picture data. `page()` gives what the file says: the size in pixels, the resolution (JFIF's density first, then Exif's), the depth, the components and what they are, the coding, the orientation, the program, and the date.
- `raster()` gives the picture turned the way its orientation says, and `png()` the same as a PNG. `image` decodes. A picture kept without loss, one with samples of more than 8 bits, and the arithmetic codings are refused by name; the page is still listed.
- `Page::size_as_read()` is row 6: the size the stated resolution makes, when a sheet could have it (sides from 2 to 18 inches). A resolution of 72 or 96 is believed only when it makes a standard sheet. Otherwise the picture is taken to be the sheet its shape is nearest to, among letter, A4, and legal, and the answer names it.
- With `ocr`: `Jpeg::recognize(lang)` gives a `Recognized`, with `page()`, `text()`, and `invoice(options)`. A size that was assumed, an enlarging, and a turn are each said in the page's warnings, and the invoice's warnings begin with them.

**The binding** (`crates/jpegdown-node`): `jpegPage`, `jpegPng`, and `jpegDocument`, and the class `JpegDown`, in the base package; `jpegLayout`, `jpegText`, `jpegInvoice`, and `jpegDocumentOcr`, with the same four on the class, in the OCR package. Each has an `Async` twin.

**`mimedown`** names `jpegdown` as the package that reads `image/jpeg`.

**Checked** on the six public invoice PDFs, first page of each, rendered in gray as JPEG at four resolutions, against the PDFs' own words (`pdftotext`) and what `pdfdown` reads from them:

| Pixels to the inch | Words found | A word's center, the middle case and the worst | Invoice fields that agree |
| --- | --- | --- | --- |
| 100 | 89.4% | 1.1 pt, 2.4 pt | 34 of 36 |
| 150 | 92.7% | 1.0 pt, 2.1 pt | 35 of 36 |
| 200 | 91.5% | 0.9 pt, 2.7 pt | 35 of 36 |
| 300 | 91.8% | 0.9 pt, 2.8 pt | 35 of 36 |

Every page's size came out within a point and a half of the PDF page's.

**Tests.** 25 in the library and 6 more behind `ocr`; the binding's specs are 17 for the base package, 6 on the WebAssembly build, and 10 for the OCR package. The specs start from two small JPEGs kept as literals, since the repository holds none. Test-first held in the library; the binding's specs were seen to fail only for want of an addon.

**What I made that is usually yours**, because the binding's folder held a manifest and nothing else: its `package.json`, `ocr/package.json`, `build.rs`, `.gitignore`, and the rest of its scaffold, each a copy of `tiffdown-node`'s with the names changed; its `ci` folder and workflow; and its entry in `version.sh`. Change or replace any of it.

## Parked

- A photograph of paper, as against a scan: perspective, shadow, a curled sheet.
- Other picture types that hold an invoice (PNG, WebP, HEIC). Each would be this crate again with another decoder. Whether they get crates of their own or share one is a question for the day one turns up.

## Not verified

- CMYK and the Adobe marker's inverted samples through `image`: still not run.
- A photograph of a sheet, as against a scan, is expected to read poorly: nothing here corrects perspective or shadow. It is the first item under Parked.
- No spec reads real files yet.
- The measurement in section 3 is six pages from clean renders. A scan has noise that a render does not.
