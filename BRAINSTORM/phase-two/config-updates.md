# Config updates

What I need from the manifests and configuration, which are yours, to keep going. Each entry has the file it belongs in above it. They are in the order I ran into them: sections 1 to 3 are done, section 4 is a decision that holds up nothing, section 5 is how releases have to be cut, section 6 is the pilot of a WebAssembly package of its own, and section 7 is what the legacy Office crates need.

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

## 6. A WebAssembly package of its own: the pilot in `rtfdown-node`

Tried on 2026-11-20, and reshaped the same day. `crates/rtfdown-node/wasm` is one package, `@d0paminedriven/rtfdown-wasm`, that holds both WebAssembly builds and everything they need. Its entries are the builds' own loaders, so nothing in it looks for a native addon and its types name a WebAssembly build exactly. Its manifest is kept by hand, as you said most are. Nothing here is published.

| Entry | Gives | Runs |
|---|---|---|
| `@d0paminedriven/rtfdown-wasm` | in Node, the build with threads; to a bundler building for a browser, the build without | see the two rows below |
| `…/threads` | the build with threads (`wasm32-wasip1-threads`) | in Node, and on a page that is cross-origin isolated; the `*Async` verbs work off the main thread |
| `…/threadless` | the build without (`wasm32-wasip1`) | anywhere WebAssembly does, a plain page included; the `*Async` verbs answer rightly and hold the main thread while they work |
| `…/workerd` with `…/wasm.wasm` | the build without threads, started from a module the caller compiled | a Cloudflare Worker |

Checked: 21 tests, which import the package by its own name as a caller would; the type check, which fails if `native` comes back into the type; and the tarball `npm pack` makes (14 files, 855 kB), unpacked into a project of its own and loaded by name, by `threadless`, and by `workerd`. Not checked: a real browser, or a bundler.

What is kept by hand, and what holds it to account in `wasm/__test__/manifest.spec.ts`:

- The `files` list and the map of entries: every path the manifest names must be one it ships, and every file it ships must be there once it is built.
- The three dependencies the loaders import: they must be the ones, at the versions, that `napi create-npm-dirs` would write. The test runs it and compares.
- `rtfdown-node-wasm.wasm32-wasip1.wasm.d.ts`, three lines that type the compiled module. `napi` writes this file only when it assembles a package of its own.

### `crates/rtfdown-node/.gitignore`

You have the loaders ignored. One declaration is worth ignoring too, against the rule that declarations are tracked: `napi build` still writes a root `index.d.ts` into the folder, the package does not ship it, and it is the one that types the target as `'native' | …`:

```gitignore
wasm/index.d.ts
```

`wasm/npm`, which you have ignored already, is made and removed by the manifest test.

### `crates/rtfdown-node/.oxfmtrc.json`

So that `yarn format` leaves the generated files alone, among `ignorePatterns`:

```json
"*.wasip1.cjs",
"*.wasip1-browser.js",
"*.wasip1-deferred.js",
"*.wasip1.d.cts",
"*-deferred.d.ts"
```

### `crates/rtfdown-node/package.json`

Among `scripts`. `--no-js` leaves out the root entry that looks for a native addon:

```json
"build:wasm-package": "yarn build:wasm-package:threads && yarn build:wasm-package:threadless",
"build:wasm-package:threads": "napi build --platform --esm --release --no-js --target wasm32-wasip1-threads --package-json-path wasm/package.json --output-dir wasm",
"build:wasm-package:threadless": "napi build --platform --esm --release --no-js --target wasm32-wasip1 --package-json-path wasm/package.json --output-dir wasm",
"test:wasm": "yarn tsx --test \"wasm/__test__/**/*.spec.ts\""
```

### CI and the version, as wired (2026-11-21)

- `crates/rtfdown-node/ci/wasm`: installs both Rust targets, runs `yarn build:wasm-package`, type-checks and runs the package's tests, lists what `npm pack` would ship, and keeps the built files for the publish job. The workflow runs it on Node 26 and on Node 24, and the Node 26 run is the one that keeps them.
- `crates/rtfdown-node/ci/publish-wasm`: takes those files, and on a release commit runs `npm publish` in `wasm/`. It needs none of `create-npm-dirs`, `artifacts`, or `pre-publish`.
- `.github/workflows/rtfdown-node.yml`: a `wasm` job and a `publish-wasm` job. The main package's `publish` now waits on `wasm` too, as the packages with an OCR folder wait on theirs, so neither goes out when the other's tests fail.
- `version.sh`: `rtfdown` now moves four files together, `wasm/package.json` among them.

