# mimedown: plan

Status: **draft.** Nothing is built and no snippet here has been compiled. Names of functions and types are proposals.

`mimedown` takes over the part of `@d0paminedriven/metadata` that looks at bytes and says what they are: the sniffing and the facts. The part that fetches stays in that package.

What it has to return is fixed by where the answer goes: the four metadata tables in `~/cloneathon/t3-chat-clone/packages/db/prisma/schema/attachmentmeta.prisma` (`ImageMetadata`, `VideoMetadata`, `AudioMetadata`, `DocumentMetadata`). You are folding each table and its `Original…` twin into one; the columns stay largely as they are. Section 13 goes through them column by column.

Research: I read `@d0paminedriven/metadata` at version 6.0.2 (`~/personal/d0paminedriven/packages/metadata`, about 10,900 lines of source outside its tests) and ran its built image reader on three headers made by hand. Paths below that start with `src/` are in that package. It is a read-only reference here.

## 1. Decisions

Rows 1 to 6 are yours (2026-11-17), and row 13 is what you asked for. The rest are my recommendations and wait for your answer. The ones that shape the most code are 8, 10, 12 and 16.

| # | Question | Decided or recommended | Why |
| --- | --- | --- | --- |
| 1 | Name | **Decided.** Crates `mimedown` and `mimedown-node`; npm `@d0paminedriven/mimedown` | Succinct, and it says what the package tells you |
| 2 | One crate, or one for each top-level type | **Decided.** One crate. `image`, `application`, `audio`, `video` and `text` are modules in it | The sniff comes before the type is known, and the code is shared by container, not by top-level type (section 3) |
| 3 | Scope | **Decided.** Bytes in, facts out. No fetching and no probing: `Extract` in the TypeScript package keeps both and hands the bytes over | It is also what lets the same code run as WebAssembly, which has no sockets |
| 4 | Image coverage | **Decided.** At least what the TypeScript package covers: PNG and APNG, JPEG, GIF, BMP, WebP, AVIF, HEIC, SVG, ICO, TIFF. I would add: every other member of the schema's `ImageFormat` is at least named (section 8.1) | It replaces that package for this job |
| 5 | A MIME type as an argument | **Decided.** Never | Finding the type is the reason to call |
| 6 | What parity means | **Decided.** The columns of the four tables (section 13). I would add: under the field names the TypeScript package and the two media packages already use, and not the same answer where today's answer is wrong; section 8 lists every difference | Three of today's answers were confirmed wrong by running the package (section 2) |
| 7 | Verbs | `sniff` names the type from as few bytes as that takes. `specs` returns the row. The extension tables come with them | A caller that only routes to a parser wants the first and nothing more |
| 8 | Shape of the answer | A union keyed on `type` (`IMAGE`, `DOCUMENT`, `AUDIO`, `VIDEO`, `ARCHIVE`, `UNKNOWN`): the four tables and what has none. Each member is a union keyed on `format` and carries `kind`, `format`, `ext` and `mime`, as `audiodown` and `videodown` do | One flat `DocSpecs` puts `pdfVersion` on a docx, and a `metadata: T` whose `T` the caller picks is checked by nothing |
| 9 | Bytes it does not recognize | `UNKNOWN` is an answer, not an error. A file it recognizes and cannot finish reading returns what it found, with `warnings` | As the other crates do: one bad file should not stop a batch |
| 10 | Part of a file | `specs` accepts ranges of a file, each with its offset, and names the further range it needs when the facts lie outside them | A TIFF's directory and a zip's are wherever the file put them. Joining a head to a tail, as `Extract` does now, makes every offset into the tail wrong |
| 11 | Text | Bytes cannot tell Markdown from a Rust source file. A file name is accepted as a hint and used for text only, and the answer says what decided it | The one case where the name knows more than the bytes |
| 12 | Audio and video | Through `audiodown` and `videodown`, the same way as row 13. They already return every column of their tables except the waveform and the thumbnails, which stay their own verbs. A type they do not read is named, with its facts null | One call then fills any of the four tables. Say if you would sooner keep these two as names only |
| 13 | PDF | **Asked for by you.** `pdfdown` is a dependency, and from the whole file it fills the PDF columns (section 14). From part of a file: the version, a linearized file's page count, and `needs` naming the rest | Reading a PDF by pattern over its raw bytes is where the TypeScript package is weakest; its README opens with a note to fix exactly that |
| 14 | Office files | The type from a zip's entry names alone: docx, xlsx, pptx, the three OpenDocument types and EPUB. The columns from `docxdown`, `xlsxdown` and `pptxdown`, as row 13 | Names are stored uncompressed, so naming the type inflates nothing; the parsers already count pages, words, pictures and links |
| 15 | Which string, when a type has several in use | The one the workspace already emits: `audio/wav`, `video/x-matroska`, `image/x-icon`. The tables accept the others on the way in | Three packages already agree on these |
| 16 | How the parsers are depended on | Each behind a cargo feature of `mimedown`, all of them on in the npm package. The dependency runs one way: no parser may depend on `mimedown` (section 14). Each answer still names the package that reads the type in full | The weight is opt-in for a Rust consumer that only sniffs, and the caller can still route |
| 17 | Dependencies | Always: `flate2`, `roxmltree`, `ooxmldown`, `thiserror`. Behind features: `pdfdown`, `docxdown`, `xlsxdown`, `pptxdown`, `audiodown`, `videodown`. No third-party crate is added | Compressed PNG chunks, XMP packets, Office packages; then the six parsers |
| 18 | WebAssembly | Yes | It is what `ExtractClient` exists for today |
| 19 | The JS API | A class `MimeDown` holding the bytes, with `sniff()` and `specs()` and their `Async` twins, plus free functions for a one-off call | As the family. The binding is yours to configure |
| 20 | Version | Start at `0.1.0`; `version.sh` gains a `mimedown` target | As the others |
| 21 | Spellings | The schema's, where it differs from the TypeScript package: `grayscale_alpha`, not `grayscale-alpha` | A row can be written without a mapping. It is the one such difference I found |
| 22 | Camera and place | `cameraMake`, `cameraModel`, `lensModel`, `gpsLat` and `gpsLon` are read from EXIF | The tables have the columns and the TypeScript package fills none of them |
| 23 | `dominantColorHex` | Not part of `specs`: it takes the pixels, not the header. A verb of its own for the formats the `image` crate decodes, if you want it in this package at all | The TypeScript package does not fill it either |
| 24 | `heic-rs`, which you pointed at | Not a dependency of `mimedown`. The reference to read beside the specification, and a dev-dependency to check our HEIC answers against if you allow one. Its real use is decoding, which is parked | Section 7.2: the walk has to be written for AVIF either way, and it is the same walk |
| 25 | `mime`, which you pointed at, and its neighbors `mime_guess` and `infer` | Not dependencies. `mime` (0.3.18, no dependencies) parses and holds a MIME string; it neither reads bytes nor maps extensions. `mime_guess` is an extension table and `infer` names a type from its first bytes: the nearest to `sniff`, and at most something to check our answers against | The types `mimedown` returns are a closed set, so they are an enum: every `match` is checked and JavaScript gets a union of literals. A `Mime` is any string that parses |

