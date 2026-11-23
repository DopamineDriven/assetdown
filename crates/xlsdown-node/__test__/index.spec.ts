import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { XlsTest } from "@/__test__/xls-test.ts";
import { XlsDown, spreadsheetCells, spreadsheetCellsAsync } from "@/index.js";

const t = new XlsTest();
const costs = t.costs;
const doc = new XlsDown(costs);
const sheet = t.first(doc.spreadsheetCells());

describe("XlsDown constructor", () => {
  it("should return an XlsDown for a workbook of Excel 97 to 2003", {}, () => {
    assert.equal(doc instanceof XlsDown, true);
  });
  it(
    "should throw unsupported format for a workbook saved with a password",
    {},
    () => {
      assert.throws(() => new XlsDown(t.locked), {
        message: "unsupported format: an encrypted workbook"
      });
    }
  );
  it("should throw malformed workbook for bytes that are not one", {}, () => {
    assert.throws(() => new XlsDown(Buffer.from("PK\u0003\u0004 a zip")), {
      message: "malformed workbook: not a compound file"
    });
  });
  it(
    "should resolve to an XlsDown from openAsync, and reject with the constructor's message",
    {},
    async () => {
      const opened = await XlsDown.openAsync(costs);
      assert.equal(opened instanceof XlsDown, true);
      assert.deepEqual(opened.spreadsheetCells(), doc.spreadsheetCells());
      await assert.rejects(XlsDown.openAsync(t.locked), {
        message: "unsupported format: an encrypted workbook"
      });
    }
  );
});

describe("spreadsheetCells", () => {
  it("should return the visible sheet with its cells in order", {}, () => {
    assert.equal(doc.spreadsheetCells().length, 1);
    assert.equal(sheet.sheet, "Costs");
    assert.deepEqual(
      sheet.cells.map((c) => c.ref),
      ["A1", "B1", "B2", "B3"]
    );
  });
  it("should type each cell by what it holds", {}, () => {
    assert.partialDeepStrictEqual(t.cell(sheet, "A1"), {
      type: "Text",
      value: "Total",
      text: "Total"
    });
    assert.partialDeepStrictEqual(t.cell(sheet, "B1"), {
      type: "Date",
      value: "2026-11-23",
      text: "11/23/2026",
      raw: "46349",
      format: "m/d/yyyy"
    });
    assert.partialDeepStrictEqual(t.cell(sheet, "B2"), {
      type: "Number",
      value: 1234.5,
      text: "1,234.50"
    });
  });
  it(
    "should give a formula's value, with its text present and empty",
    {},
    () => {
      assert.partialDeepStrictEqual(t.cell(sheet, "B3"), {
        type: "Number",
        value: 8,
        formula: "",
        cached: true
      });
      assert.equal(t.cell(sheet, "B2").formula, undefined);
    }
  );
  it("should include a hidden sheet when it is asked for", {}, () => {
    const all = doc.spreadsheetCells({ hiddenSheets: true });
    assert.deepEqual(
      all.map((s) => [s.sheet, s.hidden]),
      [
        ["Costs", false],
        ["Notes", true]
      ]
    );
    assert.deepEqual(
      doc.spreadsheetCells({ sheets: ["Notes"] }).map((s) => s.sheet),
      ["Notes"]
    );
  });
  it(
    "should return the same cells from the free function, its async twin, and the class's",
    {},
    async () => {
      const cells = doc.spreadsheetCells();
      assert.deepEqual(spreadsheetCells(costs), cells);
      assert.deepEqual(await spreadsheetCellsAsync(costs), cells);
      assert.deepEqual(await doc.spreadsheetCellsAsync(), cells);
    }
  );
  it(
    "should throw from the free function and reject from the async one for a locked workbook",
    {},
    async () => {
      const message = "unsupported format: an encrypted workbook";
      assert.throws(() => spreadsheetCells(t.locked), { message });
      await assert.rejects(spreadsheetCellsAsync(t.locked), { message });
    }
  );
});

describe("spreadsheetGrid, layout, and invoice", () => {
  it("should key the grid by row and column", {}, async () => {
    const grid = doc.spreadsheetGrid();
    assert.equal(grid.length, 1);
    assert.deepEqual(await doc.spreadsheetGridAsync(), grid);
  });
  it("should set each sheet on a page of its own", {}, async () => {
    const pages = doc.layout();
    assert.equal(pages.length, 1);
    assert.deepEqual(await doc.layoutAsync(), pages);
  });
  it("should read invoice fields from the pages", {}, async () => {
    const invoice = doc.invoice();
    assert.equal(typeof invoice, "object");
    assert.deepEqual(await doc.invoiceAsync(), invoice);
  });
});

describe("metadata", () => {
  const meta = doc.metadata();
  it("should count every sheet, the hidden one included", {}, () => {
    assert.equal(meta.sheetCount, 2);
    assert.equal(meta.hiddenSheetCount, 1);
    assert.equal(meta.cellCount, 5);
    assert.equal(meta.formulaCount, 1);
  });
  it("should say what kind of file the workbook came from", {}, () => {
    assert.equal(meta.hasMacros, false);
    assert.equal(meta.isTemplate, false);
    assert.equal(meta.date1904, false);
  });
  it("should resolve to the same metadata off the thread", {}, async () => {
    assert.deepEqual(await doc.metadataAsync(), meta);
  });
});