So the next `rtfdown@<version>` commit publishes `@d0paminedriven/rtfdown-wasm` for the first time, beside the main package and at its version. None of this has run on GitHub: its first run is that release, or any push that touches the package before it.

### A worker entry, tried on a real page (2026-11-21)

`@d0paminedriven/rtfdown-wasm/worker` gives the same verbs from a Web Worker of its own, each answered by a promise, so that a page is left free while a document is read and `terminate()` gives the build's memory back. It is the browser's only: there is no Node build of it yet.

- `wasm/src/protocol.ts`: the messages, as unions on `type`, and your types that read a verb's arguments and answer off the build by its name (`BindingRT<K>`, `BindingArgs<K>`), so that no list of verbs is kept by hand there. `wasm/src/worker.ts`: what runs in the worker. `wasm/src/client.ts`: the `RtfWorker` class a page imports. `wasm/src/buffer.ts`: see below.
- The client's verbs are the build's own promise verbs by the same names (`rtfTextAsync`, `rtfLayoutAsync`, and the rest), and the class `implements` the type that says so, so code written against the build's promises runs on the worker, and a promise verb added in Rust stops the type check until the worker has it.
- `wasm/tsdown.config.ts`, in the shape of your other configs, as two builds into `wasm/dist`: the client left as the files it is, and the worker bundled into one file with its runtime inside, because a worker is loaded by its address. The worker's build has to be `platform: "browser"`, which is what picks the runtime a page can run, and `unbundle: false`.
- The `.wasm` reaches the worker through `rolldown-plugin-wasm`, as you suggested: the worker imports it with `?url`, the bundler puts it in `dist` as `worker.wasm` and says where, and the worker compiles it and hands the module to the loader napi writes for a worker runtime (`…/workerd`). That loader starts the build at 64 MiB where the page's own starts at 250, and it lets the worker tell the page when the build could not start.
- `build:wasm-package` now ends with `build:wasm-package:worker`, which runs `tsdown` from `wasm/` by the package's own binary: yarn will not run a script from a folder that has a manifest and is not a workspace.
- `wasm/__test__/browser.spec.ts` serves `wasm/dist` with no header that isolates the page, and drives the Chrome on the machine through `playwright-core`. It holds the worker's answers against what the same build gives in Node, counts a page timer's ticks while a long document is set on pages, and checks that errors and an ended worker reject.

What the page run found:

- **A page has no `Buffer`, and the build gives its bytes back as one.** On a page, any verb that returns bytes (`rtfEmbedded`, `rtfPictures`, and a document that carries either) throws in the runtime under the build. Text, pages, and the invoice are fine. The worker carries a stand-in of a dozen lines, loaded before the build, and its answers are typed as `Uint8Array`, which is what crosses to the page.
- **The entries a page imports directly are not helped by that.** `@d0paminedriven/rtfdown-wasm` on a page still throws for bytes unless the page supplies a `Buffer`. The lasting fix is on the Rust side, giving bytes back as a `Uint8Array`, and it changes what Node callers get too, so it is yours to decide, for every package.
- The page stayed free, and needed no isolation.
- **A call made before the worker had loaded was never answered.** The worker began to listen only once the build was in, and a message that came sooner was dropped. It listens from the start now, and each call waits for the build.
- **A server that does not call the `.wasm` file `application/wasm` stopped the worker from starting**, because a browser compiles a file as it arrives only from one that does. The worker now takes the whole file first from such a server.

Costs: the package is 20 files and 1.31 MB packed, up from 14 files and 855 kB. The worker's file is 502 kB (84 kB gzipped), and the build without threads is in the package twice, once as `dist/worker.wasm`.

The plugin as installed (0.3.2) gives a `.wasm` as an instance it started itself, which does not fit a build whose imports come from napi's runtime, or as an address, which it never inlines. So the worker is two files. One file, as your worklet is, would take the bundler's own binary loader in place of the plugin, at the cost of the `.wasm` growing by a third inside the script.