## 2. The TypeScript package today

| Part | Where | Lines | What it does |
| --- | --- | --- | --- |
| Images | `src/images/` | 2,548 | One class for each format, chained by inheritance from `ImgMetadataExtractor` down through PNG, JPEG, WebP, AVIF, HEIC, TIFF, GIF, ICO, SVG and BMP to the MIME tables |
| Documents | `src/docs/`, less the tables | 5,602 | PDF, Office, RTF and text. About 4,400 of the lines read PDFs by pattern over the raw bytes, in two parallel implementations (`pdf/obj-map.ts`, and the chain that ends in `pdf/alt.ts`) |
| Fetching | `src/extract/index.ts` | 881 | Probe, range requests, deadlines, an idle guard, quarantine, origin fallback. Stays |
| Browser entry | `src/extract/client.ts` | 252 | A sniff of fifteen types in front of the same readers |
| Tables | `src/docs/mime-workup.ts` | 439 | 133 extensions and 126 types, each way |

The same question, "what is this", is answered in four places, four different ways:

| Where | How |
| --- | --- |
| `Extract.sniffMime` (`src/extract/index.ts:100`) | Five signatures: PNG, JPEG, WebP, GIF, PDF |
| `ExtractClient.detectMimeType` (`src/extract/client.ts:27`) | Fifteen. Office files are told apart by searching the first 4,096 bytes for `word/`, `xl/` and `ppt/` |
| `ImgMetadataExtractor.getImageSpecsWorkup` (`src/images/index.ts`) | Ten image signatures, tried in a fixed order |
| `DocMetadataExtractor.getDocumentSpecsWorkup` (`src/docs/index.ts:617`) | Believes `mime`, then the file name |

### The MIME argument

- `getDocumentSpecsWorkup(buffer, mime, filename?)` needs the type it is being asked to find. A zip is a docx only if `mime` or the file name says so (`src/docs/index.ts:659`); otherwise it comes back as format `bin`. Text is read as text only when `mime` is a key of the table (`:674`).
- `Extract.extractRemote(buffer)` knows five types. Any other buffer has no type, goes to the documents side and is called a PDF (`src/extract/index.ts:671`). That is every TIFF, BMP, HEIC, AVIF, SVG and ICO, and every Office file.
- For a URL, the server's `Content-Type` is used first and the bytes only when there is none (`:368`), under a comment that says to avoid trusting headers.

### Confirmed by running the built package

On headers made by hand:

- **An extended WebP of 1024 × 768 is reported as 1 × 1.** Each dimension is read only when all three of its bytes are nonzero (`src/images/webp-workup.ts:22`, `:25`), which no image under 65,537 pixels a side satisfies. The extended form (`VP8X`) is the one a WebP takes when it has a color profile, EXIF, XMP, animation, or alpha beside lossy data.
- **A HEIC throws "Not an AVIF file".** The AVIF check and the HEIC check are the same test, `ftyp` at offset 4. AVIF is asked first (`src/images/index.ts:32`) and its reader throws on the brand before HEIC has its turn. An MP4 or a MOV passes the same test.
- **A bottom-up BMP, the ordinary kind, reports EXIF orientation 6**, a quarter turn (`src/images/bmp-workup.ts:20`).

### Read in the source, not run

- **JPEG.** `exifDateTimeOriginal` is looked for in the first directory only; `src/images/workup.ts:295` notes the pointer it does not follow. That tag is kept in the Exif sub-directory, so it will rarely be found.
- **TIFF.** A tag whose value is zero is taken for unreadable (`src/images/tiff-workup.ts:142`, `:152`), so a WhiteIsZero image has no color model. BigTIFF throws (`:13`). The reader writes to `console.warn`.
- **PNG.** Reading stops at the first `IDAT` (`src/images/png-workup.ts:574`), though text, time and EXIF chunks may follow it. `xmpToolkit` is never filled: `match` with a global pattern returns whole matches, not the group (`:69`).
- **AVIF and HEIC.** `colorSpace` is set to `"rgb"`, which is not a member of its union; an assertion lets it through (`src/images/avif-workup.ts:30`, `src/images/heic-workup.ts:38`). Both take the first `ispe` box they find as the image's size. The two readers are near copies of each other.
- **SVG.** The size is the first `width=` and `height=` in the first kilobyte, whatever element they are on; `stroke-width="2"` matches the pattern.
- **XMP** is read from PNG, AVIF and HEIC only, by regular expression.
- **`Extract`.** For a PDF or a TIFF it fetches the head and the tail and joins them (`src/extract/index.ts:456`), so an offset that points into the tail lands in the wrong place. On the origin fallback, `reportedTotalBytes` and `fetchedBytes` each hold the other's value (`:350`).
- **Types.** Results are asserted (`as ImageSpecs<T>`, with `T` chosen by the caller). That is how `"rgb"` above went unnoticed.

