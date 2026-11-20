# Config updates

What I need from the manifests and configuration, which are yours, to keep going. Each entry has the file it belongs in above it. They are in the order I ran into them: sections 1 to 3 are done, section 4 is a decision that holds up nothing, and section 5 is how the releases that are waiting have to be cut.

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

Done (2026-11-19). You made the two manifests and the root's entries. The binding's folder held nothing else, so I made the rest of its scaffold as a copy of `tiffdown-node`'s with the names changed (`package.json`, `ocr/package.json`, `build.rs`, `.gitignore`, `.yarnrc.yml`, `.oxfmtrc.json`, `tsconfig.json`, `.cargo/config.toml`, `yarn.lock`), then its `ci` folder, `.github/workflows/jpegdown-node.yml`, and its entry in `version.sh`. All of it is yours to change.

One difference from `tiffdown-node`: its `.gitignore` has `/index.js`, which ignores the generated loader by itself, so the root `.gitignore` needs no line for this package.

## 3. `rtfdown`

Built (2026-11-20). The library is mine by your word; you made `crates/rtfdown-node` and its manifests. In the root `Cargo.toml` I added `"crates/rtfdown"` among the members and `rtfdown = { path = "crates/rtfdown" }` among the workspace's own, and to the library's manifest `contentdown.workspace = true` and `encoding_rs.workspace = true`, for the text and the pages.

In the binding I wrote the sources, the specs, `build.rs` (which `napi build` needs, and which is the one every binding has), the `ci` folder, `.github/workflows/rtfdown-node.yml`, and the entry in `version.sh`. I changed nothing in its manifests.

Settled by you on 2026-11-20: the package's name, which was spelled `rtfdowm` in four places of `package.json`; its version, now `0.1.0` in all three files; and the generated files that carried the old spelling, rebuilt.

### `crates/rtfdown/Cargo.toml`

As it stands:

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

### `crates/rtfdown-node/Cargo.toml`

Yours, and nothing in it needs to change. It has no OCR package: nothing in it needs Tesseract.

## 4. A decision, not a need

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

## 5. Cutting the releases

CI reads one thing to decide a release: the subject of the newest commit in a push, which must be exactly `<name>@<version>`. And a workflow only starts when the push touches its package's files. Two things follow.

- **One push to a release.** Two release commits pushed together publish only the later one.
- **A release commit has to touch its own package.** `jpegdown` and `rtfdown` are already at `0.1.0` in every file, so `version.sh` has nothing to change for them, and a commit that changes nothing of theirs starts no workflow. They are both new and uncommitted, so each can be released by the commit that adds something of its own.

In this order, each line a commit and a push of its own:

| Commit | What it holds |
|---|---|
| `jpegdown@0.1.0` | everything that is waiting, except `.github/workflows/rtfdown-node.yml` and `crates/rtfdown-node/ci` |
| `rtfdown@0.1.0` | those two, which start its workflow |
| `pdfdown@0.19.0` | what `./version.sh pdfdown 0.19.0` changes: the fix to how a page's text is read is already in the tree |
| `mimedown@0.3.0` | what `./version.sh mimedown 0.3.0` changes: its addon is built with `pdfdown` inside it, so `isSearchable` is only right once it is rebuilt, and it gains the three routes |

None of these pushes cancels a release that is still running, so they need not wait for one another: the only workflow one of them starts a second time is `mimedown-node`, whose first run is a skip. `tiffdown` would take the changes in `ocrdown` (the choice of thresholding a page at a time, and no enlarging of a page already near 200 pixels to the inch) with a `tiffdown@0.2.0` of its own.

## Nothing needed for

- The rest of `tiffdown`'s codings (Deflate, PackBits, Group 3, JPEG, tiles): `flate2`, `hayro-ccitt`, and `image` are already in its manifest.
- A working resolution for small pictures in `ocrdown`: `image` resizes.
- The change to how `pdfdown` reads a page's text: no manifest is touched. It does need `pdfdown` and `mimedown` released for it to reach the packages.
