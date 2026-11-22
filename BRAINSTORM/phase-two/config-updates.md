# Config updates

What I need from the manifests and configuration, which are yours, to keep going. Each entry has the file it belongs in above it. They are in the order I ran into them: sections 1 to 3 are done, section 4 is a decision that holds up nothing, section 5 is how releases have to be cut, and section 6 is the pilot of a WebAssembly package of its own.

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

## Nothing needed for

- The rest of `tiffdown`'s codings (Deflate, PackBits, Group 3, JPEG, tiles): `flate2`, `hayro-ccitt`, and `image` are already in its manifest.
- A working resolution for small pictures in `ocrdown`: `image` resizes.
- The change to how `pdfdown` reads a page's text: no manifest is touched. It does need `pdfdown` and `mimedown` released for it to reach the packages.
