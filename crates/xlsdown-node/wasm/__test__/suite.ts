import assert from "node:assert/strict";
import { describe, it } from "node:test";

import type { Rm } from "@d0paminedriven/type-utils/utils";

import { XlsTest } from "@/__test__/xls-test.ts";

/** What the package exports, as a caller of it sees. */
type Root = typeof import("@d0paminedriven/xlsdown-wasm");

/** Either build: the same verbs, and its own name for itself. */
type Binding = Rm<Root, "__napiBindingTarget"> & {
  readonly __napiBindingTarget: "wasm32-wasi" | "wasm32-wasip1";
};

/**
 * The WebAssembly package: the verbs of the main one, with no native addon
 * behind them and no entry that looks for one.
 *
 * It holds two builds, and this asks the same of both. The one with threads
 * is what a caller in Node gets from the package's name. The one without is
 * what a page or a worker with no shared memory can run, and is the
 * package's `threadless` entry. Each is imported by the name a caller would
 * use, so the package's own map of entries is what finds it.
 */
export function wasmSuite(
  binding: Binding,
  flavor: Binding["__napiBindingTarget"]
) {
  const { XlsDown, spreadsheetCells, spreadsheetCellsAsync } = binding;
  const xlsTest = new XlsTest();
  const notXls = Buffer.from("%PDF-1.7\n");

  describe(`the WebAssembly package (${flavor})`, () => {
    const file = xlsTest.costs;

    it(
      "should be the build it is said to be, and never a native addon",
      {},
      () => {
        assert.notEqual(binding.__napiBindingTarget, "native");
        assert.equal(binding.__napiBindingTarget, flavor);
      }
    );

    it("should read a workbook's cells, typed", {}, () => {
      const sheet = xlsTest.first(spreadsheetCells(file));
      assert.equal(sheet.sheet, "Costs");
      assert.partialDeepStrictEqual(xlsTest.cell(sheet, "A1"), {
        type: "Text",
        value: "Total"
      });
      assert.partialDeepStrictEqual(xlsTest.cell(sheet, "B1"), {
        type: "Date",
        value: "2026-11-23",
        text: "11/23/2026"
      });
      assert.partialDeepStrictEqual(xlsTest.cell(sheet, "B3"), {
        type: "Number",
        value: 2469,
        formula: "B2*2"
      });
    });

    it("should give the same answer from its promise", {}, async () => {
      const all = { hiddenSheets: true };
      assert.deepEqual(
        await spreadsheetCellsAsync(file, all),
        spreadsheetCells(file, all)
      );
    });

    it(
      "should reject from a promise for what is not a workbook",
      {},
      async () => {
        await assert.rejects(
          spreadsheetCellsAsync(notXls),
          /^Error: malformed workbook: /
        );
        assert.throws(
          () => spreadsheetCells(notXls),
          /^Error: malformed workbook: /
        );
        await assert.rejects(
          spreadsheetCellsAsync(xlsTest.locked),
          /^Error: unsupported format: /
        );
      }
    );

    it(
      "should parse once in the class, in its constructor or from a promise",
      {},
      async () => {
        const doc = await XlsDown.openAsync(file);
        assert.ok(doc instanceof XlsDown);
        assert.deepEqual(doc.spreadsheetCells(), spreadsheetCells(file));
        assert.deepEqual(
          await doc.spreadsheetCellsAsync(),
          spreadsheetCells(file)
        );
        const meta = doc.metadata();
        assert.equal(meta.sheetCount, 2);
        assert.equal(meta.formulaCount, 1);
        assert.deepEqual(await doc.metadataAsync(), meta);
        assert.deepEqual(await doc.layoutAsync(), new XlsDown(file).layout());
        assert.deepEqual(await doc.invoiceAsync(), doc.invoice());
      }
    );

    it("should take many calls at once", {}, async () => {
      // More waiting than any pool of workers has hands, and with no pool at all.
      const answers = await Promise.all(
        Array.from({ length: 64 }, () => spreadsheetCellsAsync(file))
      );
      const expected = spreadsheetCells(file);
      for (const answer of answers) assert.deepEqual(answer, expected);
    });
  });
}
