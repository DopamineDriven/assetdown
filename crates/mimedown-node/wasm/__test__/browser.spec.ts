import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { extname, join } from "node:path";
import { after, before, describe, it } from "node:test";

import {
  builtWith,
  extensionsOf,
  mediaTypesOf,
  sniffFromRanges,
  specs
} from "@d0paminedriven/mimedown-wasm/threadless";
import { chromium } from "playwright-core";

import { MimeTest } from "@/__test__/mime-test.ts";

/**
 * The worker entry, on a real page.
 *
 * The package's `dist` folder is served as it ships, with no header that
 * isolates the page, and the Chrome on this machine loads a page that starts
 * the worker and asks it about a few files, by the names the build itself
 * gives its promise verbs. The page writes what it got where this can read it,
 * and the answers are held against what the same build gives in Node.
 */
const mimeTest = new MimeTest();
const [name] = mimeTest.illustrator;
assert.ok(name !== undefined);

/** A still picture and one that moves of each family, then a set of sizes, and pages. */
const pictures = {
  "/still.png": mimeTest.pngOf(),
  "/moving.png": mimeTest.pngOf(3),
  "/still.gif": mimeTest.gifOf(1),
  "/moving.gif": mimeTest.gifOf(3),
  "/still.webp": mimeTest.webpOf(),
  "/moving.webp": mimeTest.webpOf(3),
  "/sizes.ico": mimeTest.icoOf([16, 32, 256]),
  "/pages.tiff": mimeTest.tiffOf(3)
} as const satisfies Record<string, Buffer>;

const files = {
  ...pictures,
  /** An Illustrator document, which is a PDF under another name. */
  "/document.ai": mimeTest.getFixtureBuff(name),
  // Half a minute of sound: enough that reading its shape takes the worker a while.
  "/long.wav": mimeTest.wavOf(30, 48_000, 2)
} as const satisfies Record<string, Buffer>;

/** The same files by the address the page asks for them at. */
const served = new Map<string, Buffer>(Object.entries(files));

const types = new Map<string, string>([
  [".js", "text/javascript"],
  [".wasm", "application/wasm"]
]);

/** How many times the page asks for the long sound's shape at once. */
const AT_ONCE = 24;

