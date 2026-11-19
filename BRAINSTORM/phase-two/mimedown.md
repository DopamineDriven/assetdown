# mimedown: plan

Status: **units 1 to 4 of section 10 are built** (2026-11-18): the byte source, the types, the extension tables, the sniff by signature and by container, `specs()`, and the facts of PNG, GIF, BMP, ICO, JPEG, TIFF, WebP, AVIF and HEIC with their EXIF, XMP and color profiles; SVG and text with the file-name hint; Office; all four features, `pdf`, `audio`, `office` and `video`; `retryOnUnknown`; the exchange of ranges tested end to end; the rest of the schema's `ImageFormat` (JPEG XL, JPEG 2000, JPEG XR, JPEG-LS, camera raw, Radiance); RTF; the seven PDF fields `pdfdown` now gives; the waveform, poster and keyframe verbs; and the exports of `mimedown-node`. The "as built" notes under section 10 say what exists and where it departs from this plan. Everything after unit 4 is still a proposal, and no snippet for it has been compiled.

`mimedown` takes over the part of `@d0paminedriven/metadata` that looks at bytes and says what they are: the sniffing and the facts. The part that fetches stays in that package.

What it has to return is fixed by where the answer goes: the four metadata tables in `~/cloneathon/t3-chat-clone/packages/db/prisma/schema/attachmentmeta.prisma` (`ImageMetadata`, `VideoMetadata`, `AudioMetadata`, `DocumentMetadata`). You are folding each table and its `Original…` twin into one; the columns stay largely as they are. Section 13 goes through them column by column.

Research: I read `@d0paminedriven/metadata` at version 6.0.2 (`~/personal/d0paminedriven/packages/metadata`, about 10,900 lines of source outside its tests) and ran its built image reader on three headers made by hand. Paths below that start with `src/` are in that package. It is a read-only reference here.

## 1. Decisions

Every row is answered (2026-11-17). Where your answer changes a row, the block under the table says how, and the block wins over the row.

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

### Answers (2026-11-17)

Rows 7, 10, 11, 14 to 20 and 24: agreed as written. The rest:

- **8, the shape of the answer: settled last, and it overrides the row.** `mimedown` sticks to pure MIME types. Every answer carries `mime` and `topLevel`, the word in front of the slash exactly as the registry has it: `application`, `audio`, `font`, `haptics`, `image`, `model`, `text`, `video`. It carries no `IMAGE` / `DOCUMENT` / `ARCHIVE` judgment at all. Which of your tables a type belongs to is your `AssetType`, which you are widening in your migration, and you decide it in a TypeScript helper of your own over `mime` ("I'd be happy making a helper in typescript if we just stick to pure mime types here … that's not a big deal"). Your rule of thumb for it: "TEXT = always document, APPLICATION = often document". The union `specs` returns is keyed on `topLevel` and then on `format`. "What `mimedown` does not say" under section 5 lists the `application` types by what they are, for that helper.
- **9, unknown bytes.** `UNKNOWN` stays an answer, not an error. Added: `{ retryOnUnknown: true }`. When the first pass cannot name the type, it tries again with more to go on, in steps: a wider window for a signature that sits behind leading bytes, then the end of the file (a zip's directory, a PDF's trailer), then a larger sample for text. From ranges, the retry is the `needs` it returns. The answer says which step named it. As built (2026-11-18) the first and third steps are narrower than this: see "`retryOnUnknown` and the exchange, as built" under section 10.
- **12, audio and video.** Through `audiodown` and `videodown`, and their waveform, poster and keyframe verbs are exposed through `mimedown` as well, not left to the two packages alone.
- **13, PDF.** The part of `pdfdown` that derives metadata moves into `pdfdown-meta`. `pdfdown` and `mimedown` both consume it. Users consume `pdfdown`; `pdfdown-meta` is not a package of its own. **Settled later the same day:** `mimedown` also takes `pdfdown`, behind the `pdf` feature, for the three columns that need page content (`wordCount`, `textPreview`, `isSearchable`), so that one call fills the row.
- **21, spellings.** The schema's (`grayscale_alpha`).
- **22, camera and place.** Wanted: `cameraMake`, `cameraModel`, `lensModel`, `gpsLat`, `gpsLon` from EXIF.
- **23, `dominantColorHex`.** Dropped.
- **25.** No `mime_guess`, no `infer`, and no field that guesses. The file name stays a hint for text only (row 11).

