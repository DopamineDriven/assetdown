import assert from "node:assert/strict";
import { describe, it } from "node:test";

import type { Rm } from "@d0paminedriven/type-utils/utils";

import { XlsxTest } from "@/__test__/xlsx-test.ts";

/** What the package exports, as a caller of it sees. */
type Root = typeof import("@d0paminedriven/xlsxdown-wasm");

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
  const { XlsxDown, spreadsheetCells, spreadsheetCellsAsync } = binding;
  const xlsxTest = new XlsxTest();
  const notXlsx = Buffer.from("%PDF-1.7\n");

  describe(`the WebAssembly package (${flavor})`, () => {
    const file = xlsxTest.getFixtureBuff("form");

    it(
      "should be the build it is said to be, and never a native addon",
      {},
      () => {
        assert.notEqual(binding.__napiBindingTarget, "native");
        assert.equal(binding.__napiBindingTarget, flavor);
      }
    );

    it("should read a workbook's cells, typed", {}, () => {
      const sheet = xlsxTest.first(spreadsheetCells(file));
      assert.equal(sheet.sheet, "Form");
      assert.partialDeepStrictEqual(xlsxTest.cell(sheet, "A1"), {
        type: "Text",
        value: "Maintenance Reimbursement Form"
      });
      assert.partialDeepStrictEqual(xlsxTest.cell(sheet, "C6"), {
        type: "Date",
        value: "2015-08-31",
        text: "8/31/2015"
      });
      assert.partialDeepStrictEqual(xlsxTest.cell(sheet, "C5"), {
        type: "Number",
        value: 427.5,
        text: "427.50",
        formula: "C3*C4"
      });
    });

    it(
      "should read a workbook built here, kept without compression",
      {},
      () => {
        const sheet = xlsxTest.first(spreadsheetCells(xlsxTest.short));
        assert.equal(sheet.sheet, "Few");
        assert.deepEqual(
          sheet.cells.map((cell) => [cell.ref, cell.text]),
          [
            ["A1", "0.25"],
            ["B1", "1.25"],
            ["C1", "2.25"],
            ["D1", "3.25"],
            ["E1", "4.25"],
            ["A2", "5.25"],
            ["B2", "6.25"],
            ["C2", "7.25"],
            ["D2", "8.25"],
            ["E2", "9.25"],
            ["A3", "10.25"],
            ["B3", "11.25"],
            ["C3", "12.25"],
            ["D3", "13.25"],
            ["E3", "14.25"]
          ]
        );
      }
    );

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
          spreadsheetCellsAsync(notXlsx),
          /^Error: malformed workbook: /
        );
        assert.throws(
          () => spreadsheetCells(notXlsx),
          /^Error: malformed workbook: /
        );
        // A package of another kind, and a workbook in the strict form of the format.
        await assert.rejects(
          spreadsheetCellsAsync(xlsxTest.getFixtureBuff("not-a-workbook")),
          /^Error: unsupported format: Word document package/
        );
        await assert.rejects(
          spreadsheetCellsAsync(xlsxTest.getFixtureBuff("strict")),
          /^Error: unsupported format: Strict OOXML workbook/
        );
      }
    );

    it(
      "should parse once in the class, in its constructor or from a promise",
      {},
      async () => {
        const doc = await XlsxDown.openAsync(file);
        assert.ok(doc instanceof XlsxDown);
        assert.deepEqual(doc.spreadsheetCells(), spreadsheetCells(file));
        assert.deepEqual(
          await doc.spreadsheetCellsAsync(),
          spreadsheetCells(file)
        );
        const meta = doc.metadata();
        assert.equal(meta.sheetCount, 2);
        assert.equal(meta.hiddenSheetCount, 1);
        assert.equal(meta.formulaCount, 4);
        assert.deepEqual(await doc.metadataAsync(), meta);
        assert.deepEqual(await doc.layoutAsync(), new XlsxDown(file).layout());
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
