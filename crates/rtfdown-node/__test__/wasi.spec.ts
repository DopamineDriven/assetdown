import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { RtfTest } from "@/__test__/rtf-test.ts";
import {
  RtfDown,
  __napiBindingTarget,
  rtfDocument,
  rtfDocumentAsync,
  rtfInvoice,
  rtfInvoiceAsync,
  rtfLayout,
  rtfLayoutAsync,
  rtfText,
  rtfTextAsync
} from "@/index.js";

/**
 * The WebAssembly build.
 *
 * `yarn test:wasi` sets `NAPI_RS_FORCE_WASI=error`, which makes the generated
 * loader require the wasm binding and throw when there is none. Under plain
 * `yarn test` the native addon loads and this suite skips itself.
 *
 * Async work is the part that differs most from native: on wasm it runs on
 * worker threads over shared memory, so the promise forms get the attention.
 */
const rtfTest = new RtfTest();
const skip =
  __napiBindingTarget === "native"
    ? "the native addon is loaded; run yarn test:wasi"
    : false;

describe("wasm32-wasi build", { skip }, () => {
  const file = rtfTest.invoice;

  it("should load the wasm32-wasi binding and not the native addon", {}, () => {
    assert.equal(__napiBindingTarget, "wasm32-wasi");
  });

  it("should read a document's text and set it on pages", {}, () => {
    assert.equal(rtfText(file).split("\n")[1], "Invoice Number: INV-1001");
    const [page] = rtfLayout(file);
    assert.ok(page);
    assert.deepEqual([page.width, page.height], [612, 792]);
    assert.equal(page.lines[0]?.text, "INVOICE");
  });

  it("should read the invoice's fields", {}, () => {
    const invoice = rtfInvoice(file);
    assert.equal(invoice.fields.invoiceId?.value, "INV-1001");
    assert.equal(invoice.fields.invoiceTotal?.value, 1234.56);
  });

  it("should give the same answers from its promises", {}, async () => {
    const [text, layout, invoice, document] = await Promise.all([
      rtfTextAsync(file),
      rtfLayoutAsync(file),
      rtfInvoiceAsync(file),
      rtfDocumentAsync(file)
    ]);
    assert.equal(text, rtfText(file));
    assert.deepEqual(layout, rtfLayout(file));
    assert.deepEqual(invoice, rtfInvoice(file));
    assert.deepEqual(document, rtfDocument(file));
  });

  it("should reject from a promise for what is not RTF", {}, async () => {
    await assert.rejects(
      rtfTextAsync(Buffer.from("%PDF-1.7\n")),
      /^Error: unsupported format: /
    );
  });

  it(
    "should parse once in the class, in its constructor or from a promise",
    {},
    async () => {
      const rtf = await RtfDown.openAsync(file);
      assert.ok(rtf instanceof RtfDown);
      assert.deepEqual(rtf.document(), new RtfDown(file).document());
      assert.deepEqual(await rtf.documentAsync(), rtfDocument(file));
    }
  );
});
