import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { XlsTest } from "@/__test__/xls-test.ts";
import { XlsDown, spreadsheetCells, spreadsheetCellsAsync } from "@/index.js";
import type { SheetCells, SpreadsheetOptions } from "@/index.js";

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
  it("should give a formula's value and its text", {}, () => {
    assert.partialDeepStrictEqual(t.cell(sheet, "B3"), {
      type: "Number",
      value: 2469,
      formula: "B2*2",
      cached: true
    });
    assert.equal(t.cell(sheet, "B2").formula, undefined);
  });
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

describe("metadata of a workbook that says what it is", () => {
  const described = new XlsDown(t.described);
  const meta = described.metadata();
  it("should read what the file says about itself", {}, () => {
    assert.partialDeepStrictEqual(meta, {
      title: "Order form",
      subject: "Parts",
      author: "A. Clerk",
      keywords: "orders; parts",
      description: "For the spring",
      category: "Forms",
      language: "en-US",
      lastModifiedBy: "B. Checker",
      revision: "7",
      created: "2026-11-15T09:30:00Z",
      modified: "2026-11-15T10:30:00Z",
      application: "Microsoft Excel",
      appVersion: "12.0000",
      company: "Acme Tooling"
    });
  });
  it("should leave out what the file does not state", {}, () => {
    const bare = doc.metadata();
    assert.equal(bare.title, undefined);
    assert.equal(bare.author, undefined);
    assert.equal(bare.created, undefined);
    assert.equal(bare.modified, undefined);
    assert.equal(bare.application, undefined);
    assert.equal(bare.appVersion, undefined);
    assert.equal(bare.company, undefined);
  });
  it("should count the same whatever the file says about itself", {}, () => {
    const bare = doc.metadata();
    assert.equal(meta.sheetCount, bare.sheetCount);
    assert.equal(meta.cellCount, bare.cellCount);
    assert.equal(meta.formulaCount, bare.formulaCount);
    assert.equal(meta.wordCount, bare.wordCount);
    assert.deepEqual(meta.sheets, bare.sheets);
    assert.deepEqual(
      described.spreadsheetCells({ hiddenSheets: true }),
      doc.spreadsheetCells({ hiddenSheets: true })
    );
  });
  it("should resolve to the same metadata off the thread", {}, async () => {
    assert.deepEqual(await described.metadataAsync(), meta);
    const opened = await XlsDown.openAsync(t.described);
    assert.deepEqual(opened.metadata(), meta);
  });
});

describe("the async twins", () => {
  const choices = Array.of<SpreadsheetOptions | undefined>(
    undefined,
    { hiddenSheets: true },
    { sheets: ["Notes"] },
    { sheets: ["Notes", "Costs"], hiddenCells: false },
    { sheets: ["Nope"] }
  );
  it(
    "should each resolve to what its twin returns, whatever is asked for",
    {},
    async () => {
      for (const opts of choices) {
        assert.deepEqual(
          await doc.spreadsheetCellsAsync(opts),
          doc.spreadsheetCells(opts)
        );
        assert.deepEqual(
          await doc.spreadsheetGridAsync(opts),
          doc.spreadsheetGrid(opts)
        );
        assert.deepEqual(await doc.layoutAsync(opts), doc.layout(opts));
        assert.deepEqual(
          await spreadsheetCellsAsync(costs, opts),
          spreadsheetCells(costs, opts)
        );
      }
      assert.deepEqual(await doc.invoiceAsync(), doc.invoice());
      assert.deepEqual(await doc.metadataAsync(), doc.metadata());
    }
  );
  it("should pass its options through to the worker thread", {}, async () => {
    const all = await doc.spreadsheetCellsAsync({ hiddenSheets: true });
    assert.deepEqual(
      all.map((s) => s.sheet),
      ["Costs", "Notes"]
    );
    const none = await spreadsheetCellsAsync(costs, { sheets: ["Nope"] });
    assert.deepEqual(none, []);
  });
  it(
    "should return the same cells to 32 readers running at once",
    {},
    async () => {
      const expected = doc.spreadsheetCells({ hiddenSheets: true });
      const readers = Array.of<Promise<SheetCells[]>>();
      for (let i = 0; i < 16; i++) {
        readers.push(doc.spreadsheetCellsAsync({ hiddenSheets: true }));
        readers.push(spreadsheetCellsAsync(costs, { hiddenSheets: true }));
      }
      for (const result of await Promise.all(readers)) {
        assert.deepEqual(result, expected);
      }
    }
  );
  it(
    "should answer every kind of question at once from one parsed workbook",
    {},
    async () => {
      const [cells, grid, pages, invoice, meta] = await Promise.all([
        doc.spreadsheetCellsAsync(),
        doc.spreadsheetGridAsync(),
        doc.layoutAsync(),
        doc.invoiceAsync(),
        doc.metadataAsync()
      ]);
      assert.deepEqual(cells, doc.spreadsheetCells());
      assert.deepEqual(grid, doc.spreadsheetGrid());
      assert.deepEqual(pages, doc.layout());
      assert.deepEqual(invoice, doc.invoice());
      assert.deepEqual(meta, doc.metadata());
    }
  );
  it(
    "should not be affected when the caller overwrites its buffer after the async call",
    {},
    async () => {
      const scratch = Buffer.from(costs);
      const pending = spreadsheetCellsAsync(scratch);
      const opening = XlsDown.openAsync(scratch);
      scratch.fill(0);
      assert.deepEqual(await pending, doc.spreadsheetCells());
      assert.deepEqual((await opening).metadata(), doc.metadata());
    }
  );
  it("should keep answering after a call that was rejected", {}, async () => {
    const message = "malformed workbook: not a compound file";
    const outcomes = await Promise.allSettled([
      spreadsheetCellsAsync(costs),
      spreadsheetCellsAsync(Buffer.from("not a workbook")),
      XlsDown.openAsync(t.locked),
      spreadsheetCellsAsync(costs)
    ]);
    assert.deepEqual(
      outcomes.map((outcome) => outcome.status),
      ["fulfilled", "rejected", "rejected", "fulfilled"]
    );
    await assert.rejects(spreadsheetCellsAsync(Buffer.from("not a workbook")), {
      message
    });
    assert.deepEqual(
      await spreadsheetCellsAsync(costs),
      spreadsheetCells(costs)
    );
  });
  it(
    "should read a long workbook off the thread as it reads it on it",
    {},
    async () => {
      const long = t.long;
      const expected = t.first(spreadsheetCells(long));
      assert.equal(expected.cells.length, 40_000);
      const [fromBuffer, opened] = await Promise.all([
        spreadsheetCellsAsync(long),
        XlsDown.openAsync(long)
      ]);
      assert.deepEqual(t.first(fromBuffer), expected);
      assert.equal(opened.metadata().cellCount, 40_000);
      assert.deepEqual(t.first(await opened.spreadsheetCellsAsync()), expected);
    }
  );
});
