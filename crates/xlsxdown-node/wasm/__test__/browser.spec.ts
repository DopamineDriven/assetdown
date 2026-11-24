import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { extname, join } from "node:path";
import { after, before, describe, it } from "node:test";

import { spreadsheetCells } from "@d0paminedriven/xlsxdown-wasm/threadless";
import { chromium } from "playwright-core";

import { XlsxTest } from "@/__test__/xlsx-test.ts";

/**
 * The worker entry, on a real page.
 *
 * The package's `dist` folder is served as it ships, with no header that
 * isolates the page, and the Chrome on this machine loads a page that starts
 * the worker and asks it for a few workbooks, by the name the build itself
 * gives its promise verb. The page writes what it got where this can read it,
 * and the answers are held against what the same build gives in Node.
 */
const xlsxTest = new XlsxTest();
const workbooks = {
  "/form.xlsx": xlsxTest.getFixtureBuff("form"),
  // With cells enough that reading them takes the worker a while.
  "/long.xlsx": xlsxTest.long
} as const satisfies Record<string, Buffer>;

/** The same workbooks by the address the page asks for them at. */
const served = new Map<string, Buffer>(Object.entries(workbooks));

const types = new Map<string, string>([
  [".js", "text/javascript"],
  [".wasm", "application/wasm"]
]);

/** What the page does, and reports as JSON on its body. */
const scenario = `
import { XlsxWorker } from "./client.js";
const out = {};
try {
  const bytes = async (name) => new Uint8Array(await (await fetch(name)).arrayBuffer());
  const form = await bytes("/form.xlsx");
  const worker = new XlsxWorker();
  // Asked at once, before the worker has loaded anything.
  const early = worker.spreadsheetCellsAsync(form).then((sheets) => sheets.length);
  out.early = await Promise.race([early, new Promise((settle) => setTimeout(() => settle("never answered"), 5000))]);
  out.target = await worker.ready;
  const sheets = await worker.spreadsheetCellsAsync(form, { hiddenSheets: true });
  out.sheets = sheets.map((sheet) => sheet.sheet);
  out.shown = sheets[0].cells.map((cell) => cell.text);
  let ticks = 0;
  const timer = setInterval(() => { ticks += 1; }, 5);
  const started = performance.now();
  const many = await worker.spreadsheetCellsAsync(await bytes("/long.xlsx"));
  clearInterval(timer);
  out.longCells = many[0].cells.length;
  document.body.dataset.ms = String(Math.round(performance.now() - started));
  document.body.dataset.ticks = String(ticks);
  document.body.dataset.rejected = await worker.spreadsheetCellsAsync(new TextEncoder().encode("%PDF-1.7")).then(() => "answered", (error) => error.message);
  worker.terminate();
  document.body.dataset.ended = await worker.spreadsheetCellsAsync(form).then(() => "answered", (error) => error.message);
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
import { XlsxWorker } from "./client.js";
try {
  const worker = new XlsxWorker();
  await worker.ready;
  const form = new Uint8Array(await (await fetch("/form.xlsx")).arrayBuffer());
  const [sheet] = await worker.spreadsheetCellsAsync(form);
  document.body.dataset.sheet = sheet.sheet;
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
    const workbook = served.get(path);
    const scripted = pages.get(path);
    if (scripted) {
      send(
        "text/html",
        `<!doctype html><body><script type="module">${scripted}</script></body>`
      );
    } else if (workbook) {
      send(
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        workbook
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
      const all = spreadsheetCells(workbooks["/form.xlsx"], {
        hiddenSheets: true
      });
      const answers: unknown = JSON.parse(page.get("report") ?? "{}");
      assert.deepEqual(answers, {
        early: 1,
        target: "wasm32-wasip1",
        sheets: all.map((sheet) => sheet.sheet),
        shown: xlsxTest.first(all).cells.map((cell) => cell.text),
        longCells: xlsxTest.first(spreadsheetCells(workbooks["/long.xlsx"]))
          .cells.length,
        isolated: false
      });
    }
  );

  it("should leave the page free while it works", {}, () => {
    // A timer on the page is asked to fire every 5 ms while the worker reads the long
    // workbook. On the page's own thread it would not fire until the end.
    const ms = Number.parseInt(page.get("ms") ?? "0", 10);
    const ticks = Number.parseInt(page.get("ticks") ?? "0", 10);
    assert.ok(ms > 50, `the reading took ${ms} ms, too short to tell`);
    assert.ok(ticks >= ms / 50, `${ticks} ticks in ${ms} ms`);
  });

  it(
    "should reject for what is not a workbook, and for anything asked after it is ended",
    {},
    () => {
      assert.match(page.get("rejected") ?? "", /^malformed workbook: /);
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
      assert.equal(untyped.get("sheet"), "Form");
    }
  );
});
