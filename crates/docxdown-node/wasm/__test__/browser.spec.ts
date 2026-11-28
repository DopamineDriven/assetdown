import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { extname, join } from "node:path";
import { after, before, describe, it } from "node:test";

import {
  docxImages,
  docxInvoice,
  docxLayout,
  docxMarkdown,
  docxMetadata,
  docxText
} from "@d0paminedriven/docxdown-wasm/threadless";
import { chromium } from "playwright-core";

import { WorkupService } from "@/__test__/workup.ts";

/**
 * The worker entry, on a real page.
 *
 * The package's `dist` folder is served as it ships, with no header that
 * isolates the page, and the Chrome on this machine loads a page that starts
 * the worker and asks it about a few documents, by the names the build itself
 * gives its promise verbs. The page writes what it got where this can read it,
 * and the answers are held against what the same build gives in Node.
 */
const fixtures = new WorkupService();
const documents = {
  /** A short invoice, with a table, a link, and a footnote. */
  "/invoice.docx": fixtures.getFixtureBuff("lo-pandoc.docx"),
  /** A paper of five pages with seven pictures in it. */
  "/pictures.docx": fixtures.getFixtureBuff("Muscles-of-mastication.docx"),
  /** The longest of the fixtures to read: eight pages. */
  "/long.docx": fixtures.getFixtureBuff("Harvati-2003.docx")
} as const satisfies Record<string, Buffer>;

/** The same documents by the address the page asks for them at. */
const served = new Map<string, Buffer>(Object.entries(documents));

const types = new Map<string, string>([
  [".js", "text/javascript"],
  [".wasm", "application/wasm"]
]);

/** How many times the page asks for the long document's pages at once. */
const AT_ONCE = 80;

/** What the page does, and reports as JSON on its body. */
const scenario = `
import { DocxWorker } from "./client.js";
const out = {};
try {
  const bytes = async (name) => new Uint8Array(await (await fetch(name)).arrayBuffer());
  const invoice = await bytes("/invoice.docx");
  const worker = new DocxWorker();
  // Asked at once, before the worker has loaded anything.
  const early = worker.docxTextAsync(invoice).then((text) => text.split("\\n").at(-1));
  out.early = await Promise.race([early, new Promise((settle) => setTimeout(() => settle("never answered"), 5000))]);
  out.target = await worker.ready;
  out.text = await worker.docxTextAsync(invoice);
  out.markdown = await worker.docxMarkdownAsync(invoice);
  out.metadata = await worker.docxMetadataAsync(invoice);
  out.invoiceId = (await worker.docxInvoiceAsync(invoice)).fields.invoiceId.value;
  out.lines = (await worker.docxLayoutAsync(invoice))[0].lines.map((line) => line.text);
  // Pictures come back with their bytes, which a page has as a Uint8Array.
  const images = await worker.docxImagesAsync(await bytes("/pictures.docx"));
  out.images = images.map((image) => [image.mime, image.data.length, image.data instanceof Uint8Array]);
  const long = await bytes("/long.docx");
  let ticks = 0;
  const timer = setInterval(() => { ticks += 1; }, 5);
  const started = performance.now();
  const layouts = await Promise.all(Array.from({ length: ${AT_ONCE} }, () => worker.docxLayoutAsync(long)));
  clearInterval(timer);
  out.pages = layouts.map((pages) => pages.length);
  document.body.dataset.ms = String(Math.round(performance.now() - started));
  document.body.dataset.ticks = String(ticks);
  document.body.dataset.rejected = await worker.docxTextAsync(new TextEncoder().encode("not a zip at all")).then(() => "answered", (error) => error.message);
  worker.terminate();
  document.body.dataset.ended = await worker.docxTextAsync(invoice).then(() => "answered", (error) => error.message);
  out.isolated = globalThis.crossOriginIsolated;
} catch (error) {
  document.body.dataset.error = String((error && error.stack) || error);
}
document.body.dataset.report = JSON.stringify(out);
`;

/**
 * A second page, served by a server that does not call a `.wasm` file what it
 * is, as a plain file server or a store of files often does not.
 */
const untypedScenario = `
import { DocxWorker } from "./client.js";
try {
  const worker = new DocxWorker();
  await worker.ready;
  const invoice = new Uint8Array(await (await fetch("/invoice.docx")).arrayBuffer());
  document.body.dataset.title = (await worker.docxMetadataAsync(invoice)).title;
} catch (error) {
  document.body.dataset.error = String((error && error.message) || error);
}
document.body.dataset.report = "done";
`;

/** A value as a page reports it: through JSON, which is how it is read back here. */
function reported(value: unknown): unknown {
  return JSON.parse(JSON.stringify(value));
}