- **8, how it got there.** I first proposed kinds of my own (`IMAGE`, `DOCUMENT`, `ARCHIVE`, `OTHER`, …). You asked for fonts, models and haptics as top-level entries of their own, pointed out that WebAssembly is `application/wasm`, and asked whether a PDF should then be `APPLICATION` and Markdown `TEXT`. Those are all the registry's top-levels, and one field had been carrying both the registry's word and a judgment about the file. The judgment is the part that left. Fonts: `font/woff2`, `font/woff`, `font/ttf`, `font/otf`, `font/collection`; the embedded OpenType font has no string under `font` and is `application`. Models: the ones you named (`model/vnd.usdz+zip`, `model/gltf-binary`, `model/gltf+json`, `model/vnd.pixar.usd`, `model/iges`, `model/mesh`, `model/obj`) and `model/vnd.usda`, `model/stl`, `model/3mf`, `model/mtl`, `model/step`, `model/vrml`, `model/x3d+xml` and `model/vnd.collada+xml`. `model/vnd.pixar.usd` is in use and not registered; the registry has `model/vnd.usda` for the text form, which is what a file that begins `#usda` is called.
- **Added after the answers.** A Wallet pass (`application/vnd.apple.pkpass`, section 7.8) and a vCard (`text/vcard`, a file that begins `BEGIN:VCARD`; section 7.9's second step) are both recognized. A vCard is text and goes with the documents.

The binding crate is yours and comes first; section 15's item 1 is done.

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
| Zip: entry names | docx, xlsx, pptx, OpenDocument, EPUB, pkpass, JAR, a plain zip | The first name from the head; all of them from the directory at the end |
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
// Keyed on `topLevel`, then on `format`. `ApplicationSpecs` and `TextSpecs` are where the
// documents are; a type with no facts of its own is `Named` and nothing more.
type Specs =
  | ImageSpecs | AudioSpecs | VideoSpecs | ApplicationSpecs | TextSpecs
  | FontSpecs | ModelSpecs | HapticsSpecs | UnknownSpecs;

/** What every answer carries, whatever it is. */
interface Named<TopLevel extends string, Format extends string, Mime extends string> {
  /** The word in front of the slash in `mime`, as the registry has it. Never a judgment. */
  topLevel: TopLevel;
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
  | (Named<"image", "png" | "apng", "image/png" | "image/apng"> & ImageFacts & { metadata: PngMetadata })
  | (Named<"image", "jpeg", "image/jpeg"> & ImageFacts & { metadata: JpegMetadata })
  | (Named<"image", "tiff", "image/tiff"> & ImageFacts & { metadata: TiffMetadata });
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
  /** As the file records them. In `DocSpecs` today; the schema has no column for either. */
  createdDate: string | null;
  modifiedDate: string | null;
}

type DocumentSpecs =
  | (Named<"application", "pdf", "application/pdf"> &
      DocumentFacts & { pdfVersion: string; isLinearized: boolean | null; isEncrypted: boolean | null; isSearchable: boolean | null })
  | (Named<"application", "docx" | "xlsx" | "pptx", OfficeMime> & DocumentFacts & { package: PackageKind | null })
  | (Named<"text" | "application", "txt" | "md" | "csv" | "json" | "html", TextMime> &
      DocumentFacts & { encoding: string; lineCount: number });
// and RTF, the OpenDocument types, EPUB, and the three legacy Office files

/** What audiodown and videodown already return, when their feature is on and the type is one they read. */
type AudioSpecs = AudiodownSpecs | Named<"audio", AudioFormat, AudioMime>;
type VideoSpecs = VideodownSpecs | Named<"video", VideoFormat, VideoMime>;
```

- **`metadata` is no longer a generic.** `PngMetadata` keeps its shape. `xmp` is the fields or null, not sometimes the raw string.
- **`needs` is null when the answer is complete.** A caller with the whole file never sees anything else.
- **One pair of shapes** (your word, 2026-11-17): `ImageSpecs` and `DocumentSpecs`, covering what `ImageSpecs` and `DocSpecs` return today and the schema's columns, field for field where that makes sense and not otherwise. What the `Expanded…` pair adds (`source`, `fetchedBytes`, `byteSize`, `contentType`) is known only to whoever fetched the file, so `Extract` goes on adding it.
- **`mimeType` and `contentType`.** `audiodown` and `videodown` carry both as copies of `mime`, for the columns they feed. Say if you want the copies here too.
- **`byteSize` is not here.** From part of a file `mimedown` cannot know the size of the whole; `Extract` has it from the response and adds it, as it does now.
- **PDF-only and text-only columns sit on their own members.** A docx has no `pdfVersion` to be null.
- **A fact not read is null, and `needs` says why.** A PDF's first kilobyte gives `pdfVersion` and nothing else; `needs` then names the whole file.
- **Text carries the document facts,** because `DocumentMetadata` is where its row goes: `encoding`, `lineCount`, `wordCount`, `textPreview`. Its `topLevel` is `text`, or `application` for what the registry files there (JSON, YAML, SQL, a shell script). Section 3's last row is how it is recognized, not a table.
- **Archives, fonts, models, haptics and the rest of `application`** are `Named` and nothing more: the type, and no facts.

### What `mimedown` does not say

Whether a file is a document, an archive or neither is not in the answer. For every top-level but one it follows from the word: `image`, `audio`, `video`, `font`, `model` and `haptics` are what they say, and `text` is always a document. `application` is the one that has to be looked at, type by type. These are the `application` types `mimedown` can name today, sorted the way the kinds I removed had them, for your helper:

| What they are | `application/…` |
| --- | --- |
| Documents with pages, sheets or slides | `pdf`, the three `vnd.openxmlformats-officedocument…` types, `msword`, `vnd.ms-excel`, `vnd.ms-powerpoint`, `vnd.visio`, the three `vnd.oasis.opendocument…` types, `epub+zip`, `vnd.amazon.ebook`, `x-abiword`, `rtf` |
| Text, filed here by the registry | `json`, `jsonc`, `json5`, `jsonl`, `x-ndjson`, `ld+json`, `manifest+json`, `yaml`, `toml`, `xml`, `xhtml+xml`, `sql`, `x-sh`, `x-csh`, `node`, `x-mdx`, `vnd.apple.mpegurl`, `dash+xml`, `vnd.mozilla.xul+xml` |
| Files that hold other files | `zip`, `gzip`, `x-bzip`, `x-bzip2`, `x-tar`, `x-7z-compressed`, `vnd.rar`, `java-archive`, `vnd.apple.pkpass`, `x-freearc`, `x-cfb` |
| A font | `vnd.ms-fontobject` |
| The rest | `wasm`, `x-msi`, `vnd.apple.installer+xml`, `x-python-code`, `x-cdf`, `ogg` (an Ogg of no known codec) |
| Bytes that name no type | `octet-stream` |

When the binding exists, its generated typings give the strings as a union, so a table of yours written `as const satisfies Record<…>` over the `application` members would have the compiler flag any type `mimedown` adds later.

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
- **A Wallet pass (`.pkpass`)** is a zip that always holds `pass.json` and `manifest.json`; those two names give `application/vnd.apple.pkpass`. It is named, with no row. Added at your request (2026-11-17).
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
| 1 | `source`, `media_type`, `tables`, the fixed signatures; `sniff()`. **Built 2026-11-17** | A header built in the test for every signature in section 3 |
| 2 | Containers: ISO base media, RIFF, EBML, zip names, compound file. **Built 2026-11-17** | The same, including an MP4 and a HEIC that differ only in brand |
| 3 | PNG, GIF, BMP, ICO; `specs()`. **Built 2026-11-18** | Headers built in the test |
| 4 | EXIF, XMP, ICC; JPEG and TIFF. **Built 2026-11-18** | The same, with a TIFF whose directory is at the end |
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

### Unit 1, as built (2026-11-17)

In `crates/mimedown/src`: `error.rs`, `source.rs`, `media_type.rs`, `tables.rs`, `sniff.rs`, each with its tests beside it (`<name>/tests.rs`, 48 in all). Nothing was added to a manifest.

| What | Is |
| --- | --- |
| `sniff(bytes) -> Sniffed` | The whole file. `needs` is always `None` |
| `sniff_source(&Source) -> Sniffed` | All or part of a file |
| `Source::whole(bytes)`, `Source::from_pieces(&[Piece { offset, bytes }], size)` | Section 7.1. Pieces in any order; those that touch or overlap are read as one; an overlap that disagrees, or a piece past the size given, is `MimeError::InvalidRanges` |
| `Sniffed::Named { media_type, evidence, needs }`, `Sniffed::Unknown { needs }` | Two variants, so that evidence exists exactly when a type was named. `media_type()` and `needs()` read either |
| `MediaType` | 133 types (after unit 2), one row each in one table: its string, the other strings in use for it, its extensions, its `TopLevel`, its `Parser` |
| `TopLevel` | `application`, `audio`, `font`, `haptics`, `image`, `model`, `text`, `video`: read off the string, never a judgment |
| `media_types_of(ext)`, `media_type_of_path(path)`, `extensions_of(mime)`, `MediaType::from_mime(text)` | Section 6's tables |

**Named today**, all by `Evidence::Signature`: PNG, JPEG, GIF, BMP, ICO and CUR, TIFF and BigTIFF, Radiance HDR, KTX, KTX2, DPX, EMF; PDF, RTF; gzip, bzip2, tar, 7z, RAR; FLAC, MP3 (and the other MPEG audio layers), AAC, MIDI, Ogg as audio, video or neither; MPEG program and video streams; WebAssembly; the fonts WOFF, WOFF2, OTF, TTF, TTC and EOT; the models binary glTF, USD in its binary and its text form, IGES, STEP, VRML and binary STL.

**Not named yet, so `Unknown` for now:** everything in a container (zip and the Office files, RIFF with WebP, WAV and AVI, ISO base media with AVIF, HEIC, MP4 and MOV, EBML, compound files), which is unit 2, and text of every kind, SVG included, which is unit 6. For the models that means: USDZ and 3MF are zips (unit 2); OBJ, MTL, glTF as JSON, COLLADA, X3D, a mesh and an STL written as text are text (unit 6).

Where it departs from the plan, each for you to overrule:

- **No kinds.** Row 8 had a `type` of `IMAGE`, `DOCUMENT` and so on, and I added more to it as we went. It is gone: an answer has `mime` and `topLevel`, and the judgment is yours to make in TypeScript (the Answers block, and "What `mimedown` does not say" in section 5).
- **A binary STL is known by its length alone.** It has no signature: 80 bytes of anything, a count of triangles, 50 bytes for each. So it is named only when the file's size is known and fits the count exactly.
- **`sniff(bytes)` takes its bytes to be the whole file.** Only then can "a caller with the whole file never sees `needs`" hold. The head of a file goes through `Source::from_pieces`, and then an answer can ask for more: `Unknown` asks for the head up to 4,096 bytes, or as far as the test that ran short had reached.
- **`mimedown` names every type with its own tests, in every build.** Section 14 had it call the parsers' sniffers for the types they own. Those are optional dependencies, so the answer would change with the features, and `audiodown`'s own test takes the byte-order mark of a UTF-16 text for an MP3 frame. The parsers' sniffers are to be checked against `mimedown`'s in unit 11 instead.
- **A short signature is not believed alone.** `BM`, `true`, `OTTO`, `ID3`, `wOFF`, `fLaC`, `MThd` and an icon's four bytes can each begin something else, so the header behind them has to hold together. MP3 and AAC without a tag have no signature at all: two frames of one stream are required, the second where the first says it ends.
- **More than row 1 of section 3 lists:** tar, bzip2, AAC, MIDI, MPEG streams, Radiance HDR (asked for by you), and the types of the TypeScript tables whose signature is a few exact bytes (KTX, KTX2, DPX, EMF, binary glTF, TTC, EOT), and the models with a signature.
- **The tables.** Every one of the 133 extensions names the type the TypeScript package gives first, except three: `ico` is `image/x-icon` (row 15), `ttf` is `font/ttf` with `application/font-sfnt` accepted, and `mid` and `midi` are one type, `audio/midi`. The reverse table is derived from the same rows and no longer kept by hand, so its oddities are gone: `text/event-stream` lists `sse` alone, `video/webm` lists `webm`, `text/javascript` puts `js` first, and `multipart/voice-message` is not a type here. Added: `mov`, `mkv`, `flac`, `heif`, `vcf`, `ttc`, `mpg`, `qt`, `hdr`, the extensions of the models, and the strings fonts went by before `font/` was registered (`application/font-woff`, `application/x-font-ttf`, `application/vnd.ms-opentype` and the like), which are accepted and never returned.
- **Left for the unit that needs them:** the `hint` (unit 6), `retryOnUnknown` (after unit 6, when all three of its steps exist), APNG told from PNG (unit 3, with the chunk walk), each type's `format` name (unit 3), `MimeError::LimitExceeded` (with the first limit), and the types of section 8.1 other than HDR (unit 7).

Known gaps: a PDF with bytes in front of `%PDF-` and an MP3 known only by a tag at its end are for `retryOnUnknown`. A tar from before POSIX has no signature and is not named. An MPEG transport stream is not named. Free-format MPEG audio is not named.

Checked beyond the tests, with a throwaway program that is not in the repository: the workspace's public PDF, MP3 and JPEG fixtures are all named; of 399 source and text files in the repository none is named; and across about 60,000 files under `/usr`, every answer agrees with the file's extension. Every font and every binary model found on the machine is named for what it is. A `.obj` is as often a compiler's object file as a model, which is why the name is never taken for the type.

### Unit 2, as built (2026-11-17)

In `crates/mimedown/src`: `container.rs` and `container/{isobmff,riff,ebml,zip,cfb}.rs`, each with its tests beside it; `sniff/container_tests.rs`; and `testing.rs`, which builds boxes, a zip and a compound file in memory for the tests. 101 tests in all. Nothing was added to a manifest and no fixture was added.

Each reader says what a container holds and nothing about what that makes the file; `sniff` decides that, and the answer's evidence is `container`.

| Container | Read | Named |
| --- | --- | --- |
| ISO base media | The first box. An `ftyp` gives its brands. Without one, the first box has to be a type a file opens with and has to end where another box begins, or where the file does | AVIF, HEIC, HEIF, AVCI by a picture's brand; MOV (`qt  `), M4A, 3GP, 3G2 by the major brand; any other brand is MP4. No `ftyp`: a piece of a fragmented file is MP4, anything else is a QuickTime movie from before there were brands |
| RIFF (`RIFF`, `RF64`, `RIFX`) | The form at offset 8 | WebP, WAV, AVI, CD audio. A form with no type here is `UNKNOWN` |
| EBML | The header's document type | WebM; anything else, or a header that does not say, is Matroska |
| Zip | The names in the directory at the end. Without it (a head, or a damaged file), each entry's own header leads to the next for as far as the bytes go. A first entry called `mimetype`, stored as it is, declares the type | docx, xlsx, pptx by their main part; EPUB and the three OpenDocument types by `mimetype`; Wallet pass (`pass.json` and `manifest.json`); USDZ (a scene first); 3MF; JAR. Anything else is a zip |
| Compound file | The directory, followed through the allocation table, and the names directly under the root | doc, xls, ppt, vsd by their stream; MSI by the root's class. Anything else is `application/x-cfb` |

Where it departs from the plan, each for you to overrule:

- **Boxes are tested before the fixed signatures.** A box opens with its own length, and a length can read as anything: a first box of 442 bytes opens with the four bytes an MPEG program stream does, and one of 256 with an icon's. The box test asks for a known type and a second box behind the first, which four bytes cannot fake. The plan's order was signatures, then containers.
- **A picture's brands count only when the major brand is a picture's.** `mif1` with `heic` among its compatible brands is a HEIC; `mp42` that lists `heic` as something it is compatible with is still a film.
- **A film without `ftyp` is `video/quicktime`,** where `videodown`'s own sniff calls every such file MP4. The parser is the same either way.
- **Office files are named by the canonical main part only** (`word/document.xml`, `xl/workbook.xml`, `ppt/presentation.xml`). A macro-enabled file or a template has the same part, so it comes back as the plain type until unit 8, where `ooxmldown` reads the content types of a whole file. A binary workbook (`.xlsb`) is a zip.
- **`needs` can take more than two steps.** A zip whose names a head does not settle asks for its last 65,557 bytes, which is as far back as the record that closes the directory can start; if the directory begins before that, it then asks for the directory by its exact range. A compound file asks for one piece of its directory at a time. The plan said two steps at most for every format.
- **What `UNKNOWN` asks for.** When the read that fell short lies in the first 4,096 bytes, the head is asked for up to there. When it lies far into the file, as the box after a film's media does, that range alone is asked for.
- **Two types were added** for what a compound file can be: `application/x-cfb` and `application/x-msi`. Neither string is registered; both are the ones in common use.
- **A document saved with a password** is a compound file around what cannot be read, so it is `application/x-cfb`. Which kind of document it was is not knowable from outside. `specs` is where a warning will say so.

Known gaps: a zip with bytes in front of it (a self-extracting archive) is for `retryOnUnknown`. A zip too large for the ordinary directory record (zip64) is named from its first entries only. An APK, an IPA and the like are zips. An audio-only Matroska or WebM is named as the video type: the tracks are deep in the file, and telling them is `videodown`'s.

Checked beyond the tests, with a throwaway program that is not in the repository:

- Every public docx, xlsx, pptx, MP4, MOV, Matroska, WebM and WAV fixture in the workspace is named for what it is. The two public macro-enabled workbooks come back as xlsx, as said above.
- About 30,000 files on this machine across sixteen kinds of container (WebP, AVIF, MP4, MOV, Matroska, WebM, AVI, 3GP, WAV, docx, xlsx, pptx, OpenDocument, JAR, Wallet passes, USDZ): every answer is right. The ones that came back otherwise are not what their names say: a PNG called `.webp`, a Makefile called `.doc`, zip fixtures with bytes in front.
- The same sweep as before for wrong answers, over about 60,000 system files and the repository's own text: none.
- **HEIC, checked afterward.** `heif-enc` (libheif) and ImageMagick are installed here, so real files were made in a scratch folder from a generated picture: 8-bit and 10-bit, whole and cut into a grid of tiles, and the same as AVIF. Two HEIC files already on the machine were sniffed as well. All eight are named for what they are, from their first 32 bytes. One of the two found files has the general brand `mif1` as its major brand and `heic` only among those it is compatible with, which is the case the compatible brands are read for.
- **Not checked on a real file:** `.doc`, `.xls`, `.ppt`, `.msi`, M4A and EPUB. There is none on this machine, so those are tested on files built in memory only. The compound-file reader did run on real compound files of other kinds without misnaming one.

### Units 3 and 4, as built (2026-11-18)

In `crates/mimedown/src`: `specs.rs`; `image.rs` and `image/{png,gif,bmp,ico,jpeg,tiff}.rs`; `container/{ifd,exif,icc,xmp}.rs`; each with its tests beside it. 166 tests in all. Nothing was added to a manifest (`flate2` and `roxmltree` were already there) and no fixture was added: every file the tests read is built in `testing.rs`.

| What | Is |
| --- | --- |
| `specs(bytes)`, `specs_source(&Source)` | `Specs { media_type, evidence, needs, warnings, facts }`. Never an error |
| `Facts` | `Image(ImageFacts)`, or `None` for a type that is named and has no reader here yet |
| `ImageFacts` | The image columns under their own names: `width`, `height`, `frames`, `animated`, `has_alpha`, `orientation`, `color_model`, `color_space`, `icc_profile`, `exif_date_time_original`, `camera_make`, `camera_model`, `lens_model`, `gps_lat`, `gps_lon`; `aspect_ratio()`; and `icc`, `xmp`, `detail` |
| `ImageDetail` | What only one format says: `Png`, `Gif`, `Bmp`, `Ico`, `Jpeg`, `Tiff`, each a struct of its own |
| `IccProfile` | The profile's own name, version, class, kind of color, connection space, maker, date |
| `Xmp` | Eighteen fields of a packet: the tool, the dates, the ids, the Dublin Core fields, Photoshop's color mode and profile name, the packet's own orientation |

What each format gives beyond the shared columns:

- **PNG:** bit depth, color type, interlacing, density or pixel shape, gamma, an animation's loop count, every text chunk, the time of the last change. An animated PNG is named `image/apng` by `sniff`, from the chunk that stands in front of the picture.
- **GIF:** the version, the loop count, how long one pass through the frames lasts, the size of the shared palette.
- **BMP:** the header's version, bits to the pixel, the compression, whether the rows run from the top down, the density.
- **ICO and CUR:** which of the two it is, and each picture's size, depth, whether it is a PNG, and a cursor's hotspot.
- **JPEG:** the sample depth, the channels, whether it is progressive, JFIF's density, Adobe's transform.
- **TIFF:** BigTIFF or not, the byte order, the pages, the compression, the bits and samples, what the samples mean, the density, the software, the date.

Where it differs from the TypeScript package, on purpose:

- **JPEG's date is found.** It is in the directory the first one points at, which that package does not follow. Camera make and model, the lens, and the position are new.
- **A TIFF tag whose value is zero is read,** so a white-is-zero picture is grayscale and not unknown. BigTIFF is read. Nothing is written to the console.
- **A bitmap has no orientation.** That package reports a quarter turn for the ordinary bottom-up kind. Whether the rows run from the top is in the detail.
- **Alpha is answered.** A GIF says whether a frame names a see-through color and a bitmap says whether a mask marks alpha bits, where that package returns null for both. Every icon is see-through, since each picture carries a mask.
- **A color profile is read, not only noticed.** `icc_profile` is the profile's own name, then the name the file gives it, then `embedded`; and `color_space` comes from the profile, where that package leaves it `unknown` once a profile is present.
- **A gray PNG is `gray`** whatever curve its shades follow; that package calls it `srgb` when the sRGB chunk is there.
- **An icon's color space is `unknown`,** where that package assumes sRGB.
- **XMP is parsed as XML.** `xmpToolkit` is filled. The packet's own orientation is kept in `xmp` and is not taken for EXIF's.
- **Chunks after a PNG's picture are read** when they are there.

Where it departs from this plan:

- **One range serves the name and the facts.** When `sniff` and a reader each lack something, `needs` is the span that covers both, so one fetch settles both.
- **A PNG's signature alone asks for more.** Eight bytes name a PNG and cannot say whether it is animated, so the answer is `image/png` with a `needs`.
- **A GIF's frames take the whole file,** as the plan says. From part of one, `needs` is the rest of the file when its size is known and the next megabyte when it is not.
- **Limits are warnings.** A chunk too long, a compressed chunk that inflates past four megabytes, more text than is kept: each is left out and said so. `MimeError::LimitExceeded` still has nothing to report and is not added.
- **Left out:** a PNG's chromaticities as a way to guess its space; the AI fields of `PngXmpFields` (`aiPrompt` and the rest); XMP in a GIF; a bitmap's embedded profile; a TIFF's sub-directories; the thumbnail in a JPEG's EXIF.
- **Added from real files:** a camera's notes kept in a PNG as hexadecimal text under `Raw profile type exif`, which is how ImageMagick wrote them before the format had a chunk for it.
- **Not test-first throughout.** The JPEG and TIFF tests, and all of unit 3's, failed before their readers were wired in. The four shared readers and the PNG's use of them were written before their tests.

Checked beyond the tests, with throwaway programs that are not in the repository:

- **Against `exiftool`, on 5,096 files on this machine** (2,015 JPEG, 79 TIFF, 3,002 PNG): the width and height agree on every one. EXIF's orientation agrees on every JPEG and TIFF that has one. 27 PNGs differed: 25 because `exiftool` reports the XMP packet's orientation, which is kept apart here, and 2 because of the hexadecimal text chunk, which is read since. I did not run the comparison again after that last change.
- **Thousands more PNG, GIF, BMP, ICO and CUR files:** no size of zero or beyond belief, and one warning, on a PNG that really is cut short.
- **Seen on real files:** animated PNGs, Display P3 and Adobe RGB profiles, YCCK JPEGs, camera makes, dates and positions.
- **Not seen on a real file:** BigTIFF, a TIFF of several pages, a PNG with coded color values.

### WebP, as built (2026-11-18)

Taken out of unit 5 and built ahead of AVIF and HEIC, because production holds WebP and holds neither of those. `image/webp.rs` and its tests; 172 tests in all.

- **The three forms.** The two simple ones keep their size in the picture's own header. The extended one opens with a chunk of flags and the canvas, three bytes for each side, and every byte of them is read: the TypeScript package skips a side with a zero byte in it and reports 1 by 1.
- **The flags are read at their own bits.** Alpha is 0x10 and animation is 0x02. The TypeScript package tests 0x02 for alpha and 0x01 for animation.
- **Beyond the shared columns:** whether the file is extended, whether the picture is kept without loss, an animation's loop count and how long one pass lasts. The color profile, EXIF and XMP are read from their chunks by the readers of unit 4.
- **From part of a file,** the rest is asked for only when the flags promise something further on: EXIF, XMP, an animation, or a profile not yet seen.
- **Checked against `exiftool`** on 5,636 WebP files on this machine: every width and height agrees, and no file drew a warning. None of them is animated, so the frame count is tested on files built in memory only.

### The `pdf` and `audio` features, as built (2026-11-18)

Part of unit 11, built ahead of units 5 to 9 because of what production holds. `document.rs`, `document/pdf.rs` and `audio.rs`, with their tests. 175 tests with no feature, 183 with both. No manifest was touched: the features and their optional dependencies were already declared.

| What | Is |
| --- | --- |
| `Facts::Document(DocumentFacts)` | The document columns: `page_count`, `word_count`, `language`, `title`, `author`, `subject`, `keywords`, `text_preview`, `page_width`, `page_height`, `paper_size`, `page_size_uniform`, `image_count`, `annot_count`, `created_date`, `modified_date`; and `detail` |
| `DocumentDetail::Pdf(PdfDetail)` | `version`, `is_linearized`, `is_encrypted`, `is_searchable`, `creator`, `producer`, `has_form` |
| `Facts::Audio(AudioFacts)` | The audio columns: `duration_ms`, `bitrate_kbps`, `sample_rate`, `channels`, `codec`, `title`, `artist`, `album`, `year`, `genre` |

**PDF.**

- **Without the feature, or from part of a file:** the version from the first line, and the page count of a file laid out for the network, from its first object. Without the feature nothing more is asked for; with it, the rest of the file is.
- **With the feature and the whole file,** `pdfdown` opens it once: pages, the two dates, the size of the page most pages share and the paper it is nearest to, whether every page is that size, annotations, the words of every page, the first 500 characters, whether any page has text of its own, the creator and producer, whether there is a form.
- **Still unsaid, because `pdfdown` does not give them yet (filled since: see "The `pdfdown` pass, as built"):** `title`, `author`, `subject`, `keywords`, `language`, `is_encrypted`, and `image_count`. They are `None`, not guessed. That is the `pdfdown` unit of section 15, item 3, which is yours to give a go on.
- **A file `pdfdown` cannot open** is still a PDF with what its first line says, and a warning.

**Audio.**

- **With the feature,** `audiodown` reads an MP3 or a WAVE file: how long it plays, the rate, the channels, the codec, the tags.
- **The bitrate is the average over the file** when the walk of its frames gives one, where `audiodown`'s own `bitrateKbps` is the first frame's. The two are the same for a file whose rate does not vary.
- **From part of a file,** the length is given only when the file states it up front (a Xing header, a `fact` chunk); otherwise it is left out and the rest is asked for.
- **Without the feature** an audio file is named and nothing more. A type `audiodown` does not read (FLAC, Ogg, AAC, M4A) is named either way.

Checked beyond the tests, with throwaway programs that are not in the repository:

- **430 real PDFs, against `exiftool`:** the page count agrees on every one, and none drew a warning. In a debug build the median file took 27 ms, nine in ten under 170 ms, and the slowest about three seconds; the words of every page are what that buys.
- **298 real MP3 and WAVE files, against `ffprobe`:** on each of the 287 that `ffprobe` reads, the length agrees to within one percent or 60 ms, and the sample rate and channels agree. One WAVE file with its lengths written the other way round is named by `sniff` and not read by `audiodown`; it comes back with a warning.

### AVIF and HEIC, as built (2026-11-18)

The rest of unit 5. `image/heif.rs` and its tests, with a builder in `testing.rs` that writes a `meta` box of items, properties and references. 185 tests with no feature, 193 with `pdf` and `audio`.

- **The size is the primary item's.** `pitm` names the item, `ipma` lists its properties, and its `ispe` is the one that counts. A grid's tiles have a size too, listed first, and it is passed over.
- **What a grid does not say, its tiles say for it.** A phone writes the color profile and the depth on the tiles and gives the grid a size and nothing else. This came from a real file and was not in the plan.
- **The turn is the picture's own.** `irot` and `imir` are applied in the order the item lists them and given as EXIF numbers them, 1 to 8. With neither, `orientation` is `None` whatever the camera's notes say, since a reader of this format does not turn the picture by them.
- **Alpha** is an item that points at the picture and says in a property of its own that it is its alpha.
- **Color:** a profile when there is one, coded values otherwise (`nclx`), and sRGB when the file says nothing. One channel is gray; a matrix of zero is `rgb`; otherwise `ycbcr`, or `rgba` with alpha.
- **A camera's notes and an XMP packet** are items of their own, read from wherever the list of items says they are.
- **A sequence** counts the samples of its first track of pictures.
- **Detail:** the codec as the item's type (`av01`, `hvc1`), the depth of a sample, whether the picture is tiled.
- **From part of a file,** a still picture is answered once its `meta` box is read, which is nearly always in the first kilobytes.

Checked beyond the tests, with throwaway programs that are not in the repository:

- **156 real files against `exiftool`:** 148 AVIF and HEIC files already on this machine, and 8 made here with `heif-enc` (8 and 10 bits, whole and tiled, with and without alpha, and one from a JPEG turned by its EXIF). Every width and height agrees.
- **The turned one** began as a JPEG with orientation 6 and comes back as 6.
- **A phone's photograph,** stored as a grid of 48 tiles: the size is the grid's, 3,992 by 2,992, and the profile is the one `exiftool` names. It lists a tile's size before the grid's, which settles what "Not verified" below had left open.
- **Not seen on a real file:** a mirrored picture, an image sequence, and coded values other than the usual ones. The first two are tested on files built in memory. Which way round the two mirror axes are is taken from how `libavif` documents them; I have not checked it against a file.

### Unit 6, SVG and text, as built (2026-11-18)

`text.rs`, `text/json.rs`, `text/markup.rs`, `image/svg.rs`, `document/text.rs`, with their tests. 206 tests with no feature, 214 with `pdf` and `audio`.

| What | Is |
| --- | --- |
| `Hint { file_name }` | What a caller knows beside the bytes. Never the type |
| `sniff_with(&Source, Hint)`, `specs_with(&Source, Hint)` | The verbs with a hint. `sniff`, `sniff_source`, `specs` and `specs_source` are these with none |
| `Evidence::Content`, `Evidence::FileName` | What named a text: what it says of itself, or its name |
| `DocumentDetail::Text(TextDetail)` | `encoding` and `line_count`; `word_count` and `text_preview` are among the shared fields |
| `ImageDetail::Svg(SvgDetail)` | The root's `width` and `height` as written, and its `viewBox` |

- **Text is the last step,** tried when nothing else claimed the file: a byte-order mark, or UTF-8, or one byte to a character when nearly all of the bytes are plain ASCII. A zero or a control character that text does not hold means it is not text.
- **What the text says of itself comes first:** how it begins (a vCard, a calendar, a playlist, subtitles, PostScript), the interpreter a script's first line asks for, the root element of markup, and JSON.
- **JSON is what reads through as JSON.** `[section]` and `[1]: a footnote` begin like it and are not. A value on every line is JSON Lines.
- **Markup is named for its root,** found by a scanner that stops at the first start tag: `html` (XHTML when the text says it is XML), `svg`, `COLLADA`, `X3D`, `MPD`. Any other root is XML when the text declares itself XML or declares a namespace, and plain text when it does neither.
- **The name is a hint for text only,** and only for a type text can be: `photo.png` on a file of words is still a file of words. It settles Markdown, CSV, source code, and which kind of JSON a JSON file is.
- **An SVG's size is its root's.** Absolute units are turned into pixels; a share of something else falls back to the `viewBox`; a root with neither is an SVG of no stated size, and not an error. A `width` further down the file is someone else's.
- **A text's facts:** its encoding, its lines, its words, and its first 500 characters, counted over the bytes given. From part of a file the rest is asked for.
- **`application/postscript` was added** as a type, for a text that begins `%!PS`.

Where it departs from the plan:

- **Not test-first.** The code was written before its tests in this unit.
- **`retryOnUnknown` is not built.** The hint is. (Built since: see "`retryOnUnknown` and the exchange, as built".)
- **A head shorter than the furthest signature (265 bytes) is not offered to the text step:** it asks for the head first, since a tar or another type could still claim it.
- **A head that stops inside markup's first tag** is words for now, with a `needs` for the rest.
- **An SVG is not said to be animated.** The TypeScript package's test for it was a rough search of the first kilobyte.
- **Legacy encodings are not named.** A text that is not UTF-8 is `unknown-8bit`; `encoding_rs` is in the workspace and not among this crate's dependencies.

Checked beyond the tests, with a throwaway program that is not in the repository, on about 12,000 files: the repository's own sources and text, a sample of everything under `/usr`, and the SVG files on this machine.

- **Binaries are not taken for text.** Of 8,161 files named as text, the `file` command calls 15 binary; each is a `.txt` or `.md` file.
- **Text is not missed.** Of 1,504 files left unknown, `file` calls 7 text.
- **SVG sizes agree with `exiftool`** on every one of the 2,768 files for which it gives a plain size in pixels.

### Office, as built (2026-11-18)

Unit 8's Office part and the `office` feature of unit 11. `document/office.rs` and its tests. 211 tests with no feature, 222 with `pdf`, `audio` and `office`.

- **The exact kind is named by `sniff`** when the whole file is there: `ooxmldown` opens the package and the content type of its main part says whether it is a document, a macro-enabled one, a template or a slide show. Seven types were added for those: `docm`, `dotx`, `xlsm`, `xltx`, `pptm`, `potx`, `ppsx`. From part of a file the names still say Word, Excel or PowerPoint and no more.
- **What the file says of itself needs no feature,** because `ooxmldown` is always a dependency: `title`, `author`, `subject`, `keywords`, `language`, the two dates, and the application that wrote it. `DocumentDetail::Office` adds whether the file is macro-enabled and whether it is a template.
- **Without the `office` feature** the page and word counts are the ones the application last wrote into the file, which can be out of date.
- **With it,** the parser for the type counts: pages (a deck's slides, a workbook's sheets), words, pictures, annotations, the page's size and paper, whether every page is that size, the default language, and the first 500 characters of text. A workbook's preview is its first cells that hold text.
- **A file a parser turns away** keeps what its properties say and carries a warning. Today that is every macro-enabled workbook and every Strict one (section 15, item 2).
- **From part of a file** nothing inside the package can be read, so it is named and the rest is asked for.

The feature's tests read fixtures the other crates of the workspace already track, and compare each count with what the parser itself returns. No fixture was added.

Checked beyond the tests, with a throwaway program that is not in the repository, on 173 files: the workspace's tracked Office fixtures and the Office files on this machine. Every one is named for its exact kind. The title agrees with `exiftool`'s on 170 and the author on 169; where they differ, `exiftool` gives nothing and a value was read here. The only warnings are the six workbooks `xlsxdown` turns away.

### The `video` feature, as built (2026-11-18)

The last of unit 11's four. `video.rs` and its tests. 211 tests with no feature, 224 with all four.

- **`Facts::Video(VideoFacts)`:** `width` and `height` as the picture is shown, `duration_ms`, `frame_rate`, `bitrate_kbps`, `codec`, `has_audio`, `resolution`, `orientation` (`landscape`, `portrait` or `square`, as `videodown` says it), `rotation`, `keyframe_times_ms`; and `aspect_ratio()`.
- **With the feature,** `videodown` reads MP4, MOV, WebM and Matroska. Without it, and for any other video type, the file is named and nothing more.
- **From part of a file,** what the head holds is given, and the rest is asked for when the head does not hold the file's whole description.
- **Not through `mimedown` (built since: see "The verbs, as built"):** the waveform, the poster and the frames at keyframes. Row 12's answer has them exposed here too; they decode, so each is a verb of its own, and none is built.

The tests read video fixtures `videodown-node` already tracks and compare each fact with what `videodown` itself returns.

Checked beyond the tests, with a throwaway program that is not in the repository, on 322 files: the tracked fixtures and a third of the video files on this machine. On every one `ffprobe` can read, the size agrees (249) and the length agrees to within one percent or 100 ms (248). Fourteen files named as QuickTime movies are ones `videodown` cannot read; each comes back named, with a warning.

### `retryOnUnknown` and the exchange, as built (2026-11-18)

`sniff/retry_tests.rs` and `specs/exchange_tests.rs`. 231 tests with no feature, 244 with all four.

- **`Options { file_name, retry_on_unknown }` replaces `Hint`** as what `sniff_with` and `specs_with` take. `Sniffed::Named` and `Specs` gain `retry: Option<Retry>`, and `Retry` is `LeadingBytes { skipped }`, `EndOfFile` or `LenientText` (`as_str()`: `leadingBytes`, `endOfFile`, `lenientText`).
- **A second look is taken only when it is asked for and the first pass names nothing.** What the first pass names is the same with or without the option.
- **Behind leading bytes, up to 1,024 of them, a signature is not enough.** This departs from the Answers' "a wider window for a signature". A type is named there only by what a constant cannot be: a PNG whose header chunk's checksum holds, EBML whose header names its document type, a compound file whose directory parses, and a PDF whose header (`%PDF-` and a version) stands in the first 1,024 bytes and whose end is a PDF's. Real files are why: three or four bytes turn up by chance (three files were named MPEG video by the four bytes a sequence header opens with), and every program that reads a format holds its signature as a constant (two compiled Python modules, the ones that read PNG and GIF, were named PNG and GIF).
- **At the end of the file:** a zip by its directory, when the directory lists at least one entry and its records are where it says; a PDF by `startxref`, a number and `%%EOF`, in that order, in the last 1,024 bytes; an MP3 by `TAG` 128 bytes from the end. A zip's directory counts either from the zip's own first byte (a self-extracting archive) or from the file's (an archive written after a launcher's script); both are read, and the answer says how many bytes stand in front.
- **Text is read with more allowance, not from a larger sample.** The first pass already samples 64 KB; what it refuses is UTF-16 with no byte-order mark and text with a control character in it. The second look takes UTF-16 when nine in ten of its pairs have their zero on the same side, and takes text with stray control characters when they are no more than one byte in a thousand, leaving them out.
- **Facts are read from where the file starts.** `Source::shifted` gives the file with its leading bytes left off, `specs` reads through it, and any range a reader then asks for is given by its place in the bytes as they were handed over.
- **From part of a file,** the second look asks for the end: the last 65,557 bytes, which is as far back as a zip's closing record can stand and which holds a PDF's trailer and the tag as well. With no size known, the range is counted from the end.
- **The exchange is tested end to end.** `specs/exchange_tests.rs` plays a fetcher: the head, then each `needs` in turn, until an answer asks for nothing. It runs over PNG, APNG, GIF, JPEG, a TIFF whose directories are 50,000 bytes in, a WebP with EXIF after its picture, AVIF, SVG, text, JSON, PDF, a docx, a zip with its sizes after the data, a compound file, a tar, Matroska, and a movie whose media comes first; from heads of 16, 300 and 4,096 bytes; with the size known and unknown. Every one settles within eight fetches on the type, facts and warnings the whole file gives. No code changed to make them pass. One difference is recorded as a test: text chunks after a PNG's picture are read when they are there and are never asked for.
- **A read of no bytes lacks nothing.** An empty file used to ask for the range of zero bytes at its start; it asks for nothing.

The tests for the second look failed before its code was written, and again before each tightening. The option, the `retry` field and `Source::shifted` were put in place first with no behavior, so that those tests could compile.

Checked beyond the tests, with a throwaway program that is not in the repository, on 29,011 files on this machine: a quarter of `/usr/share`, `/usr/lib` and `/usr/bin`, 4,243 compiled Python modules, and what is named `.zip`, `.pdf`, `.exe`, `.mp3` or `.pyz`. The first pass names 20,640. A second look names 15 more: four zips at the end of Windows libraries, and eleven texts with a stray control character in them, most of which `file` calls data. No compiled Python module is named.

Closed by this: a PDF with bytes in front of `%PDF-`, an MP3 known only by the tag at its end, and a zip with bytes in front of it, which units 1 and 2 left for here.

Not found by a second look: a GIF, a JPEG, a RIFF file, sound, or an archive other than a zip behind leading bytes; an empty zip behind a program; a zip too large for the ordinary directory record.

### The rest of `ImageFormat`, as built (2026-11-18)

Section 8.1 and unit 7. `image/jxl.rs`, `image/jp2.rs`, `image/jxr.rs`, `image/hdr.rs` and their tests; `sniff/rare_tests.rs`; changes to `image/jpeg.rs` and `image/tiff.rs`. 311 tests with no feature, 324 with all four.

**Seventeen types are added.** None of the camera strings is registered; they are the ones the desktop's shared database of types uses.

| Type | Named by | Facts |
| --- | --- | --- |
| `image/jxl` | The container's signature box, or the codestream's two bytes when what follows them is not text | Size, depth, alpha, how it is turned, animation and its loop count, the color space, whether it is in the format's own space; EXIF and XMP from the container |
| `image/jp2`, `image/jpx`, `image/jpm`, `image/jph` | The signature box, then the brand | Size, channels, depth, the color space by its number or its profile, alpha by the channel definitions, a palette, XMP |
| `image/j2c` | `FF 4F FF 51` | Size, channels and depth from the size marker |
| `image/jxr` | `II`, `BC`, and a version of 0 or 1 | Size, the pixel format (color, alpha, bits), a plane of alpha, how it is turned, profile, XMP, EXIF |
| `image/jls` | A JPEG whose frame header is `F7` | A JPEG's, and `lossless` from the scan header |
| `image/x-adobe-dng` | A TIFF with the DNG version tag | The size of the full picture's directory, EXIF from the first |
| `image/x-canon-cr2` | A TIFF with `CR` and 2 after its header | The same |
| `image/x-nikon-nef`, `image/x-sony-arw` | A TIFF in which some directory is a color filter mosaic, by its maker | The same |
| `image/x-dcraw` | A mosaic by any other maker | The same |
| `image/x-canon-cr3`, `image/x-fuji-raf`, `image/x-olympus-orf`, `image/x-panasonic-rw2` | A brand, or a signature of its own | None: named and no more |

- **`image/x-dcraw` is mine, not the plan's.** The plan kept "raw" for formats with a signature of their own. With pure MIME each of those has its own string, so the general one goes to a sensor's recording in a TIFF by a maker that is not told apart (Pentax, Samsung and the rest), which would otherwise come back as `image/tiff`.
- **`hdr`, `rgbe`, `xyze` and `pic` are one type,** `image/vnd.radiance`. Which of them a file is, is `HdrDetail.encoding`: `rgbe` or `xyze`. This answers the question section 8.1 left for you.
- **`ImageDetail` gains `Jxl`, `Jp2`, `Jxr` and `Hdr`. `JpegDetail` gains `lossless`:** true for a JPEG of the lossless kind and for a JPEG-LS whose scan header allows no difference.
- **A camera's file gives the size of what its sensor recorded.** Among the first directory and those it points at, the one marked as the full picture is taken, the largest when several are; who made it, when and how it is turned come from the first. Its other directories are copies, so `frames` is 1. A TIFF that keeps smaller copies of itself still gives its own size.
- **A TIFF from part of a file now asks for its directory before it is sure of its name,** since only the directory tells a camera's file from a TIFF. A JPEG never asks: JPEG-LS is told when the bytes given reach the frame header, which every writer of it puts at the start.
- **What is missing is asked for a block at a time** (4,096 bytes, `Source::block`) by the TIFF, JPEG XR, JPEG 2000 and JPEG XL readers. A directory is followed by its values and a box's header by what it holds; asking for each few bytes took more than eight fetches on a camera's file.
- **JPEG XL and JPEG 2000 ask for a camera's notes wherever they stand:** before the codestream, between its pieces or after it.

Not read, each with a warning where a file has it: the frames of an animated JPEG XL (`frames` is 1); EXIF and XMP that a JPEG XL keeps compressed with Brotli, which is how the reference encoder writes them unless told otherwise; a color profile inside a JPEG XL codestream (`icc_profile` is `embedded`). Not read and not warned of: a JPEG 2000's resolution and its EXIF box; Motion JPEG 2000 is not named.

The naming tests, and the tests for JPEG-LS, Radiance, JPEG XR, JPEG 2000 and JPEG XL, failed before their code was written. The change that picks a camera file's full picture went in with the naming code, so its own tests passed when they were written.

Checked beyond the tests, with throwaway programs that are not in the repository, on files made here by encoders that are not this code:

| Format | Files | Against | Agrees on |
| --- | --- | --- | --- |
| JPEG XL | 32, from `cjxl` and ImageMagick: ten sizes, gray, alpha, 16 bits, lossless, animations, from a JPEG, in boxes, Rec. 2020 and P3 | `jxlinfo` | Size, animation, alpha, depth, lossy or not, color layout, turn, color space: all 32 |
| JPEG 2000 | 8 files and 5 codestreams, from OpenJPEG and ImageMagick and two found on this machine | `exiftool` | Size on all; channels, depth and color number on the 8 files |
| JPEG XR | 6 pixel formats from `JxrEncApp`, and one file with each of the eight turns written into it | `exiftool` | Size, pixel format and the turn's number: all 14 |
| JPEG-LS | 4 from `ffmpeg` | `ffprobe` | Size and channels; the one written to allow a difference is not `lossless` |
| Radiance | 9 found on this machine, 2 from ImageMagick, and one with each of the eight turns written into it | `exiftool` | Size: all 19 |

`exiftool` names two of a Radiance picture's eight turns the other way round from here (`-X -Y` and `+X +Y`, the two quarter turns). It gives them the words the format's own description uses, which say what was done to the picture; EXIF's numbers say what a viewer has to do, and for a quarter turn those are opposite. The numbers here follow from which side of the picture the first line and the first pixel of a line are.

### RTF, as built (2026-11-18)

Unit 8's RTF. `document/rtf.rs` and its tests.

- **`DocumentDetail::Rtf(RtfDetail)`:** `code_page` and `generator`.
- **What the document says of itself** is read from its `info` group: `title`, `author`, `subject`, `keywords`, `created_date`, `modified_date`, and `page_count` as the writer last counted it. A fact stated twice is the first: LibreOffice names who made a document and who last saved it both as its author. A date of all zeros is none.
- **The text** is what is left once the font, color and style tables, pictures, headers, footers, footnotes, comments and field instructions are set aside, with a field's result and a text box's words kept. `word_count` and `text_preview` are counted from it. From part of a file they are over what was given, and the rest is asked for, as for text.
- **Characters** written by their number are read, with the stand-ins after them stepped over, and so are those beyond 16 bits written as two numbers. A byte above 127 is read in Windows code page 1252; in any other code page it is left out, with one warning.
- **`language`** is the one most of the text is said to be in, and the document's default when its text names none. Windows numbers languages; fifty of the numbers, for forty-nine languages, have tags here.
- **`page_width` and `page_height`** are the paper's, in points. **`paper_size` is not filled:** the table of paper sizes is in `contentdown`, which `mimedown` does not depend on. One line in your manifest (`contentdown.workspace = true`) and one here would name it as the PDF and Word readers do.
- **`image_count`** is the document's own pictures, a picture written a second time for older readers counted once, and a picture placed as a shape counted.

The tests failed before the code, with one exception: the handling of text written twice (`upr`) was written first and then shown to fail its test with the handling switched off.

Checked beyond the tests on 14 files: six written by LibreOffice from public Word fixtures and from text, one by pandoc, and seven found on this machine that TextEdit wrote. Author and both dates agree with `exiftool` on the four that state them. The word count is LibreOffice's own on 8; on the other 6 it differs by between 1 and 7 words, each accounted for by two things: LibreOffice counts the bullet in front of a list item as a word and this does not, and this counts the words of a text box and LibreOffice's plain text leaves them out.

### The `pdfdown` pass, as built (2026-11-18)

Row 3 of section 15, as far as the seven facts `mimedown` had to leave unsaid. In `crates/pdfdown/src`: `types.rs`, `meta.rs`, `images.rs`, `pdf.rs` and `meta/tests.rs`. 143 tests in `pdfdown`.

- **`PdfMeta` gains `title`, `author`, `subject`, `keywords`, `language` and `is_encrypted`.** The first four are the Info dictionary's, as the file writes them; `keywords` is the one string it is there, and `mimedown` splits it at commas and semicolons. `language` is the catalog's `/Lang`. `is_encrypted` is true for a file that was encrypted on disk, whether or not it opened without a password.
- **A text string is read in whichever way it is written:** sixteen bits to a character behind a mark, in either order of bytes; UTF-8 behind its mark; the format's own one-byte encoding; and UTF-8 with no mark, which the format does not allow and many writers produce. A string that holds nothing is none. The four fields `PdfMeta` already had (`creator`, `producer` and the two dates) are read as before, by the older decoder, which turns a byte it cannot read into a replacement mark: I left released behavior alone, and it is a cleanup worth making.
- **`Pdf::image_count()`** (`images::count_images`) counts the images the pages paint without decoding one. It is not on `PdfMeta`, so `metadata()` stays as cheap as it was.
- **A file that asks for a password** opens with nothing in it readable. `mimedown` says it is encrypted, warns, and gives no counts, which would each read as none.
- **In `mimedown`,** `document/pdf.rs` fills `title`, `author`, `subject`, `keywords`, `language`, `image_count` and `is_encrypted` from these.
- **Not done:** `pdfdown-node`'s `PdfMeta` does not carry the six new fields; the move into `pdfdown-meta`; a way to stop reading text early; the Illustrator facts.

The tests failed before the code, but for the one on a file that asks for a password, which records what the parser already did.

Checked beyond the tests, with a throwaway program that is not in the repository, on 4,820 PDFs on this machine, four of them copies of a public fixture that `qpdf` encrypted:

| Fact | Against | Result |
| --- | --- | --- |
| Title, author, subject, keywords | `pdfinfo` | The same on all but 30 of 19,276 comparisons. On 12 the title is UTF-8 with no mark, which is read here and comes out garbled there; the rest are strings with control characters in them, or keywords on more than one line |
| Encrypted | `pdfinfo` | The same on every file |
| Language | `exiftool` | 126 files state one; the same on 125, and the other is one it does not report |
| Image count | The decoding path, on the 527 files that have images | Equal on 392. On the other 135 it is higher: it counts images the decoding path cannot decode |

Counting took a millisecond on the median file and a fifth of a second on the slowest.

### The verbs, as built (2026-11-18)

`verbs.rs` and its tests. 320 tests with no feature, 353 with all four.

- **`waveform(bytes, options)`, `poster(bytes, options)` and `keyframes(bytes)`.** Each names the file's type itself and hands the file to the parser that reads it: `audiodown` for MP3 and WAVE, `videodown` for MP4, MOV, WebM and Matroska.
- **None of them fails.** Each returns `Outcome::Made(…)` or `Outcome::Nothing(why)`, and `WhyNot` says why: `NoReader(type)` for a type no reader here gives it for (a picture has no waveform, and FLAC is not decoded here), `FeatureOff(feature)`, `Absent` (a film with no sound), `Unsupported(coding)`, `Truncated`, and `Failed(reason)`.
- **One shape whatever the type:** `Waveform` is the same for a song and for a film's sound. Times are in milliseconds, as everywhere else in `mimedown`.
- **`built_with()`** says which of the four parsers the crate was built with.

Checked on 472 audio and video files on this machine. A waveform was made for all 77 WAVE files and 164 of 166 MP3s (the other two are longer than the cap on decoded samples, and say so), and for every film that has sound. A poster was made for 202 films; each is the type it says, at the size it says, which is the size the film is shown at. Nine films are in codings with no decoder here (VP9, AV1) and one is a fragmented MP4, and each says so.

### Asking by the block, and a WAVE file from its head (2026-11-18)

Both came out of running the binding against real files.

- **Every reader's ask is made up to a block** (`Source::block`, applied in `specs`): at least 4,096 bytes, and at least as much again as the start of the file already given when the ask carries straight on from it. A JPEG with ten segments in front of its frame header took more than eight fetches when each few bytes were asked for in turn; it takes three.
- **A WAVE file's sound is never asked for.** Its header states how many bytes of sound there are, so how long it plays is known from the head. Only what follows the sound is asked for, since what the file is called may be there. A file shorter than its header says, and one too long for the header's four bytes, are still asked for whole. An MP3 is still asked for whole.

### `mimedown-node`, as built (2026-11-18)

Row 7's exports. `src/lib.rs`, `src/types.rs`, `src/mime.rs`, and `__test__/index.spec.ts`. The manifest, `package.json` and the napi setup are yours and were not edited.

- **Functions:** `sniff`, `sniffFromRanges`, `specs`, `specsFromRanges`, `mediaTypesOf`, `mediaTypeOfPath`, `extensionsOf`, `waveform`, `poster`, `keyframes`, `builtWith`; a promise-returning twin for `sniff`, `specs` and the three verbs; and the class `MimeDown`, which holds a file for more than one question.
- **`Mime` is a union of every type's string,** generated from the library's table (158 members). A type added to the library does not compile in the binding until it is added there too.
- **The shapes mirror the library's.** A fact that was not read is `null`. A union is keyed on `type` and holds what it carries under the key of the same name: `{ type: 'image', image: … }`. This is plainer than section 5's model, which flattened the facts into the answer; say if you want that instead.
- **Promises run on libuv's pool** (`AsyncTask`), not on tokio as the other packages' do: your manifest takes `napi` without `tokio_rt`. The switch is one feature and a helper.
- **Which parsers the addon holds is the manifest's to say.** `mimedown.workspace = true` builds none: 1.4 MB, every type named, header facts only. With all four it is 9 MB. To try it I passed `--features mimedown/pdf,mimedown/office,mimedown/audio,mimedown/video` to the build and edited nothing.
- **Not built:** the `ci/` folder, the WebAssembly build, a README, a `version.sh` target.

The public spec (16 tests) runs on the four Illustrator fixtures and on bytes built in the test, and passes against the addon built either way. A second spec, in the folder git ignores, asks for every file on your private list the way a fetcher does; its results are yours and are not written here.

### The inspection before 0.1.0 (2026-11-18)

A review of `crates/mimedown/src` by a reader that had not written it, and a throwaway program that fed the crate damaged copies of real files (cut short at every length, and with bytes flipped, zeroed, cut out and doubled), read whole and by ranges, in every feature set. Each finding below was reproduced by a test that failed, then fixed.

- **A request for more that never ended.** Text that opens with `<` and is not markup (a PHP file with no closing tag, a merge conflict) asked for the rest of the file, and a whole file over 64 KiB asked for a range of no bytes. Markup now has three answers (`text/markup.rs`, `Opening`: a root, cut short, not markup), and text is asked for as far as it is read and no further. A WAVE file of unknown size asked again for what follows its sound after it was given.
- **HTML named as plain text.** A first tag with a bare value (`<html lang=en>`) or a name alone was not read. The attribute reader takes quoted values, bare ones and names alone.
- **TIFF.** A BigTIFF that states an impossible count of entries overflowed an addition; it is now a file that ends early. A chain of directories that leads back on itself was followed 100,000 times; the count now stops there, with a warning. A value whose bytes lie past the end of the file took every other fact with it; it is now left out alone (`if_there` in `container/ifd.rs`), with a warning for a TIFF and silently inside a picture's EXIF block. A pointer of zero is no directory.
- **Office files from part of a file.** Whether a file is macro-enabled or a template was read only from a whole file, and from ranges the plain type came back with nothing asked for. The entry that lists the content types is now read through the source (`zip::part`), inflated when it is deflated, and asked for when it was not given. `sniff` no longer copies the file to do it. `ooxmldown` gained `PackageKind::of_main_type`, which `Package::kind` now uses, so the content types are still mapped in one place.
- **The second look.** The eight bytes a compound file begins with named one behind leading bytes; its directory now has to be there and open with a root storage. Without a byte-order mark, UTF-16 is guessed from eight units or more, and UTF-16 with control characters in it is not text. A PDF's trailer and an MP3's tag are read as soon as the last kilobyte is given, without waiting for the 64 KiB a zip's directory could close in. A start read with allowance still asks for the end of a file whose size is known.
- **Smaller.** A surrogate pair cut by the end of the 64 KiB sample; `.jsonc` and `.json5` by name; a local name with `#` or `?` in it (`report #3.pdf`); RTF behind a byte-order mark read by the RTF reader; a film's bit rate no longer worked out from the part of the file that was given.
- **In `pdfdown`.** A date that holds a character outside ASCII panicked on a slice (`pdf_date_to_iso8601`); it is returned as written.

Left as they are, none of them a wrong answer: an Office file is copied once for each parser that opens it in `specs`; `document::text::read` decodes the whole file to count it; a few helpers and constants are written out more than once (`PREVIEW_CHARACTERS`, the split of keywords, `heif::top_level` beside `isobmff::box_at`); a file of the two bytes `FF 0A` is named JPEG XL.

A contract to state in the README: a range counted from the end of the file can only be placed by a caller that then passes the file's size. With no size, an answer that asked for the end asks for it again.

Not named yet, and common enough to add: Photoshop (`.psd`), AIFF, and an Outlook message (`.msg`, which is a compound file and comes back as `application/x-cfb`).

### After 0.1.0: XHTML (2026-11-19)

A page with XHTML's doctype and namespace that opens as HTML does was named `application/xhtml+xml`, and saved as `.xhtml` a browser read it with its XML parser and showed nothing: such pages are served as HTML and seldom parse as XML. A page is now XHTML only when it opens with an XML declaration and names the namespace (`Root::media_type` in `text/markup.rs`); otherwise an `html` root is `text/html`. Not yet released.

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
- **What an application left in the file (`/PieceInfo`), raised by you on 2026-11-17.** An Illustrator document saved as `.ai` is a PDF: it opens with `%PDF-`, `sniff` names it `application/pdf` from its first bytes, and `pdfdown` reads it as any other. What makes it an Illustrator document is a page-piece dictionary, which the format allows on a page, on a form or on the catalog. Probed on the four `.ai` fixtures in `crates/mimedown-node/__test__/fixtures` (Illustrator 29.8, PDF 1.6), where it sits on the page:
  - `/PieceInfo /Illustrator /LastModified`, a date.
  - `/Private /AIMetaData`: a short stream, not compressed, that reads as the header of a PostScript file: Illustrator's version (`%%AI8_CreatorVersion`), the document's title, who saved it (`%%For`), its bounding box, its file format number (`%AI5_FileFormat`), its color model.
  - `/Private /AIPrivateData1` to `12`: the native document in blocks of 65,536 bytes, about three fifths of each file. The first block opens with `%AI24_ZStandard_Data`, so it is compressed with Zstandard, which nothing in the workspace reads.
  - `/Private /ContainerVersion`, `/CreatorVersion`, `/NumBlock`, `/RoundtripStreamType`, `/RoundtripVersion`: numbers.

  What I would add to `PdfMeta`, if you want it: which applications left a piece (`Illustrator`, and whatever else is found), and for Illustrator its version, title, bounding box and how much of the file its private data takes. The type stays `application/pdf`. It has to be found by walking the pages, not by searching the bytes: from PDF 1.5 a page's dictionary may sit inside a compressed object stream, where a search cannot see it. `%%For` is a person's name and would not be returned by default. An `.ai` from before Illustrator 9 is PostScript, not PDF, and is not named until the text step.

**`xlsxdown` has `metadata()`: built 2026-11-17 and released as `xlsxdown@0.4.0`.** `Workbook::metadata()` in the library, `metadata()` and `metadataAsync()` on `XlsxDown`.

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

## 15. To do

In the order I would take them. Each is its own unit: built, tested, reported, then the next on your go.

| # | To do | Where | Waits on |
| --- | --- | --- | --- |
| 1 | Your answers to section 1, rows 7 to 25. **Done** | This file | |
| 2 | **`xlsxdown`: read macro-enabled workbooks (`.xlsm`) and templates (`.xltx`, `.xltm`)** instead of refusing them, as `docxdown` and `pptxdown` read theirs. Macros are never run. `metadata()` gains `hasMacros` and `isTemplate`. Today `Workbook::open` returns `UnsupportedFormat` for `PackageKind::MacroWorkbook` and `TemplateWorkbook` (`unsupported_detail` in `crates/xlsxdown/src/workbook.rs`), and the public spec and `tests/workbook.rs` assert that refusal for `macro.xlsm`, so those assertions change with it. A Strict workbook stays refused: its namespaces differ | `crates/xlsxdown`, `crates/xlsxdown-node` | Your go |
| 3 | `pdfdown`: `title`, `author`, `subject`, `keywords`, `isEncrypted` and `language` on `PdfMeta`; an image count that does not decode. **Built 2026-11-18 in the library. In `pdfdown-node` since 2026-11-19: the six fields on `PdfMeta`, `PdfDocument` and `PdfDocumentOcr`, and the count as `imageCount()` / `pdfImageCount()` with their promise twins, in the base, OCR and WebAssembly builds. Not yet released.** Still to do: a way to stop reading text early (section 14). **With it, the move into `pdfdown-meta`** (row 13): `meta.rs` is 271 lines and needs only lopdf and `contentdown`, so it moves as it is, with `PdfMeta`, `PageBox` and `BoxType`; `pdfdown` re-exports them, so nothing changes for its users. The one knot is `PdfMeta.form`, which is filled by the form scan (`forms.rs`, 1,119 lines): either the scan moves too, or `pdfdown-meta` leaves `form` empty and `pdfdown` fills it. `pdfdown-meta` is a one-line crate today that `pdfdown`, `pdfdown-node` and `pdfdown-ocr` already list | `crates/pdfdown/src/meta.rs`, `images.rs`, `crates/pdfdown-meta` | Your go |
| 4 | `xlsxdown`: a standalone `metadata` function beside the class method, if you want one; and `text()`, which the other two packages have and a `textPreview` needs | `crates/xlsxdown-node/src/lib.rs` | Your call |
| 5 | `mimedown` units 1 to 9 of section 10: the sniff, the containers, the ten image formats, the rest of `ImageFormat`, Office by names, ranges. **Units 1 to 8, Office, all four features, `retryOnUnknown` and ranges end to end are built. The waveform, poster and keyframe verbs are built too. Nothing of this row remains** | `crates/mimedown` | Your go on each |
| 6 | `mimedown` unit 11: the `pdf`, `office`, `audio` and `video` features | `crates/mimedown` | Items 1 and 3 |
| 7 | `mimedown-node`: the binding. **The exports, a public spec, the `ci/` folder with its workflow (`.github/workflows/mimedown-node.yml`) and a spec for the WebAssembly build (`__test__/wasi.spec.ts`) are built (2026-11-18). The addon is built with all four parsers: you turned the features on in its manifest (2026-11-18). A README remains.** Its manifest, `package.json` and napi setup are yours; the exports, tests and `ci/` folder follow the other packages | `crates/mimedown-node` (new) | You creating the crate |
| 8 | `version.sh`: a `mimedown` target (two manifests and one `package.json`). **Done 2026-11-18** | `version.sh` | Item 7 |
| 9 | In `@d0paminedriven/metadata`: `Extract` and `ExtractClient` call `mimedown` (section 12) | Another repository | Yours to schedule |
| 10 | `pdfdown-ocr`: try the page's `/Rotate` turn first, and stop the blind retry on a page's other rasters once one has read convincingly. Measured after the rasterizing pass as where most of tier 2's time goes | `crates/pdfdown-ocr/src/ocr.rs` | Your go |

## Parked

- Fetching in Rust (`reqwest`, `tokio`). Dropped by decision 3.
- PDF facts from part of a file, beyond section 7.10. The page model's old home, `pdfdown-meta`, is empty and reserved for PDF metadata; a metadata-only load of a whole PDF belongs there.
- `dominantColorHex` (decision 23).
- Facts for the audio and video types the two parsers do not read.
- Telling CSV from TSV from plain text by content; naming the language of a text.
- More image formats: JPEG XL, JPEG 2000, PSD, camera raw, KTX2 (commented out in the TypeScript package).
- Facts from inside a Wallet pass, read from `pass.json` (one small entry to inflate). The keys are Apple's own, public schema:
  - What it is: the style (`boardingPass`, `coupon`, `eventTicket`, `generic`, `storeCard`), `formatVersion`, `passTypeIdentifier`, `serialNumber`, `teamIdentifier`.
  - What it says: `organizationName`, `description`, `logoText`, and how many primary, secondary, auxiliary, header and back fields it has.
  - When: `expirationDate`, `relevantDate`, `voided`.
  - Its barcodes' formats, and whether it names a web service to update from.
  - Which pictures it carries (`icon`, `logo`, `thumbnail`, `strip`, `background`, `footer`) and at which scales, from the entry names alone.
  - **Never returned:** `authenticationToken`, which is a secret, and by default the barcode's message, which is often an identifier of a person.
- Decoding a HEIC to pixels with `heic-rs`. Nothing in the workspace can today, and a phone's photograph of an invoice is a HEIC: it would be the way into OCR for such a picture, and into returning a HEIC embedded in a docx or a pptx as a PNG. Not sniffing, so not this crate's.
- `ooxmldown`'s `image_info`, which names an embedded picture from its first bytes, sharing code with `image/`. `mimedown` depends on `ooxmldown`, so the shared code would have to live in `ooxmldown` or below it.

## Not verified

- Nothing after unit 4 is compiled.
- Camera raw on a camera's own file: none is on this machine and no encoder here writes one, so DNG, CR2, NEF, ARW and the general raw type are tested on directories built in memory, and CR3, RAF, ORF and RW2 on their first bytes alone.
- How a Radiance picture and a JPEG XR are turned, by decoding one: the numbers come from each format's definition, and no decoder here applies them.
- The items under "Read in the source, not run" in section 2. Only the WebP, HEIC and BMP results were run.
- `heic-rs` itself: I read its source and README. I did not build it or run it.
- That every Office writer puts the names section 7.8 relies on where it expects. `ooxmldown` settles it when the whole file is given.
- How many real files answer in one step and how many need the second range.
- The earlier finding that lopdf's metadata-only load is far faster than a full one was measured for `pdfdown`, not for this plan.
- Section 13 against the real tables: I read the schema file, not the code that writes the rows, so a column filled today by something other than the TypeScript package may be missing from my account.
- `xlsxdown`'s chart, pivot table and link counts on a real file: no public fixture holds any, so they are tested on parts built in memory.
- The size of an addon holding all six parsers, and the time `specs` takes on a large PDF.
- That each parser's metadata is cheap enough to call for a row. `docxdown` and `pptxdown` build their whole model to answer.