Three dev dependencies came with this, all in `crates/rtfdown-node/package.json`: `tsdown`, `rolldown-plugin-wasm`, and `playwright-core`. The last drives the Chrome already installed and downloads no browser. `tsdown` warns that its `tsgo` declarations are deprecated on TypeScript 7.0 and wants 7.1.

`wasm/README.md` says which entry to import where, and states the limits above. Writing it is what turned up the last two findings: each was a sentence in it that no test stood behind.

### Still to do

- A Node build of the worker entry, if the same import is to work on a server.
- Bytes on a page without the worker: the decision above.
- A parsed document held in the worker and referred to from the page. The first cut has the six verbs that take bytes and nothing that keeps state.
- `napi.wasm.threadlessInitialMemory` in `wasm/package.json`: imported directly on a page, the build without threads starts at 250 MiB unless told, which is more than a phone may give. The worker no longer has this: it starts at 64 MiB.
- A copy command for an app whose bundler does not pick up a worker's file, as `sttworklet` is for your worklet.

Two things to know. `NAPI_RS_WASI_FLAVOR` speaks to every napi package in the process, and one that ships a single build throws on a name it does not have, so nothing here uses it. And the first shape tried, napi's own (a root package and a package to a build), works too, but its root entry types the target as `'native' | 'wasm32-wasi' | 'wasm32-wasip1'` and looks for native packages that are never published before it falls through.

## 7. The legacy Office crates: `cfbdown`, `xlsdown`, and `docdown`

Planned on 2026-11-22 in [xlsdown.md](xlsdown.md) and [docdown.md](docdown.md). Nothing is built. These are the manifests I need, in the order the work takes them. No crate from outside the workspace is added.

`cfbdown` is a library only: it has no binding and no npm package.

### `Cargo.toml` (the root)

Among `members`:

```toml
"crates/cfbdown",
"crates/docdown",
"crates/docdown-node",
"crates/xlsdown",
"crates/xlsdown-node",
```

Among the workspace's own, under `[workspace.dependencies]`:

```toml
cfbdown      = { path = "crates/cfbdown" }
docdown      = { path = "crates/docdown" }
docdown-node = { path = "crates/docdown-node" }
xlsdown      = { path = "crates/xlsdown" }
xlsdown-node = { path = "crates/xlsdown-node" }
```

### `crates/cfbdown/Cargo.toml`

```toml
[package]
description            = "Compound file reading: the streams and storages of the container legacy Office files are kept in"
edition.workspace      = true
keywords               = ["cfb", "ole", "compound-file", "extraction"]
license.workspace      = true
name                   = "cfbdown"
repository.workspace   = true
rust-version.workspace = true
version                = "0.1.0"

[dependencies]
```

### `crates/rtfdown/Cargo.toml`

One line more, once `cfbdown` exists and the reader has moved into it:

```toml
cfbdown.workspace = true
```

### `crates/xlsdown/Cargo.toml`

```toml
[package]
description            = "xls extraction: the cells, formats, and sheet geometry of an Excel 97 to 2003 workbook"
edition.workspace      = true
keywords               = ["xls", "excel", "extraction", "spreadsheet"]
license.workspace      = true
name                   = "xlsdown"
repository.workspace   = true
rust-version.workspace = true
version                = "0.1.0"

[dependencies]
cfbdown.workspace     = true
contentdown.workspace = true
thiserror.workspace   = true
xlsxdown.workspace    = true
```

### `crates/xlsdown-node/Cargo.toml`

```toml
[package]
description            = "Node bindings for xlsdown: xls extraction, workbook cells, formats, and sheet geometry"
edition.workspace      = true
license.workspace      = true
name                   = "xlsdown-node"
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
xlsdown.workspace     = true
xlsxdown.workspace    = true

[build-dependencies]
napi-build.workspace = true
```

### `crates/docdown/Cargo.toml`

