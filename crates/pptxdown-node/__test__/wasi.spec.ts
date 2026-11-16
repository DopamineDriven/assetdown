import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { WorkupService } from "@/__test__/workup.ts";
import {
  PptxDown,
  __napiBindingTarget,
  pptxDocumentAsync,
  pptxMarkdown,
  pptxMarkdownAsync,
  pptxMetadata,
  pptxSlidesAsync,
  pptxTextAsync
} from "@/index.js";
import type { PptxSlide } from "@/index.js";

/**
 * The WebAssembly build.
 *
 * `yarn test:wasi` sets `NAPI_RS_FORCE_WASI=error`, which makes the generated
 * loader require the wasm binding and throw when there is none. Under plain
 * `yarn test` the native addon loads and this suite skips itself.
 *
 * Async work is the part that differs most from native: on wasm it runs on
 * worker threads over shared memory, so the async readers get the attention.
 */
const t = new WorkupService();
const skip =
  __napiBindingTarget === "native"
    ? "the native addon is loaded; run yarn test:wasi"
    : false;

describe("wasm32-wasi build", { skip }, () => {
  /** Thirteen slides, 16:9, with five pictures. */
  const wide = t.getFixtureBuff("HMEM-Horizon-Mediated-Episodic-Memory.pptx");
  /** Four slides, 4:3, with nine pictures and numbered steps. */
  const older = t.getFixtureBuff("Activity-7.pptx");
  const notDeck = Buffer.from("not a zip at all");

  it("should load the wasm32-wasi binding and not the native addon", {}, () => {
    assert.equal(__napiBindingTarget, "wasm32-wasi");
  });
  it("should return a PptxDown that reads a deck", {}, () => {
    const deck = new PptxDown(wide);
    assert.equal(deck instanceof PptxDown, true);
    assert.deepEqual(deck.text().split("\n").slice(0, 2), [
      "Horizon-Mediated Episodic Memory",
      "CONVERSATION MEMORY SUBSYSTEM"
    ]);
  });
  it(
    "should return the same counts and slide size as the native build",
    {},
    () => {
      assert.partialDeepStrictEqual(pptxMetadata(wide), {
        title: "HMEM: Horizon-Mediated Episodic Memory",
        language: "en-US",
        slideCount: 13,
        slidesRecorded: 13,
        imageCount: 5,
        tableCount: 0,
        linkCount: 0,
        width: 960,
        height: 540,
        aspectRatio: "16:9",
        paperSize: { type: "UNKNOWN" },
        layouts: ["Title and body"],
        warnings: []
      });
      assert.partialDeepStrictEqual(pptxMetadata(older), {
        slideCount: 4,
        imageCount: 9,
        width: 720,
        height: 540,
        aspectRatio: "4:3",
        sizeKind: "screen4x3"
      });
    }
  );
  it("should write the same Markdown and set the same lines", {}, () => {
    const deck = new PptxDown(wide);
    assert.deepEqual(pptxMarkdown(wide).split("\n").slice(0, 3), [
      "## Horizon-Mediated Episodic Memory",
      "",
      "**CONVERSATION MEMORY SUBSYSTEM**"
    ]);
    assert.equal(
      pptxMarkdown(older)
        .split("\n")
        .includes(
          "4. Using the chart given at the end of this activity to determine the point group."
        ),
      true
    );
    const [page] = deck.layout();
    assert.ok(page);
    assert.partialDeepStrictEqual(page, { page: 1, width: 960, height: 540 });
    assert.equal(page.lines[0]?.text, "CONVERSATION MEMORY SUBSYSTEM");
    assert.partialDeepStrictEqual(page.words[0], {
      x0: 64,
      y1: 476,
      fontSize: 12
    });
    assert.equal(deck.invoice().fields.invoiceId, undefined);
  });
  it("should return each picture with its bytes", {}, () => {
    const images = new PptxDown(older).images();
    assert.equal(images.length, 9);
    assert.equal(
      images.every(
        (image) => image.data.length > 0 && image.mime !== undefined
      ),
      true
    );
  });
  it(
    "should resolve each async form to what its sync twin returns",
    {},
    async () => {
      const deck = new PptxDown(wide);
      const opened = await PptxDown.openAsync(wide);
      assert.deepEqual(opened.metadata(), deck.metadata());
      assert.deepEqual(await deck.metadataAsync(), deck.metadata());
      assert.equal(await deck.textAsync(), deck.text());
      assert.deepEqual(await deck.textPerPageAsync(), deck.textPerPage());
      assert.deepEqual(await deck.structuredTextAsync(), deck.structuredText());
      assert.deepEqual(await deck.slidesAsync(), deck.slides());
      assert.deepEqual(await deck.tablesAsync(), deck.tables());
      assert.deepEqual(await deck.imagesAsync(), deck.images());
      assert.deepEqual(await deck.annotationsAsync(), deck.annotations());
      assert.deepEqual(await deck.notesAsync(), deck.notes());
      assert.deepEqual(await deck.chartsAsync(), deck.charts());
      assert.deepEqual(await deck.mediaAsync(), deck.media());
      assert.deepEqual(await deck.layoutAsync(), deck.layout());
      assert.deepEqual(await deck.invoiceAsync(), deck.invoice());
      assert.equal(await pptxMarkdownAsync(wide), deck.markdown());
      const activity = new PptxDown(older);
      assert.deepEqual(await activity.imagesAsync(), activity.images());
      assert.deepEqual(await pptxDocumentAsync(older), activity.document());
    }
  );
  it(
    "should return the same slides to 32 readers running at once",
    {},
    async () => {
      const deck = new PptxDown(wide);
      const expected = deck.slides();
      const readers = Array.of<Promise<PptxSlide[]>>();
      for (let i = 0; i < 16; i++) {
        readers.push(deck.slidesAsync());
        readers.push(pptxSlidesAsync(wide));
      }
      for (const result of await Promise.all(readers)) {
        assert.deepEqual(result, expected);
      }
    }
  );
  it(
    "should throw from the sync forms and reject from the async ones for bytes that are not a deck",
    {},
    async () => {
      const message = /^malformed presentation: /;
      assert.throws(() => new PptxDown(notDeck), { message });
      assert.throws(() => pptxMetadata(notDeck), { message });
      await assert.rejects(PptxDown.openAsync(notDeck), { message });
      await assert.rejects(pptxTextAsync(notDeck), { message });
    }
  );
});