## 3. What tells one type from another

| Evidence | Types | Bytes it takes |
| --- | --- | --- |
| A signature at a fixed offset | PNG, JPEG, GIF, BMP, ICO, TIFF, PDF, RTF, gzip, 7z, RAR, FLAC, Ogg, MP3, WebAssembly, fonts | Under 16 |
| ISO base media: `ftyp` and its brands | AVIF, HEIC, HEIF, MP4, MOV, M4A, 3GP | The first box, usually under 64 |
| RIFF: the form type at offset 8 | WebP, WAV, AVI | 12 |
| EBML: `DocType` | WebM, Matroska | Under 64 |
| Zip: entry names | docx, xlsx, pptx, OpenDocument, EPUB, JAR, a plain zip | The first name from the head; all of them from the directory at the end |
| Compound file (OLE) | doc, xls, ppt, msi | 8 for the container; its directory to tell them apart |
| XML: the root element | SVG, XHTML, plain XML | Up to the first start tag |
| Text: what decodes, and how it begins | JSON, HTML, scripts with a `#!` line, and everything the file name has to settle | A sample |

The code follows these rows, not the top-level types. The box walk that finds a HEIC's size is the one that names an MP4, and the RIFF reader serves an image, an audio type and a video type. That is the reason for one crate.

Order: exact signatures first, containers next, text last. No type is offered to a loose test before a stricter one could have claimed it, which is the fault behind the HEIC result above.

## 4. Target layout

```text
crates/
├─ ooxmldown/            unchanged
├─ mimedown/             new. No napi
│  └─ src/
│     ├─ lib.rs          sniff(), specs()
│     ├─ error.rs
│     ├─ media_type.rs   every type it can name: its string, extensions, top-level type, and which parser reads it
│     ├─ tables.rs       extension to type and back: the 133 extensions of the TypeScript tables
│     ├─ source.rs       ranges of a file with their offsets, bounded reads, and what is missing
│     ├─ sniff.rs        signatures, then containers, then text
│     ├─ container/      isobmff.rs  riff.rs  ebml.rs  zip.rs  cfb.rs
│     │                  ifd.rs (TIFF directories and EXIF)  xmp.rs  icc.rs
│     ├─ image/          png.rs  jpeg.rs  gif.rs  bmp.rs  webp.rs  heif.rs (AVIF and HEIC)
│     │                  svg.rs  ico.rs  tiff.rs
│     ├─ application/    pdf.rs  office.rs  rtf.rs    (pdf.rs and office.rs call the parsers when their feature is on)
│     ├─ text.rs         encoding, counts, kind
│     ├─ audio.rs        names; audiodown's facts when its feature is on
│     └─ video.rs        names; videodown's facts when its feature is on
└─ mimedown-node/        new. @d0paminedriven/mimedown. Yours to configure
```

```toml
# crates/mimedown/Cargo.toml
[dependencies]
flate2.workspace    = true
ooxmldown.workspace = true
roxmltree.workspace = true
thiserror.workspace = true
audiodown           = { workspace = true, optional = true }
docxdown            = { workspace = true, optional = true }
pdfdown             = { workspace = true, optional = true }
pptxdown            = { workspace = true, optional = true }
videodown           = { workspace = true, optional = true }
xlsxdown            = { workspace = true, optional = true }

[features]
pdf    = ["dep:pdfdown"]
office = ["dep:docxdown", "dep:xlsxdown", "dep:pptxdown"]
audio  = ["dep:audiodown"]
video  = ["dep:videodown"]

# root Cargo.toml: members, and
mimedown      = { path = "crates/mimedown" }
mimedown-node = { path = "crates/mimedown-node" }
```

## 5. The model

What JavaScript is given. Rust holds the same thing as enums whose variants carry their own fields.