```toml
[package]
description            = "doc extraction: the text, tables, pictures, and embedded files of a Word 97 to 2003 document"
edition.workspace      = true
keywords               = ["doc", "word", "extraction", "documents"]
license.workspace      = true
name                   = "docdown"
repository.workspace   = true
rust-version.workspace = true
version                = "0.1.0"

[dependencies]
cfbdown.workspace     = true
contentdown.workspace = true
encoding_rs.workspace = true
thiserror.workspace   = true
```

### `crates/docdown-node/Cargo.toml`

```toml
[package]
description            = "Node bindings for docdown: doc extraction, text, tables, pictures, and embedded files"
edition.workspace      = true
license.workspace      = true
name                   = "docdown-node"
publish                = false
repository.workspace   = true
rust-version.workspace = true
version                = "0.1.0"

[lib]
crate-type = ["cdylib"]

[dependencies]
contentdown.workspace = true
docdown.workspace     = true
napi.workspace        = true
napi-derive.workspace = true

[build-dependencies]
napi-build.workspace = true
```

### `crates/xlsdown-node/package.json` and `crates/docdown-node/package.json`

Each is `crates/rtfdown-node/package.json` with the name changed, at version `0.1.0`, with the same fourteen targets and without the `wasm` scripts and the three dev dependencies that came with the WebAssembly pilot (`tsdown`, `rolldown-plugin-wasm`, `playwright-core`). The fields that differ:

```json
"name": "@d0paminedriven/xlsdown",
"description": "Convenient Rust-powered xls parsing for Node",
"keywords": ["N-API", "NAPI", "Rust", "napi-rs", "node-addon", "node-addon-api", "xls", "excel"],
"napi": {
  "binaryName": "xlsdown-node",
  "dtsHeader": "/* auto-generated by @d0paminedriven/xlsdown */\n\n",
  "packageName": "@d0paminedriven/xlsdown"
}
```

```json
"name": "@d0paminedriven/docdown",
"description": "Convenient Rust-powered doc parsing for Node",
"keywords": ["N-API", "NAPI", "Rust", "napi-rs", "node-addon", "node-addon-api", "doc", "word"],
"napi": {
  "binaryName": "docdown-node",
  "dtsHeader": "/* auto-generated by @d0paminedriven/docdown */\n\n",
  "packageName": "@d0paminedriven/docdown"
}
```

The rest of each binding's folder is the scaffold `rtfdown-node` has: `build.rs`, `.gitignore`, `.yarnrc.yml`, `.oxfmtrc.json`, `tsconfig.json`, `.cargo/config.toml`, and `__test__/tsconfig.json` and `__test__/workup.ts`. Mine once the folders are there: the sources, the specs, the `ci` folder, the workflow, and the line in `version.sh`.

### `.xlsm`, which needs no manifest

`xlsxdown` refuses a macro-enabled workbook today, and names it. Reading one changes no manifest: the package reader already tells the kinds apart, and the workbook inside is the same.

## Nothing needed for

- The rest of `tiffdown`'s codings (Deflate, PackBits, Group 3, JPEG, tiles): `flate2`, `hayro-ccitt`, and `image` are already in its manifest.
- A working resolution for small pictures in `ocrdown`: `image` resizes.
- The change to how `pdfdown` reads a page's text: no manifest is touched. It does need `pdfdown` and `mimedown` released for it to reach the packages.

### Where the manifests stand, as of 2026-11-23

`cfbdown`, `xlsdown`, `xlsdown-node`, `docdown`, and `docdown-node` are built on the manifests above. You added `cfbdown.workspace = true` to `crates/rtfdown/Cargo.toml`, and `rtfdown` uses the shared reader now. One line I added myself, as you said I may:

#### `crates/docdown/Cargo.toml`

```toml
flate2.workspace = true
```

A metafile in a Word document is kept compressed, and this inflates it. `flate2` was already a dependency of the workspace.

#### `crates/xlsdown-node/wasm/package.json` and `crates/docdown-node/wasm/package.json`

New, made on 2026-11-23 when you asked for the WebAssembly folders to be configured. Each is `rtfdown-node/wasm/package.json` with the names changed (`@d0paminedriven/xlsdown-wasm`, `@d0paminedriven/docdown-wasm`, binaries `xlsdown-node-wasm` and `docdown-node-wasm`), at version 0.1.0 to match its base package, with its own description and keywords. The `build:wasm-package` scripts and `test:wasm` that both bindings inherited from `rtfdown-node` now have a folder to work on, so nothing in either binding's own `package.json` needed to change.

