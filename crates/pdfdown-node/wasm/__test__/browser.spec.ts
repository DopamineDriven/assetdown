import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { extname, join } from "node:path";
import { after, before, describe, it } from "node:test";

import {
  capabilities,
  extractImagesPerPage,
  extractInvoice,
  extractTextPerPage,
  layoutPerPage,
  pdfMetadata
} from "@d0paminedriven/pdfdown-wasm/threadless";
import { chromium } from "playwright-core";

import { WorkupService } from "@/__test__/workup.ts";

/**
 * The worker entry, on a real page.
 *
 * The package's `dist` folder is served as it ships, with no header that
 * isolates the page, and the Chrome on this machine loads a page that starts
 * the worker and asks it about a few PDFs, by the names the build itself
 * gives its promise verbs. The page writes what it got where this can read it,
 * and the answers are held against what the same build gives in Node.
 */
const fixtures = new WorkupService();
const documents = {
  /** One page with an invoice's fields on it. */
  "/invoice.pdf": fixtures.getFixtureBuff(
    "invoices/wordpress-pdf-invoice-plugin-sample"
  ),
  /** One page that is a JPEG scan. */
  "/scan.pdf": fixtures.getFixtureBuff("ascii85-dct-scan"),
  /** The longest of the fixtures to read: four pages of invoices. */
  "/long.pdf": fixtures.getFixtureBuff(
    "invoices/Downloadable-PDF-Invoices-Add-On-Samples"
  )
} as const satisfies Record<string, Buffer>;

/** The same documents by the address the page asks for them at. */
const served = new Map<string, Buffer>(Object.entries(documents));

const types = new Map<string, string>([
  [".js", "text/javascript"],
  [".wasm", "application/wasm"]
]);

/** How many times the page asks for the long document's pages at once. */
const AT_ONCE = 64;

/** What the page does, and reports as JSON on its body. */
const scenario = `
import { PdfWorker } from "./client.js";
const out = {};
try {
  const bytes = async (name) => new Uint8Array(await (await fetch(name)).arrayBuffer());
  const invoice = await bytes("/invoice.pdf");
  const worker = new PdfWorker();
  // Asked at once, before the worker has loaded anything.
  const early = worker.pdfMetadataAsync(invoice).then((meta) => meta.version);
  out.early = await Promise.race([early, new Promise((settle) => setTimeout(() => settle("never answered"), 5000))]);
  out.target = await worker.ready;
  out.capabilities = await worker.capabilitiesAsync();
  out.metadata = await worker.pdfMetadataAsync(invoice);
  out.text = await worker.extractTextPerPageAsync(invoice);
  out.invoiceId = (await worker.extractInvoiceAsync(invoice)).fields.invoiceId.value;
  // Pages a page already has go back to the worker for their invoice fields.
  const pages = await worker.layoutPerPageAsync(invoice);
  out.fromPages = (await worker.invoiceFromLayoutAsync(pages)).fields.invoiceId.value;
  // Pictures come back with their bytes, which a page has as a Uint8Array.
  const images = await worker.extractImagesPerPageAsync(await bytes("/scan.pdf"));
  out.images = images.map((image) => [image.page, image.width, image.height, image.data.length, image.data instanceof Uint8Array]);
  const long = await bytes("/long.pdf");
  let ticks = 0;
  const timer = setInterval(() => { ticks += 1; }, 5);
  const started = performance.now();
  const layouts = await Promise.all(Array.from({ length: ${AT_ONCE} }, () => worker.layoutPerPageAsync(long)));
  clearInterval(timer);
  out.pages = layouts.map((pages) => pages.length);
  document.body.dataset.ms = String(Math.round(performance.now() - started));
  document.body.dataset.ticks = String(ticks);
  document.body.dataset.rejected = await worker.extractTextPerPageAsync(new TextEncoder().encode("not a pdf at all")).then(() => "answered", (error) => error.message);
  worker.terminate();
  document.body.dataset.ended = await worker.pdfMetadataAsync(invoice).then(() => "answered", (error) => error.message);
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
import { PdfWorker } from "./client.js";
try {
  const worker = new PdfWorker();
  await worker.ready;
  const invoice = new Uint8Array(await (await fetch("/invoice.pdf")).arrayBuffer());
  document.body.dataset.version = (await worker.pdfMetadataAsync(invoice)).version;
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
      send("application/pdf", file);
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
      assert.equal(got.get("early"), "1.3");
      assert.equal(got.get("target"), "wasm32-wasip1");
      assert.equal(got.get("isolated"), false);
    }
  );

  it("should answer as the build does in Node", {}, () => {
    const got = answers();
    const invoice = documents["/invoice.pdf"];
    assert.deepEqual(got.get("capabilities"), capabilities());
    assert.deepEqual(got.get("metadata"), reported(pdfMetadata(invoice)));
    assert.deepEqual(got.get("text"), reported(extractTextPerPage(invoice)));
    const id = extractInvoice(invoice).fields.invoiceId?.value;
    assert.equal(id, "INV-3337");
    assert.equal(got.get("invoiceId"), id);
    assert.equal(got.get("fromPages"), id);
  });

  it("should give a picture's bytes as a page has them", {}, () => {
    // The runtime under the build asks for a `Buffer` to give bytes back in, and a page has
    // none: the worker stands one in, and what crosses to the page is a `Uint8Array`.
    const images = extractImagesPerPage(documents["/scan.pdf"]);
    assert.equal(images.length, 1);
    assert.deepEqual(
      answers().get("images"),
      images.map((image) => [
        image.page,
        image.width,
        image.height,
        image.data.length,
        true
      ])
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
    const pages = layoutPerPage(documents["/long.pdf"]).length;
    assert.deepEqual(
      answers().get("pages"),
      Array.from({ length: AT_ONCE }, () => pages)
    );
  });

  it(
    "should reject for what is not a PDF, and for anything asked after it is ended",
    {},
    () => {
      assert.match(page.get("rejected") ?? "", /^Failed to load PDF: /);
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
      assert.equal(untyped.get("version"), "1.3");
    }
  );
});