```ts
type Specs = ImageSpecs | DocumentSpecs | AudioSpecs | VideoSpecs | ArchiveSpecs | UnknownSpecs;

/** What every answer carries, whatever it is. */
interface Named<Type extends string, Format extends string, Mime extends string> {
  type: Type;
  /** For narrowing; equals `format`, as in audiodown and videodown. */
  kind: Format;
  format: Format;
  ext: string;
  mime: Mime;
  /** What decided it. */
  evidence: "signature" | "container" | "content" | "fileName";
  /** The workspace package that reads this type in full. */
  parser: "pdfdown" | "xlsxdown" | "docxdown" | "pptxdown" | "audiodown" | "videodown" | null;
  /** A range that would complete the answer, when the bytes given stop short of it. */
  needs: ByteRange | null;
  warnings: Array<string>;
}

interface ByteRange {
  offset: number;
  length: number;
  /** `offset` counts back from the end of the file: a zip's directory, a PDF's trailer. */
  fromEnd: boolean;
}

/** The columns of `ImageMetadata`, under the TypeScript package's names; `metadata` is the format's own. */
interface ImageFacts {
  width: number;
  height: number;
  frames: number;
  animated: boolean;
  hasAlpha: boolean | null;
  orientation: number | null;
  aspectRatio: number;
  colorModel: ColorModel;
  colorSpace: ColorSpace;
  iccProfile: string | null;
  exifDateTimeOriginal: string | null;
  cameraMake: string | null;
  cameraModel: string | null;
  lensModel: string | null;
  gpsLat: number | null;
  gpsLon: number | null;
}

type ImageSpecs =
  | (Named<"IMAGE", "png" | "apng", "image/png" | "image/apng"> & ImageFacts & { metadata: PngMetadata })
  | (Named<"IMAGE", "jpeg", "image/jpeg"> & ImageFacts & { metadata: JpegMetadata })
  | (Named<"IMAGE", "tiff", "image/tiff"> & ImageFacts & { metadata: TiffMetadata });
// and so on, one member for each format in section 8

/** The columns of `DocumentMetadata` that every kind of document can have. Null when not read. */
interface DocumentFacts {
  pageCount: number | null;
  wordCount: number | null;
  language: string | null;
  title: string | null;
  author: string | null;
  subject: string | null;
  keywords: Array<string>;
  textPreview: string | null;
  pageWidth: number | null;
  pageHeight: number | null;
  paperSize: PaperSize;
  pageSizeUniform: boolean | null;
  imageCount: number;
  annotCount: number;
}

type DocumentSpecs =
  | (Named<"DOCUMENT", "pdf", "application/pdf"> &
      DocumentFacts & { pdfVersion: string; isLinearized: boolean | null; isEncrypted: boolean | null; isSearchable: boolean | null })
  | (Named<"DOCUMENT", "docx" | "xlsx" | "pptx", OfficeMime> & DocumentFacts & { package: PackageKind | null })
  | (Named<"DOCUMENT", "txt" | "md" | "csv" | "json" | "html", TextMime> &
      DocumentFacts & { encoding: string; lineCount: number });
// and RTF, the OpenDocument types, EPUB, and the three legacy Office files

/** What audiodown and videodown already return, when their feature is on and the type is one they read. */
type AudioSpecs = AudiodownSpecs | Named<"AUDIO", AudioFormat, AudioMime>;
type VideoSpecs = VideodownSpecs | Named<"VIDEO", VideoFormat, VideoMime>;
```

- **`metadata` is no longer a generic.** `PngMetadata` keeps its shape. `xmp` is the fields or null, not sometimes the raw string.
- **`needs` is null when the answer is complete.** A caller with the whole file never sees anything else.
- **`mimeType` and `contentType`.** `audiodown` and `videodown` carry both as copies of `mime`, for the columns they feed. Say if you want the copies here too.
- **`byteSize` is not here.** From part of a file `mimedown` cannot know the size of the whole; `Extract` has it from the response and adds it, as it does now.
- **PDF-only and text-only columns sit on their own members.** A docx has no `pdfVersion` to be null.
- **A fact not read is null, and `needs` says why.** A PDF's first kilobyte gives `pdfVersion` and nothing else; `needs` then names the whole file.
- **Text is a document here,** because `DocumentMetadata` is where its row goes. Section 3's last row is how it is recognized, not a table.
- **Archives** are `Named` and nothing more.

## 6. Verbs

| Verb | Returns | What it reads |
| --- | --- | --- |
| `sniff(bytes, hint?)` | The type, the evidence, and `needs` | As little as settles it: a signature, or a container's first record |
| `specs(bytes, hint?)` | `Specs` | The facts of whatever `sniff` found: the header's, and the parser's when its feature is on and the whole file is given |
| `specsFromRanges(ranges, hint?)` | `Specs` | The same, from pieces of a file: `{ offset, bytes }` each, and the file's size when it is known |
| `mediaTypesOf(ext)`, `extensionsOf(mime)`, `mediaTypeOfPath(path)` | Strings | The tables. They stand in for `getMimes`, `mimeToExt` and `getMimeTypeForPath` |

`hint` is `{ fileName?: string }` and nothing else. It is consulted for text only (decision 11).

## 7. How the hard parts work

### 7.1 Ranges, and asking for more

Every reader goes through `source.rs`: a list of ranges, each knowing where in the file it sits. A read at an offset returns the bytes or says they are not there. When a reader needs bytes that are not there, the answer carries what was found so far and one `needs`.

The exchange with a fetcher is then two steps at most for every format here: the head, and one more range. `Extract` would fetch a head, call `specsFromRanges`, and when `needs` is set, fetch that range and call again with both.

### 7.2 ISO base media

One box walk. The brands in `ftyp` say which family: `avif` and `avis`; `heic`, `heix`, `heim`, `heis`, `hevc`, `hevx`; `mif1` and `msf1` alone are HEIF of an unnamed codec; `qt` is a MOV; `M4A` is audio; the rest of the MP4 brands are video.

For an image, the size is the primary item's: `pitm` names the item, `ipma` lists its properties, and its `ispe` is the one that counts. A picture stored as a grid of tiles has an `ispe` for the tiles as well, and whichever comes first in the file is not a rule. `irot` and `imir` become `orientation`. `colr` gives the color: its coded values (`nclx`) or a profile.

**`heic-rs` 0.1.4** (MIT or Apache-2.0, no `unsafe`, no dependency once its `parallel` feature is off, first published 2026-09-12) is a HEIC decoder in pure Rust whose `probe()` reads exactly these facts without decoding: the primary item, grids, `clap`, `irot`, `imir`, the alpha item, and whether EXIF, XMP and a profile are present. Its README says macOS and phones tile anything over 512 pixels a side, so an ordinary 4032 × 3024 photograph is a grid of 8 by 6. That is the case the TypeScript reader's first-`ispe` rule gets wrong.

Why it is still not a dependency here:

- It refuses AVIF when it opens the file, along with image sequences and any HEIF that is not HEVC. The walk has to exist in `mimedown` for those, and it is the same walk.
- It takes one slice. Section 7.1's ranges and `needs` would have to be worked around it.
- A probe would bring its HEVC decoder into the build, about 8,000 lines that `mimedown` never calls.
- It is two months old, with five releases and one maintainer.

What it is good for: reading beside the specification, checking our HEIC answers in tests, and decoding (see Parked).

### 7.3 EXIF, XMP and color profiles

Read once in `container/`, and used by every format that carries them:

