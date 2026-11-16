# assetdown

Rust parsers for documents and media, each published to npm as a native Node addon with a WebAssembly build beside it.

Every format has two crates: a library that reads the format and knows nothing about Node, and a `-node` crate that binds it and is the npm package.

## npm packages

All need Node 24 or later.

| Package                                                                                    | Reads                                                                                                                                                      | Main export | Binding crate                                        |
| ------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------- | ---------------------------------------------------- |
| [`@d0paminedriven/pdfdown`](https://www.npmjs.com/package/@d0paminedriven/pdfdown)         | PDF: text, layout, images, annotations, form fields, attachments, metadata, invoice fields, page rendering                                                 | `PdfDown`   | [`crates/pdfdown-node`](crates/pdfdown-node)         |
| [`@d0paminedriven/pdfdown-ocr`](https://www.npmjs.com/package/@d0paminedriven/pdfdown-ocr) | Everything pdfdown reads, plus OCR for scanned pages. Needs a system Tesseract                                                                             | `PdfDown`   | [`crates/pdfdown-node/ocr`](crates/pdfdown-node/ocr) |
| [`@d0paminedriven/xlsxdown`](https://www.npmjs.com/package/@d0paminedriven/xlsxdown)       | xlsx: cells, formulas, sheet geometry, layout, invoice fields                                                                                              | `XlsxDown`  | [`crates/xlsxdown-node`](crates/xlsxdown-node)       |
| [`@d0paminedriven/docxdown`](https://www.npmjs.com/package/@d0paminedriven/docxdown)       | docx: text, blocks, Markdown, tables, images, annotations, form fields, notes, metadata, layout, invoice fields                                            | `DocxDown`  | [`crates/docxdown-node`](crates/docxdown-node)       |
| [`@d0paminedriven/pptxdown`](https://www.npmjs.com/package/@d0paminedriven/pptxdown)       | pptx: text, slides and their shapes, Markdown, tables, images, annotations, notes, charts as data, audio and video clips, metadata, layout, invoice fields | `PptxDown`  | [`crates/pptxdown-node`](crates/pptxdown-node)       |
| [`@d0paminedriven/audiodown`](https://www.npmjs.com/package/@d0paminedriven/audiodown)     | MP3 and WAV: metadata, PCM decoding, waveform peaks                                                                                                        | `AudioDown` | [`crates/audiodown-node`](crates/audiodown-node)     |
| [`@d0paminedriven/videodown`](https://www.npmjs.com/package/@d0paminedriven/videodown)     | MP4, MOV, WebM and MKV: metadata, keyframe index, cover art, poster image, audio-track waveform peaks                                                      | `VideoDown` | [`crates/videodown-node`](crates/videodown-node)     |

Each package has a class that parses a buffer once and is read from as often as needed, free functions that take a buffer for one call, and `...Async` forms that do their work off the JavaScript thread.

## Crates

### Bindings 

One npm package each. `pdfdown-node` builds two: the base package, and the OCR superset with its `ocr` feature.

| Crate                                     | npm package                                              | Binds                    |
| ----------------------------------------- | -------------------------------------------------------- | ------------------------ |
| [`pdfdown-node`](crates/pdfdown-node)     | `@d0paminedriven/pdfdown`, `@d0paminedriven/pdfdown-ocr` | `pdfdown`, `pdfdown-ocr` |
| [`xlsxdown-node`](crates/xlsxdown-node)   | `@d0paminedriven/xlsxdown`                               | `xlsxdown`               |
| [`docxdown-node`](crates/docxdown-node)   | `@d0paminedriven/docxdown`                               | `docxdown`               |
| [`pptxdown-node`](crates/pptxdown-node)   | `@d0paminedriven/pptxdown`                               | `pptxdown`               |
| [`audiodown-node`](crates/audiodown-node) | `@d0paminedriven/audiodown`                              | `audiodown`              |
| [`videodown-node`](crates/videodown-node) | `@d0paminedriven/videodown`                              | `videodown`              |

### Format libraries

Plain Rust, with no dependency on Node.

| Crate                               | What it does                                                            | Uses                                     |
| ----------------------------------- | ----------------------------------------------------------------------- | ---------------------------------------- |
| [`pdfdown`](crates/pdfdown)         | PDF extraction, with page rendering behind its `render` feature         | `pdfdown-meta`, `contentdown`            |
| [`pdfdown-ocr`](crates/pdfdown-ocr) | pdfdown plus OCR through Tesseract                                      | `pdfdown`, `pdfdown-meta`, `contentdown` |
| [`xlsxdown`](crates/xlsxdown)       | SpreadsheetML workbooks read into a sparse cell model                   | `ooxmldown`, `contentdown`               |
| [`docxdown`](crates/docxdown)       | WordprocessingML documents read into blocks                             | `ooxmldown`, `contentdown`               |
| [`pptxdown`](crates/pptxdown)       | PresentationML decks read into slides and their shapes                  | `ooxmldown`, `contentdown`               |
| [`audiodown`](crates/audiodown)     | MP3 and WAV metadata, PCM decoding, waveform peaks                      |                                          |
| [`videodown`](crates/videodown)     | MP4, MOV, WebM and MKV demux, metadata, keyframes, posters, audio peaks |                                          |

### Shared crates

| Crate                                 | What it does                                                                                                                                                                                | Used by                                                      |
| ------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------ |
| [`contentdown`](crates/contentdown)   | Structured content from pages of positioned text: lines from words, invoice fields, paper sizes; and what the flow formats share: setting text by estimate, Markdown writing, list counting | `pdfdown`, `pdfdown-ocr`, `xlsxdown`, `docxdown`, `pptxdown` |
| [`ooxmldown`](crates/ooxmldown)       | The OOXML package layer: the zip within limits, relationships, document properties, image headers, charts as data, cell addresses                                                           | `xlsxdown`, `docxdown`, `pptxdown`                           |
| [`pdfdown-meta`](crates/pdfdown-meta) | Plain data types for PDF pages                                                                                                                                                              | `pdfdown`, `pdfdown-ocr`                                     |

### How they fit together

Each arrow reads "is built on".

```text
pdfdown-node    →  pdfdown-ocr  →  pdfdown  →  pdfdown-meta, contentdown
xlsxdown-node   →  xlsxdown     →  ooxmldown, contentdown
docxdown-node   →  docxdown     →  ooxmldown, contentdown
pptxdown-node   →  pptxdown     →  ooxmldown, contentdown
audiodown-node  →  audiodown
videodown-node  →  videodown
```

## Layout

```text
crates/
├─ <name>/            the library: reads the format
└─ <name>-node/       the binding: the npm package
   ├─ src/            what JavaScript sees
   ├─ __test__/       node:test specs and their fixtures
   └─ ci/             composite actions, and the target matrix read from package.json
.github/workflows/    one thin workflow per package
BRAINSTORM/           plans
```

## Working here

You need Rust (edition 2024, 1.99.1 or later), Node 24 or later, and Yarn 4.

```sh
# Rust: every crate but the one that links the system Tesseract
cargo test --workspace --exclude pdfdown-ocr

# One package
cd crates/docxdown-node
yarn install
yarn build        # the native addon, with index.js and index.d.ts
yarn test
yarn build:wasm   # the WebAssembly build
yarn test:wasi
```

`index.js`, `browser.js` and the WASI loader files are generated by the build and are not in git, so a package cannot be imported until `yarn build` has run. `index.d.ts` is generated too, and is kept in git so that a change to a package's types shows in a diff.

## Releases

A commit to `main` titled `<name>@<version>`, with the version in that package's `package.json`, publishes that package: `pptxdown@0.1.0`, `videodown@0.3.0`. `pdfdown@<version>` publishes both `pdfdown` and `pdfdown-ocr`. Such a commit runs only the pipeline of the package it names. Any other commit builds and tests whatever it touched and publishes nothing.

## License

MIT
