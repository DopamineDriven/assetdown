# Config updates

What I need from the manifests and configuration, which are yours, to keep going. Each entry has the file it belongs in above it. They are in the order I will run into them: section 1 is done, sections 2 and 3 open the next two crates, and section 4 is a decision and not yet a need.

Nothing here adds a third-party dependency: every outside crate named is already in the workspace's table. The only new names are the workspace's own crates.

## 1. `tiffdown-node`: the OCR package

Done (2026-11-19). You added `ocr/package.json`, the four scripts, and the ignore rule for the generated loader; the `[features]` block in `crates/tiffdown-node/Cargo.toml` was already there. I made `crates/tiffdown-node/ci/` (the `pdfdown-node` folder with the names changed and no `render` feature), `.github/workflows/tiffdown-node.yml`, and the `tiffdown` entry in `version.sh`, which moves four files together: the two `package.json` files, the binding's `Cargo.toml`, and the library's.

The workflow's lint job installs Tesseract and, beyond what `pdfdown-node`'s runs, checks the code behind `ocr`:

```sh
cargo clippy --package tiffdown --package tiffdown-node --all-targets --all-features -- -D warnings
cargo test --package tiffdown --features ocr
```

One thing left, and it blocks nothing: the keywords in `crates/tiffdown-node/package.json` are still the ones `mimedown` has.

### `crates/tiffdown-node/package.json`

```json
"keywords": [
  "N-API",
  "NAPI",
  "Rust",
  "napi-rs",
  "node-addon",
  "node-addon-api",
  "tiff",
  "fax",
  "scan",
  "extraction"
]
```

## 2. `jpegdown`

The next crate by what the archive holds. It is thin: `image` decodes, `ocrdown` reads, and `contentdown` has the page and the invoice. The plan is [jpegdown.md](jpegdown.md).

### `Cargo.toml`

The root. Two members, and two of the workspace's own, each in its alphabetical place:

```toml
# [workspace] members
  "crates/jpegdown",
  "crates/jpegdown-node",

# [workspace.dependencies], the workspace's own
jpegdown       = { path = "crates/jpegdown" }
jpegdown-node  = { path = "crates/jpegdown-node" }
```

### `crates/jpegdown/Cargo.toml`

```toml
[package]
description          = "Convenient jpeg extraction"
edition.workspace    = true
keywords             = ["jpeg", "scan", "photo", "extraction", "images"]
license              = { workspace = true }
name                 = "jpegdown"
repository.workspace = true
version              = "0.1.0"

[dependencies]
contentdown.workspace = true
image.workspace       = true
ocrdown               = { workspace = true, optional = true }
thiserror.workspace   = true

[features]
ocr = ["dep:ocrdown"]
```

What each is for: `image` the decoding and the turn a picture's orientation asks for, `contentdown` the page and the paper sizes, `thiserror` the three kinds of error, and `ocrdown` the words, only with the feature.

### `crates/jpegdown-node/Cargo.toml`

```toml
[package]
description            = "Convenient jpeg extraction for Node"
edition.workspace      = true
keywords               = ["jpeg", "scan", "photo", "extraction", "node-bindings", "rust"]
license                = { workspace = true }
name                   = "jpegdown-node"
publish                = false
repository.workspace   = true
rust-version.workspace = true
version                = "0.1.0"

[lib]
crate-type = ["cdylib"]

[dependencies]
contentdown.workspace = true
jpegdown.workspace    = true
napi.workspace        = true
napi-derive.workspace = true

[features]
ocr = ["jpegdown/ocr"]

[build-dependencies]
napi-build.workspace = true
```

The rest of the binding's folder is the scaffold `tiffdown-node` has, with section 1's four scripts, its two `.gitignore` lines, and its `ocr/package.json` in from the start, under this package's names (`@d0paminedriven/jpegdown-ocr`, `jpegdown-node-ocr`, `jpegdown_ocr.napi.js`).

## 3. `rtfdown`

After `jpegdown`. Its first use is the invoice attached inside a mail message saved as RTF. The plan is [rtfdown.md](rtfdown.md).

### `Cargo.toml`

The root:

```toml
# [workspace] members
  "crates/rtfdown",
  "crates/rtfdown-node",

# [workspace.dependencies], the workspace's own
rtfdown        = { path = "crates/rtfdown" }
rtfdown-node   = { path = "crates/rtfdown-node" }
```

### `crates/rtfdown/Cargo.toml`

```toml
[package]
description          = "Convenient rtf extraction"
edition.workspace    = true
keywords             = ["rtf", "rich-text", "extraction", "documents"]
license              = { workspace = true }
name                 = "rtfdown"
repository.workspace = true
version              = "0.1.0"

[dependencies]
contentdown.workspace = true
encoding_rs.workspace = true
thiserror.workspace   = true
```

What each is for: `encoding_rs` the code pages a file's text is written in, `contentdown` the text set on pages and the invoice, and `thiserror` the three kinds of error. The reader of the compound file an embedded object is kept in is written in the crate.

### `crates/rtfdown-node/Cargo.toml`

```toml
[package]
description            = "Convenient rtf extraction for Node"
edition.workspace      = true
keywords               = ["rtf", "rich-text", "extraction", "node-bindings", "rust"]
license                = { workspace = true }
name                   = "rtfdown-node"
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
rtfdown.workspace     = true

[build-dependencies]
napi-build.workspace = true
```

It has no OCR package: nothing in it needs Tesseract.

## 4. A decision, not yet a need

Row 3 of [rtfdown.md](rtfdown.md) recommends that `mimedown`'s RTF reader move into `rtfdown`, and that `mimedown` then take its RTF facts from `rtfdown` behind a feature, as it takes a Word file's from `docxdown`. If you take that row, these follow; if you do not, `mimedown` keeps its own reader and nothing here changes.

### `crates/mimedown/Cargo.toml`

```toml
# [dependencies]
rtfdown = { workspace = true, optional = true }

# [features]
rtf = ["dep:rtfdown"]
```

### `crates/mimedown-node/Cargo.toml`

`rtf` joins the features the binding turns on:

```toml
mimedown = { workspace = true, features = ["pdf", "office", "audio", "video", "rtf"] }
```

## Nothing needed for

- The rest of `tiffdown`'s codings (Deflate, PackBits, Group 3, JPEG, tiles): `flate2`, `hayro-ccitt`, and `image` are already in its manifest.
- A working resolution for small pictures in `ocrdown`: `image` resizes.
- The change to how `pdfdown` reads a page's text: no manifest is touched. It does need `pdfdown` and `mimedown` released for it to reach the packages.