/** What the page does, and reports as JSON on its body. */
const scenario = `
import { MimeWorker } from "./client.js";
const out = {};
try {
  const bytes = async (name) => new Uint8Array(await (await fetch(name)).arrayBuffer());
  const still = await bytes("/still.png");
  const worker = new MimeWorker();
  // Asked at once, before the worker has loaded anything.
  const early = worker.sniffAsync(still).then((found) => found.mime);
  out.early = await Promise.race([early, new Promise((settle) => setTimeout(() => settle("never answered"), 5000))]);
  out.target = await worker.ready;
  // The tables, which read no file: asked of the worker, so that the page needs no build of its own.
  out.tables = {
    types: await worker.mediaTypesOfAsync(".PDF"),
    none: await worker.mediaTypesOfAsync("no-such-extension"),
    extensions: await worker.extensionsOfAsync("image/jpeg"),
    ofPath: await worker.mediaTypeOfPathAsync("a/b/report.final.pdf"),
    ofNoPath: await worker.mediaTypeOfPathAsync("a/b/no-extension"),
    builtWith: await worker.builtWithAsync()
  };
  out.pictures = {};
  for (const name of ${JSON.stringify(Object.keys(pictures))}) {
    out.pictures[name] = await worker.specsAsync(await bytes(name));
  }
  // A document from its head, and then from what each answer asks for. The pieces are
  // views of a file held whole, as a page that has the file would cut them.
  const file = await bytes("/document.ai");
  const pieces = [{ offset: 0, bytes: file.subarray(0, 1024) }];
  out.named = await worker.sniffFromRangesAsync(pieces, file.byteLength);
  let answer = await worker.specsFromRangesAsync(pieces, file.byteLength);
  let fetches = 1;
  while (answer.needs !== null && fetches < 8) {
    const { offset, length } = answer.needs;
    pieces.push({ offset, bytes: file.subarray(offset, offset + length) });
    answer = await worker.specsFromRangesAsync(pieces, file.byteLength);
    fetches += 1;
  }
  out.fetches = fetches;
  out.fromRanges = answer;
  out.whole = await worker.specsAsync(file);
  out.stillWhole = file.byteLength;
  const sound = await bytes("/long.wav");
  let ticks = 0;
  const timer = setInterval(() => { ticks += 1; }, 5);
  const started = performance.now();
  const shapes = await Promise.all(Array.from({ length: ${AT_ONCE} }, () => worker.waveformAsync(sound, { peakCount: 100 })));
  clearInterval(timer);
  out.shapes = shapes.map((shape) => shape.type === "made" ? shape.waveform.frameCount : shape.type);
  out.peaks = shapes[0].type === "made" && shapes[0].waveform.channels[0].peak instanceof Float32Array;
  document.body.dataset.ms = String(Math.round(performance.now() - started));
  document.body.dataset.ticks = String(ticks);
  document.body.dataset.rejected = await worker.specsFromRangesAsync([{ offset: -1, bytes: still }]).then(() => "answered", (error) => error.message);
  worker.terminate();
  document.body.dataset.ended = await worker.sniffAsync(still).then(() => "answered", (error) => error.message);
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
import { MimeWorker } from "./client.js";
try {
  const worker = new MimeWorker();
  await worker.ready;
  const moving = new Uint8Array(await (await fetch("/moving.png")).arrayBuffer());
  document.body.dataset.mime = (await worker.sniffAsync(moving)).mime;
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
      // The page is told nothing of what a file is: that is the worker's to say.
      send("application/octet-stream", file);
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
      assert.equal(got.get("early"), "image/png");
      assert.equal(got.get("target"), "wasm32-wasip1");
      assert.equal(got.get("isolated"), false);
    }
  );

  it("should answer from the tables, as the build does in Node", {}, () => {
    assert.deepEqual(answers().get("tables"), {
      types: mediaTypesOf(".PDF"),
      none: [],
      extensions: extensionsOf("image/jpeg"),
      ofPath: "application/pdf",
      ofNoPath: null,
      builtWith: builtWith()
    });
  });

  it(
    "should tell a still picture from one that moves, as the build does in Node",
    {},
    () => {
      const expected = Object.fromEntries(
        Object.entries(pictures).map(([path, file]) => [path, specs(file)])
      );
      assert.deepEqual(answers().get("pictures"), reported(expected));
      // And what those answers are, said once here in full.
      const moves = Object.entries(expected).map(([path, found]) => {
        assert.ok(found.facts.type === "image", path);
        const { frames, animated } = found.facts.image;
        return [path, found.mime, frames, animated];
      });
      assert.deepEqual(moves, [
        ["/still.png", "image/png", 1, false],
        ["/moving.png", "image/apng", 3, true],
        ["/still.gif", "image/gif", 1, false],
        ["/moving.gif", "image/gif", 3, true],
        ["/still.webp", "image/webp", 1, false],
        ["/moving.webp", "image/webp", 3, true],
        ["/sizes.ico", "image/x-icon", 1, false],
        ["/pages.tiff", "image/tiff", 1, false]
      ]);
    }
  );

  it(
    "should give a whole file's answer from its head and what that asks for",
    {},
    () => {
      const got = answers();
      const file = files["/document.ai"];
      const whole = reported(specs(file));
      assert.deepEqual(got.get("whole"), whole);
      assert.deepEqual(got.get("fromRanges"), whole);
      assert.equal(got.get("fetches"), 2);
      assert.deepEqual(
        got.get("named"),
        reported(
          sniffFromRanges(
            [{ offset: 0, bytes: file.subarray(0, 1024) }],
            file.byteLength
          )
        )
      );
      // The page cut its pieces from the file it held, and still holds the file.
      assert.equal(got.get("stillWhole"), file.byteLength);
    }
  );

  it("should leave the page free while it works", {}, (test) => {
    // A timer on the page is asked to fire every 5 ms while the worker reads the shape of
    // the long sound, many times over. On the page's own thread it would not fire until
    // the end.
    const ms = Number.parseInt(page.get("ms") ?? "0", 10);
    const ticks = Number.parseInt(page.get("ticks") ?? "0", 10);
    test.diagnostic(`the page's timer fired ${ticks} times in ${ms} ms`);
    assert.ok(ms > 50, `the reading took ${ms} ms, too short to tell`);
    assert.ok(ticks >= ms / 50, `${ticks} ticks in ${ms} ms`);
    // Each asking was answered, with the shape of all of the sound, as typed arrays.
    const got = answers();
    assert.deepEqual(
      got.get("shapes"),
      Array.from({ length: AT_ONCE }, () => 30 * 48_000)
    );
    assert.equal(got.get("peaks"), true);
  });

  it(
    "should reject for what a caller got wrong, and for anything asked after it is ended",
    {},
    () => {
      assert.match(
        page.get("rejected") ?? "",
        /^offset must be a whole number/
      );
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
      assert.equal(untyped.get("mime"), "image/apng");
    }
  );
});