| | Carried by |
| --- | --- |
| EXIF (a TIFF directory) | JPEG `APP1`, PNG `eXIf`, WebP `EXIF`, HEIF items, and a TIFF itself |
| XMP | JPEG `APP1`, PNG `iTXt`, WebP `XMP `, HEIF items, TIFF tag 700 |
| ICC profile | JPEG `APP2` (in pieces), PNG `iCCP` (compressed), WebP `ICCP`, HEIF `colr`, TIFF tag 34675 |

- **EXIF.** The first directory for orientation, `Make` and `Model`; the pointer to the Exif sub-directory for `DateTimeOriginal` and `LensModel`; the pointer to the GPS directory for latitude and longitude, as signed degrees. The date is returned as written, with its offset when the file records one and none when it does not: the column is a `DateTime`, and which zone a bare date is in is for the pipeline to say.
- **XMP.** A packet is whole XML, so it is parsed with `roxmltree`, not matched by pattern. The fields are those of `PngXmpFields`.
- **ICC.** The 128-byte header and the description tag. `iccProfile` becomes the profile's own name where it can be read and stays `"embedded"` where it cannot, and `colorSpace` is filled from the profile, where today a profile's presence sets it to `"unknown"`.

### 7.4 PNG

Chunks are walked by their lengths, so walking past `IDAT` costs nothing when the bytes are there; when they are not, `needs` is left null, since what follows `IDAT` is optional. A compressed chunk (`iCCP`, `zTXt`, a compressed `iTXt`) is inflated up to a cap.

### 7.5 JPEG

Segments up to the start of scan: the frame header for size and components, `APP1` for EXIF and XMP, `APP2` for the profile, `APP14` (Adobe) to tell CMYK from YCCK, `APP0` for density. A profile may be split across several `APP2` segments and is put back together.

### 7.6 TIFF

The directory chain, with every value read through its type, zero included. BigTIFF differs in the width of its offsets and counts and is read by the same code. Pages are counted to a cap. The first directory is wherever the header says: when it is outside the bytes given, `needs` names it.

### 7.7 SVG

A small scanner reads up to the root element's start tag, past a byte-order mark, the XML declaration, comments and a doctype. `roxmltree` cannot be used here: a head of a file is not whole XML. Only the root's own attributes count. `width` and `height` keep their units; the pixel size is filled for absolute units and from `viewBox` otherwise. A root with neither is an SVG without a size, not an error.

### 7.8 Zip and Office

- **From the head:** the first entry's name. An OpenDocument file and an EPUB put an uncompressed `mimetype` entry first, whose content is the type itself.
- **From the directory at the end:** every name. `word/document.xml`, `xl/workbook.xml` and `ppt/presentation.xml` name the three Office types.
- **With the whole file:** `ooxmldown` opens the package for its exact kind, within the limits it already enforces, and with the `office` feature the parser for that kind fills the row (section 13).
- **A compound file** is recognized by its first eight bytes. Its directory names the stream that tells a doc from an xls from a ppt; an encrypted Office file is also a compound file and is said to be one.

### 7.9 Text