describe("the worker entry on a page", () => {
  const pages = new Map<string, string>([
    ["/", scenario],
    ["/untyped", untypedScenario]
  ]);
  /** Whether the server says what kind of file it sends. It stops for the second page. */
  let typed = true;
  const server = createServer((request, response) => {
    const path = request.url ?? "/";
    const send = (type: string, body: Buffer | string) => {
      response.writeHead(200, { "content-type": type });
      response.end(body);
    };
    const file = served.get(path);
    const scripted = pages.get(path);
    if (scripted) {
      send(
        "text/html",
        `<!doctype html><body><script type="module">${scripted}</script></body>`
      );
    } else if (file) {
      send(
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        file
      );
    } else {
      readFile(join("wasm/dist", path)).then(
        (body) =>
          send(
            // Only the `.wasm` goes untyped: a page will not run a script sent as anything else.
            (extname(path) === ".wasm" && !typed
              ? undefined
              : types.get(extname(path))) ?? "application/octet-stream",
            body
          ),
        () => {
          response.writeHead(404);
          response.end();
        }
      );
    }
  });
  /** What the page wrote on its body: the answers as JSON, and the rest a string each. */
  const page = new Map<string, string>();
  /** The same, of the page served with no type on its `.wasm` file. */
  const untyped = new Map<string, string>();
  /** The answers, read back. */
  const answers = () => {
    const read: unknown = JSON.parse(page.get("report") ?? "{}");
    assert.ok(typeof read === "object" && read !== null);
    return new Map<string, unknown>(Object.entries(read));
  };

  before(async () => {
    await new Promise<void>((listening) =>
      server.listen(0, "127.0.0.1", listening)
    );
    const address = server.address();
    assert.ok(typeof address === "object" && address !== null);
    const browser = await chromium.launch({
      channel: "chrome",
      args: ["--no-sandbox"]
    });
    try {
      const read = async (path: string, into: Map<string, string>) => {
        const tab = await browser.newPage();
        await tab.goto(`http://127.0.0.1:${address.port}${path}`);
        await tab.waitForFunction(
          () => document.body.dataset.report !== undefined,
          undefined,
          { timeout: 60_000 }
        );
        const written = await tab.evaluate(() =>
          Object.entries(document.body.dataset)
        );
        for (const [name, value] of written) into.set(name, value ?? "");
      };
      await read("/", page);
      typed = false;
      await read("/untyped", untyped);
    } finally {
      await browser.close();
    }
  });
  after(() => server.close());

  it(
    "should start on a page that is not isolated, and answer a call made before it had",
    {},
    () => {
      assert.equal(page.get("error"), undefined);
      const got = answers();
      assert.equal(got.get("early"), "Total due: 25.50 USD");
      assert.equal(got.get("target"), "wasm32-wasip1");
      assert.equal(got.get("isolated"), false);
    }
  );

  it("should answer as the build does in Node", {}, () => {
    const got = answers();
    const invoice = documents["/invoice.docx"];
    assert.equal(got.get("text"), docxText(invoice));
    assert.equal(got.get("markdown"), docxMarkdown(invoice));
    assert.deepEqual(got.get("metadata"), reported(docxMetadata(invoice)));
    assert.equal(
      got.get("invoiceId"),
      docxInvoice(invoice).fields.invoiceId?.value
    );
    const [first] = docxLayout(invoice);
    assert.ok(first);
    assert.deepEqual(
      got.get("lines"),
      first.lines.map((line) => line.text)
    );
  });

  it("should give a picture's bytes as a page has them", {}, () => {
    // The runtime under the build asks for a `Buffer` to give bytes back in, and a page has
    // none: the worker stands one in, and what crosses to the page is a `Uint8Array`.
    const images = docxImages(documents["/pictures.docx"]);
    assert.equal(images.length, 7);
    assert.deepEqual(
      answers().get("images"),
      images.map((image) => [image.mime, image.data.length, true])
    );
  });

  it("should leave the page free while it works", {}, (test) => {
    // A timer on the page is asked to fire every 5 ms while the worker sets the long
    // document on pages, many times over. On the page's own thread it would not fire until
    // the end.
    const ms = Number.parseInt(page.get("ms") ?? "0", 10);
    const ticks = Number.parseInt(page.get("ticks") ?? "0", 10);
    test.diagnostic(`the page's timer fired ${ticks} times in ${ms} ms`);
    assert.ok(ms > 50, `the reading took ${ms} ms, too short to tell`);
    assert.ok(ticks >= ms / 50, `${ticks} ticks in ${ms} ms`);
    const pages = docxLayout(documents["/long.docx"]).length;
    assert.deepEqual(
      answers().get("pages"),
      Array.from({ length: AT_ONCE }, () => pages)
    );
  });

  it(
    "should reject for what is not a Word document, and for anything asked after it is ended",
    {},
    () => {
      assert.match(page.get("rejected") ?? "", /^malformed document: /);
      assert.equal(page.get("ended"), "the worker was ended");
    }
  );

  it(
    "should start from a server that does not say its file is WebAssembly",
    {},
    () => {
      // A browser will only compile a `.wasm` as it arrives when the server calls it
      // `application/wasm`. From any other the worker takes the whole file first.
      assert.equal(untyped.get("error"), undefined);
      assert.equal(untyped.get("title"), "Invoice 4711");
    }
  );
});
