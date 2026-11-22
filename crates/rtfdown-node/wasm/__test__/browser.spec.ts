import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { extname, join } from "node:path";
import { after, before, describe, it } from "node:test";

import {
  rtfDocument,
  rtfLayout,
  rtfText
} from "@d0paminedriven/rtfdown-wasm/threadless";
import { chromium } from "playwright-core";

import { RtfTest } from "@/__test__/rtf-test.ts";

/**
 * The worker entry, on a real page.
 *
 * The package's `dist` folder is served as it ships, with no header that
 * isolates the page, and the Chrome on this machine loads a page that starts
 * the worker and asks it for a few documents, by the names the build itself
 * gives its promise verbs. The page writes what it got
 * where this can read it, and the answers are held against what the same
 * build gives in Node.
 */
const rtfTest = new RtfTest();
const held = Buffer.from("%PDF-1.4 the file itself");
const documents = {
  "/invoice.rtf": rtfTest.invoice,
  "/carrying.rtf": rtfTest.document(rtfTest.object("Paint.Picture", held)),
  // Long enough that setting it on pages takes the worker a while.
  "/long.rtf": rtfTest.document(
    Array.from(
      { length: 4000 },
      (_, n) => `Line ${n} of a long document, with words enough to wrap.\\par `
    ).join("")
  )
} as const satisfies Record<string, Buffer>;

/** The same documents by the address the page asks for them at. */
const served = new Map<string, Buffer>(Object.entries(documents));

const types = new Map<string, string>([
  [".js", "text/javascript"],
  [".wasm", "application/wasm"]
]);

/** What the page does, and reports as JSON on its body. */
const scenario = `
import { RtfWorker } from "./client.js";
const out = {};
try {
  const bytes = async (name) => new Uint8Array(await (await fetch(name)).arrayBuffer());
  const worker = new RtfWorker();
  // Asked at once, before the worker has loaded anything.
  const slash = String.fromCharCode(92);
  const early = worker.rtfTextAsync(new TextEncoder().encode("{" + slash + "rtf1" + slash + "ansi Early" + slash + "par}"));
  out.early = await Promise.race([early, new Promise((settle) => setTimeout(() => settle("never answered"), 5000))]);
  out.target = await worker.ready;
  const invoice = await bytes("/invoice.rtf");
  out.text = await worker.rtfTextAsync(invoice);
  const whole = await worker.rtfDocumentAsync(invoice);
  out.pages = whole.pages.length;
  out.invoiceId = whole.invoice.fields.invoiceId ? whole.invoice.fields.invoiceId.value : null;
  const [embedded] = await worker.rtfEmbeddedAsync(await bytes("/carrying.rtf"));
  out.embedded = new TextDecoder().decode(embedded.data);
  let ticks = 0;
  const timer = setInterval(() => { ticks += 1; }, 5);
  const started = performance.now();
  const laid = await worker.rtfLayoutAsync(await bytes("/long.rtf"));
  clearInterval(timer);
  out.longPages = laid.length;
  document.body.dataset.ms = String(Math.round(performance.now() - started));
  document.body.dataset.ticks = String(ticks);
  document.body.dataset.rejected = await worker.rtfTextAsync(new TextEncoder().encode("%PDF-1.7")).then(() => "answered", (error) => error.message);
  worker.terminate();
  document.body.dataset.ended = await worker.rtfTextAsync(invoice).then(() => "answered", (error) => error.message);
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
import { RtfWorker } from "./client.js";
try {
  const worker = new RtfWorker();
  await worker.ready;
  const invoice = new Uint8Array(await (await fetch("/invoice.rtf")).arrayBuffer());
  document.body.dataset.text = await worker.rtfTextAsync(invoice);
} catch (error) {
  document.body.dataset.error = String((error && error.message) || error);
}
document.body.dataset.report = "done";
`;

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
    const document = served.get(path);
    const scripted = pages.get(path);
    if (scripted) {
      send(
        "text/html",
        `<!doctype html><body><script type="module">${scripted}</script></body>`
      );
    } else if (document) {
      send("application/rtf", document);
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
    "should start on a page that is not isolated, and answer as the build does in Node",
    {},
    () => {
      assert.equal(page.get("error"), undefined);
      const whole = rtfDocument(documents["/invoice.rtf"]);
      const answers: unknown = JSON.parse(page.get("report") ?? "{}");
      assert.deepEqual(answers, {
        early: "Early",
        target: "wasm32-wasip1",
        text: rtfText(documents["/invoice.rtf"]),
        pages: whole.pages.length,
        invoiceId: whole.invoice.fields.invoiceId?.value ?? null,
        embedded: held.toString(),
        longPages: rtfLayout(documents["/long.rtf"]).length,
        isolated: false
      });
    }
  );

  it("should leave the page free while it works", {}, () => {
    // A timer on the page is asked to fire every 5 ms while the worker sets the long
    // document on pages. On the page's own thread it would not fire until the end.
    const ms = Number.parseInt(page.get("ms") ?? "0", 10);
    const ticks = Number.parseInt(page.get("ticks") ?? "0", 10);
    assert.ok(ms > 50, `the layout took ${ms} ms, too short to tell`);
    assert.ok(ticks >= ms / 50, `${ticks} ticks in ${ms} ms`);
  });

  it(
    "should reject for what is not RTF, and for anything asked after it is ended",
    {},
    () => {
      assert.match(page.get("rejected") ?? "", /^unsupported format: /);
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
      assert.equal(untyped.get("text"), rtfText(documents["/invoice.rtf"]));
    }
  );
});