1. A byte-order mark, or else whether the sample is valid UTF-8. A NUL byte in something that is not UTF-16 means it is not text.
2. How it begins: `{` or `[` for JSON, `<` for markup (then section 7.7's scanner for the root), `#!` for a script, `%!PS`, `{\rtf`.
3. The file name, through the tables, for what bytes cannot settle: Markdown, CSV, source code.
4. Otherwise `text/plain`.

The answer carries `encoding`, `lineCount`, `wordCount` and `textPreview`, counted over the bytes given. `language` is left null: the column is documented as an ISO 639-1 code, and the TypeScript package puts the programming language there (`typescript`, from the extension). Say if that use is the one you want kept.

### 7.10 PDF

- **From part of a file:** the header line gives the version, and a linearized file states its page count (`/N`) and its length (`/L`) in its first object, within the first kilobyte. Everything else is null and `needs` names the whole file.
- **From the whole file, with the `pdf` feature:** `pdfdown` opens it once and section 14 lists which call fills which column.
- **Without the feature:** as from part of a file, and `parser` says `pdfdown`.

## 8. Images: what is kept and what changes

The fields of `ImageFacts` are kept for every format. `colorSpace` is always a member of its union.

| Format | The TypeScript package reads | In `mimedown` |
| --- | --- | --- |
| PNG, APNG | Size, bit depth, color type, frames (`acTL`), `iCCP` name and header, `sRGB`, `cHRM`, `pHYs`, the three text chunks, XMP fields, `tIME`, `eXIf` | The same, and the same `metadata`. Chunks after `IDAT` are read when given. `xmpToolkit` is filled |
| JPEG | Size and components, EXIF orientation, whether a profile is present | The same. Adds the date from the Exif sub-directory, XMP, the profile's name, density, and CMYK told from YCCK |
| GIF | Size, frame count | The same. `hasAlpha` from the transparency flag, where today it is null. Adds the loop count |
| BMP | Size; indexed or RGB by bit depth | The same. `orientation` is null: the order of the rows is not a rotation, and it goes in `metadata`. Alpha from the masks of a 32-bit file |
| WebP | Size, alpha, animation, frame count, whether a profile is present | The same, with the size of an extended file read from all its bytes. Adds EXIF orientation, the date and XMP |
| AVIF | Size, color from `colr`, alpha from `auxC`, a date from XMP, frames of a sequence | The same, with the primary item's size, `irot` and `imir` as orientation, and a real `colorSpace` |
| HEIC, HEIF | Written as AVIF's twin; not reached (section 2) | Reached, by the same reader as AVIF |
| SVG | `width` and `height`, or `viewBox`; a rough test for animation | From the root element only, units kept. No size is an answer, not an error |
| ICO | The largest entry, the entry count, entries that are PNGs | The same. A `.cur` is named as a cursor |
| TIFF | Size, samples, photometric, extra samples, orientation, whether a profile is present, pages, the EXIF date | The same, with zero-valued tags read. BigTIFF is read. Adds XMP |

### 8.1 The rest of `ImageFormat`

The schema's enum has members the TypeScript package never returns. All are recognized; the size is read where the header states it plainly.

| Members | Recognized by | Size |
| --- | --- | --- |
| `jxl` | The codestream's `FF 0A`, or the container's signature box | From the codestream header |
| `jp2`, `jpx` | The JPEG 2000 signature box and the brand after it | From the image header box |
| `jxr` | `II` followed by `BC 01` | From its directory, read as a TIFF's is |
| `jls` | A JPEG whose frame marker is `F7` | From the frame header, as any JPEG |
| `dng`, `cr2`, `nef`, `arw`, `raw` | A TIFF: `CR` at offset 8 for `cr2`, the DNG version tag for `dng`, the maker for `nef` and `arw`. `raw` for the other camera formats that have a signature of their own | The full picture's directory, not the preview's |
| `hdr`, `rgbe`, `xyze`, `pic` | Radiance's `#?` first line; the `FORMAT` line says RGBE or XYZE | From the resolution line |

`hdr`, `rgbe`, `xyze` and `pic` are one file format under four names, and `pic` differs from `hdr` only in its extension. Which member you want for which case is a question for you.

## 9. Errors and limits

```rust
pub enum MimeError {
  /// Ranges that overlap and disagree, or lie past the size given for the file.
  InvalidRanges(String),
  LimitExceeded(String),
}
```

- **Not recognized** is `UNKNOWN`, with `application/octet-stream`. It is not an error.
- **Recognized and damaged** returns what was read, with `warnings`. A PNG whose header is cut short is still a PNG.
- **Limits.** Caps on boxes and their nesting, on TIFF directories and their entries, on PNG chunks, on what a compressed chunk may inflate to, on zip entries listed, and on the length of a text preview. `ooxmldown` keeps its own.
- **Nothing is written to the console.**

## 10. Order of work

Each unit ends with its own tests passing.

| # | Unit | Verified by |
| --- | --- | --- |
| 1 | `source`, `media_type`, `tables`, the fixed signatures; `sniff()` | A header built in the test for every signature in section 3 |
| 2 | Containers: ISO base media, RIFF, EBML, zip names, compound file | The same, including an MP4 and a HEIC that differ only in brand |
| 3 | PNG, GIF, BMP, ICO; `specs()` | Headers built in the test |
| 4 | EXIF, XMP, ICC; JPEG and TIFF | The same, with a TIFF whose directory is at the end |
| 5 | WebP; AVIF and HEIC | The three forms of WebP; a grid with two `ispe` |
| 6 | SVG and text | Roots with and without a size; each encoding |
| 7 | The rest of `ImageFormat` (section 8.1) | Headers built in the test |
| 8 | Office by entry names, RTF, a PDF's header facts | The public docx, xlsx, pptx and PDF fixtures already in the workspace |
| 9 | Ranges and `needs` end to end; `specsFromRanges()` | A file cut at several points, read in two steps |
| 10 | In `pdfdown`: what section 14 says it lacks. Its own release. (`xlsxdown`'s `metadata()` is built) | Its tests |
| 11 | The `pdf` feature, then `office`, `audio`, `video` | Section 13's tables, column by column, on the public fixtures |
| 12 | `mimedown-node`: binding, tests on your `WorkupService`, wasm | Native and wasm suites |
| 13 | `ci/` folder and workflow; release `mimedown@0.1.0` | The first run |
| 14 | In `@d0paminedriven/metadata`: `Extract` and `ExtractClient` call `mimedown` | That package's own suite |

## 11. Fixtures and tests

- **No fixture is added unless you add it.** These formats suit that: a PNG header is 33 bytes and a BMP's is 54, so the library's tests build what they read.
- **Already in the workspace:** the public docx, xlsx, pptx and PDF fixtures of the other packages, and their audio and video files for the naming.
- **A parity run, outside the repository.** A scratch script reads the assets the TypeScript package's own suite downloads (its README says 629) with both and compares field by field, with every difference checked against section 8. Those assets are almost all PNG and JPEG, with a few of each other format and no HEIC, so they say little about the formats that changed most.
- **Files that would cover the rest**, if you have them: a HEIC from a phone; an animated AVIF and an animated WebP; an APNG; a CMYK JPEG; a JPEG with a large profile; a BigTIFF and a TIFF of several pages; an ICO with PNG entries; an SVG with only a `viewBox`.

## 12. What `@d0paminedriven/metadata` keeps

- **Keeps:** `Extract`'s fetching: the probe, range requests, deadlines, the idle guard, quarantine, the origin fallback and the CloudFront hook.
- **Hands over:** the four sniffers of section 2, the image classes, and the documents side.
- **The call:** fetch a head; `specsFromRanges`; when `needs` is set, fetch that range and call once more. The head-and-tail join goes away, and so does the choice between the server's header and the bytes: the bytes decide, and the header can be reported beside them.
- **A PDF, an Office file, an audio or a video file is fetched whole** to fill its row. For those, `needs` names the whole file after the first call. An image still costs a head.
- **`ExtractClient`** becomes the WebAssembly build.

Unit 14 is a change in another repository and is yours to schedule.

## 13. The four tables

Every column of `attachmentmeta.prisma`, and where its value comes from. `attachmentId`, `createdAt` and `updatedAt` are the application's.

### `ImageMetadata`

| Column | From | Note |
| --- | --- | --- |
| `format` | The sniff | Sections 8 and 8.1 |
| `width`, `height`, `aspectRatio` | The header | |
| `frames`, `animated` | The header | A GIF's frame count takes the whole file |
| `hasAlpha` | The header | |
| `orientation` | EXIF; `irot` and `imir` in a HEIC or AVIF | |
| `colorSpace`, `colorModel` | The header and the color profile | Spelled as the schema spells them (decision 21) |
| `iccProfile` | The profile's own description | |
| `exifDateTimeOriginal` | EXIF | A string as written; section 7.3 |
| `cameraMake`, `cameraModel`, `lensModel` | EXIF | New: not returned today |
| `gpsLat`, `gpsLon` | EXIF's GPS directory | New: not returned today |
| `dominantColorHex` | The pixels | Not a header fact (decision 23) |

### `DocumentMetadata`

| Column | PDF | docx | pptx | xlsx | Text |
| --- | --- | --- | --- | --- | --- |
| `format` | `pdf` | `docx` | `pptx` | `xlsx` | `txt`, `md`, `csv`, `json`, `html` |
| `pageCount` | `metadata()` | `metadata()` | Slides | Sheets | null |
| `wordCount` | The words of `layout()` | `metadata()` | `metadata()` | `metadata()` | Counted |
| `language` | **Not read** | The default language | The language most runs state | The properties | null (section 7.9) |
| `title`, `author`, `subject`, `keywords` | **Not read** | The properties | The properties | The properties | null |
| `pdfVersion`, `isLinearized` | `metadata()` | | | | |
| `isEncrypted` | **Not read** | false: an encrypted file is refused | The same | The same | false |
| `isSearchable` | Whether any page has words of its own | true | true | true | true |
| `encoding`, `lineCount` | | | | | Read and counted |
| `textPreview` | The start of `structuredText()` | The start of `text()` | The start of `text()` | The first text cells | The start of the text |
| `pageWidth`, `pageHeight`, `paperSize` | The first page box | The first section | The slide | null | null |
| `pageSizeUniform` | Whether there is one page box | `metadata()` | true | null | null |
| `imageCount` | **Costly today** | `metadata()` | `metadata()` | `metadata()` | 0 |
| `annotCount` | `annotations()` | `metadata()` | `metadata()` | `metadata()` | 0 |

- **Bold cells are what `pdfdown` lacks** (section 14).
- **xlsx** has `metadata()` since 2026-11-17 (section 14). It has no `text()`, so a preview is put together from its text cells.
- **A workbook's own numbers have no column.** Sheets, rows, columns, cells, formulas and merged ranges ride on the xlsx member's `metadata`, as a PNG's chunks ride on its member. Whether they get columns is for your migration; the TypeScript package folds the sheet names and words such as `formulas` and `charts` into `keywords` for want of one.
- The docx and pptx columns are the tables already in section 8 of their two plans.

### `VideoMetadata`

| Column | From `videodown` |
| --- | --- |
| `format`, `width`, `height`, `aspectRatio` | The same names |
| `duration` | `durationMs` |
| `frameRate`, `hasAudio`, `resolution`, `orientation`, `rotation` | The same names |
| `bitrate` | `bitrateKbps` |
| `codec` | `videoCodec` |
| `keyframeTimes` | Its keyframe index |
| `waveformPeaks` | `audioPeaks()`: decodes the audio track, so a verb of its own, not part of `specs` |
| `thumbnailCount` | The application's: it is how many posters were made |

`videodown` reads MP4, MOV, WebM and Matroska. An AVI or an MPEG is named, and its row has the name only.

### `AudioMetadata`

| Column | From `audiodown` |
| --- | --- |
| `format`, `sampleRate`, `channels`, `codec` | The same names |
| `duration` | `durationMs` |
| `bitrate` | `bitrateKbps` |
| `title`, `artist`, `album`, `year`, `genre` | The same names |
| `waveformPeaks` | `waveformPeaks()`: decodes the stream, so a verb of its own, not part of `specs` |

`audiodown` reads MP3 and WAV. FLAC, Ogg, AAC and M4A are named, and their rows have the name only.

## 14. The parsers as dependencies

`mimedown` is the top of the graph: it depends on the parsers, each behind a feature, and `specs` hands the whole file to the one that reads it.

| Feature | Brings | Crates in its tree |
| --- | --- | --- |
| `pdf` | `pdfdown` | 100 |
| `office` | `docxdown`, `xlsxdown`, `pptxdown` | About 30 each, mostly shared with one another |
| `audio` | `audiodown` | 38 |
| `video` | `videodown` | 73 |

What this costs, so that it is chosen knowingly:

- **The whole file.** None of these parsers reads part of a file. Section 12 says what that means for `Extract`.
- **Time.** A PDF's row means opening it with lopdf and reading every page's text for `wordCount`; that is the price of the column, and it is what `pdfdown` charges today.
- **Weight.** The npm package's addon holds all six parsers. The release build strips what is never called, such as the image and video decoders, so it is smaller than the sum; I have not measured it.
- **One direction.** Once `mimedown` depends on a parser, that parser cannot depend on `mimedown`: Cargo refuses the cycle. So the parsers keep their own small sniffers, and `mimedown` calls them for the types they own (`videodown::sniff`, `audiodown`'s `sniff`, `ooxmldown`'s `PackageKind`) so that no type is recognized by two different pieces of code.
- **Releases.** A fix in a parser reaches `mimedown`'s users when `mimedown` is released again.

What `pdfdown` lacks for its columns. These are changes to `pdfdown` and need your go, as its own unit:

- **`title`, `author`, `subject`, `keywords` and `isEncrypted`.** `PdfMeta` reads only the creator, the producer and the two dates from the Info dictionary. Section 5.5 of the port plan proposed the five fields; they were not added.
- **A count of images that does not decode them.** `images()` decodes every image to a PNG to return it, which is far too much work for a number.
- **`language`.** The catalog's `/Lang`, where there is one. The port plan left it out as often absent or wrong.
- **A way to stop early.** `textPreview` needs the first 500 characters and `isSearchable` needs one page with words; both are cheap if the reading can stop there. `wordCount` cannot stop.

**`xlsxdown` has `metadata()`: built 2026-11-17, not released.** `Workbook::metadata()` in the library, `metadata()` and `metadataAsync()` on `XlsxDown`.

| Field | From |
| --- | --- |
| The properties: `title`, `author`, `subject`, `keywords`, `created`, `modified`, `application` and the rest | `ooxmldown`'s `DocProps`, read when the workbook is opened and kept as `Workbook::props` |
| `sheetCount`, `hiddenSheetCount`, and for each sheet its name, its position and whether it is hidden | The model |
| For each sheet: the `dimension` the file declares, the `usedRange` its cells span, `rowCount` and `columnCount` | The model. The used range is from the first to the last cell that holds something |
| For each sheet and in total: `cellCount`, `formulaCount`, `mergedRangeCount` | The model. A cell that follows a shared formula is a formula cell |
| `wordCount`, `characterCount` | The cells that hold text |
| `linkCount`, `tableCount` | The sheet part, counted as it is parsed |
| `commentCount`, `imageCount`, `chartCount`, `pivotTableCount` | The parts a sheet's relationships name: its comments, its drawing, its pivot tables. A part that cannot be read is left out with a warning on the sheet |
| `definedNameCount` | The workbook part, the built-in names left out |
| `annotationCount` | Links and comments, as the other two packages count it |
| `date1904` | The model |

- **How it differs from the proposal.** `drawingCount` was dropped for the pictures and charts a drawing holds. `dimension` was added beside `usedRange`, the declared beside the counted, as the other packages set a recorded count beside their own. Rows and columns are given for each sheet and not summed over the workbook.
- **Still refused.** A macro-enabled workbook and a template are turned away when opened, so there is no `hasMacros` or `isTemplate`. Reading them, as `docxdown` and `pptxdown` read theirs, is a decision of its own.
- **Tests.** The parts of a workbook are now reached through a small seam (`source.rs`), so the tests build one from XML held in memory: the counts that no public fixture exercises (comments, pictures, charts, pivot tables, links, tables, names) are tested that way. The public fixtures cover sheets, cells, formulas, merged ranges, words and the used range, on the native addon and the WebAssembly build.

### Why not copy the reading into `mimedown`

The alternative is for `mimedown` to depend on the parser underneath (lopdf for a PDF) and carry its own copy of the logic. Measured on this workspace:

- **It saves little.** lopdf alone is 74 of the 100 crates in `pdfdown`'s tree. The other 26 are the image decoders, and the release build already leaves those out of an addon that never calls them.
- **The columns need the part that is hard to copy.** `wordCount`, `isSearchable` and `textPreview` come from reading page content: the interpreter, the fonts and the Unicode maps, about 1,500 lines. The structural facts (`meta.rs`, 271 lines) are the small part.
- **Two readers drift.** A fix made in one is missing from the other. The TypeScript package is in that state now, with two PDF readers side by side (section 2).

The same holds for a workbook: its numbers come from the model `xlsxdown` builds, so the verb belongs there and `mimedown` asks for it.

If the weight of `pdfdown` ever does matter, the answer is to move code, not copy it: the structural facts go into `pdfdown-meta`, which is empty and reserved for PDF metadata and would need lopdf alone. `pdfdown` and `mimedown` would both call it, and only the three text columns would need `pdfdown` itself.

The other route to the same rows is to leave `mimedown` without these dependencies and have the TypeScript side call each package after the sniff; `@d0paminedriven/metadata` already depends on `@d0paminedriven/pdfdown`. It keeps `mimedown` small and puts the mapping to columns in TypeScript, once for each table. I recommend the features: one call, one place that knows the columns, and the same answer in Rust and in JavaScript.

## Parked

- Fetching in Rust (`reqwest`, `tokio`). Dropped by decision 3.
- PDF facts from part of a file, beyond section 7.10. The page model's old home, `pdfdown-meta`, is empty and reserved for PDF metadata; a metadata-only load of a whole PDF belongs there.
- `dominantColorHex` (decision 23).
- Facts for the audio and video types the two parsers do not read.
- Telling CSV from TSV from plain text by content; naming the language of a text.
- More image formats: JPEG XL, JPEG 2000, PSD, camera raw, KTX2 (commented out in the TypeScript package).
- Decoding a HEIC to pixels with `heic-rs`. Nothing in the workspace can today, and a phone's photograph of an invoice is a HEIC: it would be the way into OCR for such a picture, and into returning a HEIC embedded in a docx or a pptx as a PNG. Not sniffing, so not this crate's.
- `ooxmldown`'s `image_info`, which names an embedded picture from its first bytes, sharing code with `image/`. `mimedown` depends on `ooxmldown`, so the shared code would have to live in `ooxmldown` or below it.

## Not verified

- Nothing here is compiled.
- The items under "Read in the source, not run" in section 2. Only the WebP, HEIC and BMP results were run.
- That a phone's HEIC lists a tile's `ispe` before the picture's. The format allows it and `heic-rs`'s README describes such files as the common case; I have not read one here.
- `heic-rs` itself: I read its source and README. I did not build it or run it.
- That every Office writer puts the names section 7.8 relies on where it expects. `ooxmldown` settles it when the whole file is given.
- How many real files answer in one step and how many need the second range.
- The earlier finding that lopdf's metadata-only load is far faster than a full one was measured for `pdfdown`, not for this plan.
- Section 13 against the real tables: I read the schema file, not the code that writes the rows, so a column filled today by something other than the TypeScript package may be missing from my account.
- `xlsxdown`'s chart, pivot table and link counts on a real file: no public fixture holds any, so they are tested on parts built in memory.
- The size of an addon holding all six parsers, and the time `specs` takes on a large PDF.
- That each parser's metadata is cheap enough to call for a row. `docxdown` and `pptxdown` build their whole model to answer.