### What changed on 2026-11-24

Both changes were asked for, and are made, not proposed.

#### Locked `.xls` workbooks: two hashing crates

`Cargo.toml` (the root), under `[workspace.dependencies]`:

```toml
md-5 = "0.11.0"
sha1 = "0.11.0"
```

`crates/xlsdown/Cargo.toml`:

```toml
md-5.workspace = true
sha1.workspace = true
```

Excel makes the key of a locked workbook from its password with MD5 (Excel 97 and 2000) or SHA-1 (Excel 2002 and 2003). `md-5` was already in the lock file, through `lopdf`; `sha1` is new to it, and is the same family at the same version. The cipher itself, RC4, is twenty lines in `crates/xlsdown/src/crypt.rs`.

#### `@d0paminedriven/xlsxdown-wasm`: the WebAssembly-only package for `.xlsx`

`crates/xlsxdown-node/wasm/` is new, laid out as `crates/xlsdown-node/wasm/` is.

- `wasm/package.json` is a new manifest: the xls one with the names changed (`@d0paminedriven/xlsxdown-wasm`, binary `xlsxdown-node-wasm`), at 0.6.0 to match its base package. The two must be at the same version when the base is released, which `version.sh` now sees to: `xlsxdown` moves four files, as `rtfdown`, `xlsdown`, and `docdown` do.
- `crates/xlsxdown-node/package.json` gained what the xls binding's already had: the scripts `build:wasm-package` (with its three parts) and `test:wasm`, and the dev dependencies `playwright-core`, `rolldown-plugin-wasm`, and `tsdown`, at the versions `xlsdown-node` uses. `yarn.lock` moved with them.
- `crates/xlsxdown-node/.gitignore` and `.oxfmtrc.json` gained the five lines each that keep the package's generated scripts out of git and out of the formatter. Both files are now the same as the xls binding's.
- `crates/xlsxdown-node/ci/wasm` and `ci/publish-wasm` are the xls binding's with the names changed, and `.github/workflows/xlsxdown-node.yml` has the `wasm` and `publish-wasm` jobs, with `wasm` among what `publish` waits on. The base package's release commit, `xlsxdown@<version>`, publishes both.
- The worker (`XlsxWorker`) answers `spreadsheetCellsAsync`, the one verb the binding has as a function, as `XlsWorker` does.
- Its tests read the public fixtures, and one workbook no fixture is: `crates/xlsxdown-node/__test__/xlsx-test.ts` writes an `.xlsx` in code (a zip kept without compression), which is where the 40,000 cells come from for the test that a page stays free while the worker reads.

### What changed on 2026-11-26

Asked for, and made, not proposed.

#### `@d0paminedriven/mimedown-wasm`: the WebAssembly-only package for `mimedown`

`crates/mimedown-node/wasm/` is new, laid out as `crates/xlsxdown-node/wasm/` is.

- `wasm/package.json` is a new manifest: the xlsx one with the names changed (`@d0paminedriven/mimedown-wasm`, binary `mimedown-node-wasm`), at 0.8.0 to match its base package, with its own description and keywords. `version.sh` now moves four files for `mimedown`, as it does for `rtfdown`, `xlsxdown`, `xlsdown`, and `docdown`.
- `crates/mimedown-node/package.json` gained what the xlsx binding's has: the scripts `build:wasm-package` (with its three parts) and `test:wasm`, `build:wasm-package` at the end of `build:all`, and the dev dependencies `playwright-core`, `rolldown`, `rolldown-plugin-wasm`, and `tsdown`, from the catalogs the other bindings use. `yarn.lock` moved with them (four lines). It did not gain the xlsx binding's `workspaces` key, which the `rtfdown` and `docdown` bindings do without.
- `crates/mimedown-node/.gitignore` is now the same as the xlsx binding's, and `.oxfmtrc.json` gained the five lines that keep the package's generated scripts out of the formatter.
- `crates/mimedown-node/ci/wasm` and `ci/publish-wasm` are the xlsx binding's with the names changed, and `.github/workflows/mimedown-node.yml` has the `wasm` and `publish-wasm` jobs, with `wasm` among what `publish` waits on. The base package's release commit, `mimedown@<version>`, publishes both.
- The worker (`MimeWorker`) answers the build's eleven promise verbs, the four tables among them. `wasm/src/protocol.ts` differs from the xlsx one in one type: the verbs the worker runs are the ones that answer at once and have a promise twin (`WorkerBindingKeys`). When the package was first built the tables had none, which is what the type was written for; they have one now (below), and the type stays as the rule a later verb is held to. The client sends only a view's own bytes: a message carries the whole of the buffer a view looks onto, and the pieces a page cuts from a file it holds are views.
- Its tests build their files in code. `crates/mimedown-node/__test__/mime-test.ts` is new and holds `MimeTest`, moved out of `index.spec.ts` so that the package's tests can use it without loading the native addon: a still and an animated PNG, GIF, and WebP, an icon of several sizes, a TIFF of several pages, and a WAVE file of any length. The page test asks for the shape of half a minute of sound 24 times at once, which is what takes the worker long enough to show that the page stays free.

