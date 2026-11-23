import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { DocTest } from "@/__test__/doc-test.ts";
import {
  DocDown,
  __napiBindingTarget,
  docDocument,
  docDocumentAsync,
  docText,
  docTextAsync
} from "@/index.js";
import type { DocDocument } from "@/index.js";

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
const t = new DocTest();
const skip =
  __napiBindingTarget === "native"
    ? "the native addon is loaded; run yarn test:wasi"
    : false;

describe("wasm32-wasi build", { skip }, () => {
  const carrying = t.carrying;
  it("should load the wasm binding, not the native addon", {}, () => {
    assert.notEqual(__napiBindingTarget, "native");
  });
  it(
    "should read a document's text, on this thread and on a worker",
    {},
    async () => {
      const text = docText(carrying);
      assert.equal(text.split("\n").length, 4);
      assert.equal(await docTextAsync(carrying), text);
    }
  );
  it(
    "should read the same document on a worker as on this thread, many at once",
    {},
    async () => {
      const doc = new DocDown(carrying);
      const expected = doc.document();
      const readers = Array.of<Promise<DocDocument>>();
      for (let i = 0; i < 16; i++) {
        readers.push(doc.documentAsync());
        readers.push(docDocumentAsync(carrying));
      }
      for (const result of await Promise.all(readers)) {
        assert.deepEqual(result, expected);
      }
      assert.deepEqual(docDocument(carrying), expected);
    }
  );
  it(
    "should throw from the constructor and reject from openAsync for a locked document",
    {},
    async () => {
      const message = "unsupported format: an encrypted document";
      assert.throws(() => new DocDown(t.locked), { message });
      await assert.rejects(DocDown.openAsync(t.locked), { message });
    }
  );
});