The package is 7.5 MB packed, where the xlsx one is 1.6 MB: each build is 6.6 MB (2.4 MB gzipped) because the binding turns on all four parsers. Built for the target without threads, for comparison:

| Parsers | Raw | Gzipped |
|---|---|---|
| none (sniffing, and a picture's facts) | 0.89 MB | 0.35 MB |
| PDF and Office | 3.17 MB | 1.24 MB |
| audio and video | 4.51 MB | 1.66 MB |
| all four, as built | 6.57 MB | 2.42 MB |

A smaller entry for a page would take features on `crates/mimedown-node/Cargo.toml` and a second build in the package. Neither is made, and neither is wanted for now: you took the size as it is the same day, for reading a file's facts on the client, to be whittled down later.

#### `@d0paminedriven/mimedown-wasm` needs Node 26

The first `mimedown@0.9.0` commit failed one job, the package's tests on Node 24, with `Fatal process out of memory: Zone`, and so published nothing. It is not memory: it happens at once on a machine with 128 GB, and a heap of 16 GB changes nothing.

- What fails is the engine's optimizing compiler for WebAssembly (V8 13.6, in Node 24.22.3), on a background thread, in one function: `lopdf::encodings::glyphnames::Glyph::from_name`, which is 532 kB of the build, five times the next largest. Told to optimize only that function, Node 24 fails; told to optimize any other, or none, it does not. Node 26 optimizes it.
- It takes reading between 11 and 20 PDFs with `specs` in one process to get there, which the package's tests do.
- `wasm/package.json` now says `"node": ">=26"`, the README says so and why, and the `wasm` job in `.github/workflows/mimedown-node.yml` runs on Node 26 alone.
- The same failure is in what is already published: `@d0paminedriven/pdfdown`'s own WebAssembly build fails the same way on Node 24 after enough PDFs. Nothing tested it, because the `test-wasi` jobs run on Node 26. It matters only where no native addon loads.
- What would bring Node 24 back, and take half a megabyte out of every build that reads PDFs: that lookup as a sorted table and not one function, in `lopdf` or in `pdfdown` in its place. Not started.

#### `crates/mimedown-node`: six promise verbs

`sniffFromRangesAsync` and `specsFromRangesAsync` are new, so that every verb that reads a file has a twin that does not block. The pieces are checked on the JS thread and then copied, as a whole file's bytes are, so what is wrong with them is thrown where they were passed.

The four tables have twins too, which you asked for after the first two: `mediaTypesOfAsync`, `mediaTypeOfPathAsync`, `extensionsOfAsync`, and `builtWithAsync`. They read no file and cannot block; the twins are there so that a caller who has only promises, as a page has of the worker, can ask them without a second copy of the build on its own thread. Every verb the binding has as a function now has a promise twin. No manifest changed for any of this.

#### `mimedown`: what `frames` counts

`frames` is now the frames of an animation and nothing else: an icon's sizes and a TIFF's pages are no longer counted in it, and stay in each format's own detail. This changes what a published package returns for those two formats, so it goes out as the next minor. No manifest changed for it.
