import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { WorkupService } from "@/__test__/workup.ts";
import { XlsxDown, spreadsheetCells, spreadsheetCellsAsync } from "@/index.js";
import type { CellBox, SheetCells, SheetColumn, SheetRow } from "@/index.js";

/**
 *
 * the WorkupService base can be regenerated with `yarn tsx __test__/gen.ts --gen workup`
 *
 */
class TestXlsx extends WorkupService {
  /** The first sheet of a result; every fixture has one. */
  public first(sheets: SheetCells[]) {
    const [sheet] = sheets;
    assert.ok(sheet, "the workbook returned no sheets");
    return sheet;
  }

  /** The cell at `ref`, failing the test when the sheet has none there. */
  public cell(sheet: SheetCells, ref: string) {
    const found = sheet.cells.find((c) => c.ref === ref);
    assert.ok(found, `${ref} is not among the cells of ${sheet.sheet}`);
    return found;
  }

  public hasCell(sheet: SheetCells, ref: string) {
    return sheet.cells.some((c) => c.ref === ref);
  }

  public sheetNames(sheets: SheetCells[]) {
    return sheets.map((s) => s.sheet);
  }
}

const t = new TestXlsx();
const form = t.getFixtureBuff("form");
const doc = new XlsxDown(form);
const sheet = t.first(doc.spreadsheetCells());

describe("XlsxDown constructor", () => {
  it("should return an XlsxDown for the form fixture", {}, () => {
    assert.equal(doc instanceof XlsxDown, true);
  });
  it(
    "should throw unsupported format for macro.xlsm, a macro-enabled workbook",
    {},
    () => {
      assert.throws(() => new XlsxDown(t.getFixtureBuff("macro")), {
        message:
          "unsupported format: macro-enabled workbook (not supported in this release)"
      });
    }
  );
  it(
    "should throw unsupported format for strict.xlsx, a Strict OOXML workbook",
    {},
    () => {
      assert.throws(() => new XlsxDown(t.getFixtureBuff("strict")), {
        message:
          "unsupported format: Strict OOXML workbook (not supported in this release)"
      });
    }
  );
  it(
    "should throw unsupported format for not-a-workbook.docx and say it is a Word document",
    {},
    () => {
      assert.throws(() => new XlsxDown(t.getFixtureBuff("not-a-workbook")), {
        message: "unsupported format: Word document package"
      });
    }
  );
  it(
    "should throw malformed workbook for ole.bin, which is not a zip",
    {},
    () => {
      assert.throws(() => new XlsxDown(t.getFixtureBuff("ole")), {
        message: /^malformed workbook: not a readable zip package/
      });
    }
  );
  it(
    "should throw malformed workbook for bytes that are not a file at all",
    {},
    () => {
      assert.throws(() => new XlsxDown(Buffer.from("hello")), {
        message: /^malformed workbook: /
      });
    }
  );
});

describe("XlsxDown.openAsync", () => {
  it(
    "should resolve to an XlsxDown that reads the same cells as the constructor",
    {},
    async () => {
      const opened = await XlsxDown.openAsync(form);
      assert.equal(opened instanceof XlsxDown, true);
      assert.deepEqual(opened.spreadsheetCells(), doc.spreadsheetCells());
    }
  );
  it(
    "should reject with the constructor's message for macro.xlsm",
    {},
    async () => {
      await assert.rejects(XlsxDown.openAsync(t.getFixtureBuff("macro")), {
        message:
          "unsupported format: macro-enabled workbook (not supported in this release)"
      });
    }
  );
  it("should reject with malformed workbook for ole.bin", {}, async () => {
    await assert.rejects(XlsxDown.openAsync(t.getFixtureBuff("ole")), {
      message: /^malformed workbook: not a readable zip package/
    });
  });
});

describe("spreadsheetCells sheet selection", () => {
  it("should return only the visible sheet Form with no options", {}, () => {
    assert.deepEqual(
      doc.spreadsheetCells().map((s) => [s.sheet, s.index, s.hidden]),
      [["Form", 0, false]]
    );
  });
  it(
    "should return the same sheets for null and for an empty object",
    {},
    () => {
      assert.deepEqual(doc.spreadsheetCells(null), doc.spreadsheetCells());
      assert.deepEqual(doc.spreadsheetCells({}), doc.spreadsheetCells());
    }
  );
  it("should return Form and Notes with hiddenSheets set to true", {}, () => {
    assert.deepEqual(
      t.sheetNames(doc.spreadsheetCells({ hiddenSheets: true })),
      ["Form", "Notes"]
    );
  });
  it(
    "should return the hidden sheet Notes when it is asked for by name",
    {},
    () => {
      const notes = t.first(doc.spreadsheetCells({ sheets: ["Notes"] }));
      assert.equal(notes.hidden, true);
      assert.equal(notes.index, 1);
      assert.deepEqual(
        notes.cells.map((c) => c.text),
        ["secret"]
      );
    }
  );
  it(
    "should return sheets in workbook order whatever order they are asked for in",
    {},
    () => {
      assert.deepEqual(
        t.sheetNames(doc.spreadsheetCells({ sheets: ["Notes", "Form"] })),
        ["Form", "Notes"]
      );
    }
  );
  it(
    "should return no sheets for a name the workbook does not have",
    {},
    () => {
      assert.deepEqual(doc.spreadsheetCells({ sheets: ["Nope"] }), []);
    }
  );
});

describe("spreadsheetCells sheet facts", () => {
  it("should return A1:H30 for the declared dimension", {}, () => {
    assert.equal(sheet.dimension, "A1:H30");
  });
  it("should return A1:F12 for the print area", {}, () => {
    assert.equal(sheet.printArea, "A1:F12");
  });
  it("should return the two merged ranges in A1 form", {}, () => {
    assert.deepEqual(sheet.merges, ["A1:F1", "A11:A12"]);
  });
  it("should warn that C22 holds serial 60, the 1900 leap day", {}, () => {
    assert.equal(
      sheet.warnings.some((w) => /C22.*serial 60/.test(w)),
      true
    );
  });
});

describe("spreadsheetCells cell values", () => {
  it("should return Text for A1 with its merge and position", {}, () => {
    assert.partialDeepStrictEqual(t.cell(sheet, "A1"), {
      type: "Text",
      value: "Maintenance Reimbursement Form",
      text: "Maintenance Reimbursement Form",
      merge: "A1:F1",
      row: 0,
      col: 0
    });
  });
  it(
    "should return Number for C3 with its stored spelling and format code",
    {},
    () => {
      const c3 = t.cell(sheet, "C3");
      assert.partialDeepStrictEqual(c3, {
        type: "Number",
        value: 142.5,
        raw: "142.5",
        format: '#,##0.00"/hr"',
        formatSupported: true,
        cached: true
      });
      assert.equal(c3.formula, undefined);
    }
  );
  it("should return Float with no currency symbol for C3", {}, () => {
    const c3 = t.cell(sheet, "C3");
    assert.partialDeepStrictEqual(c3, { type: "Number", subtype: "Float" });
    assert.equal("currencySymbol" in c3, false);
  });
  it("should return Integer for C4", {}, () => {
    assert.partialDeepStrictEqual(t.cell(sheet, "C4"), {
      type: "Number",
      subtype: "Integer"
    });
  });
  it("should return Integer with the $ currency symbol for C8", {}, () => {
    assert.partialDeepStrictEqual(t.cell(sheet, "C8"), {
      type: "Number",
      subtype: "Integer",
      currencySymbol: "$"
    });
  });
  it("should return Float for C9", {}, () => {
    assert.partialDeepStrictEqual(t.cell(sheet, "C9"), {
      type: "Number",
      subtype: "Float"
    });
  });
  it("should return Text 000123 for C7, keeping the leading zeros", {}, () => {
    assert.partialDeepStrictEqual(t.cell(sheet, "C7"), {
      type: "Text",
      value: "000123",
      text: "000123",
      raw: "000123"
    });
  });
  it("should return Date for C6 as an ISO date with no time zone", {}, () => {
    const c6 = t.cell(sheet, "C6");
    if (c6.type !== "Date") assert.fail(`C6 is ${c6.type}, not Date`);
    assert.match(c6.value, /^\d{4}-\d{2}-\d{2}$/);
  });
  it("should return Error #DIV/0! for D20", {}, () => {
    assert.partialDeepStrictEqual(t.cell(sheet, "D20"), {
      type: "Error",
      value: "#DIV/0!",
      text: "#DIV/0!"
    });
  });
  it("should return Boolean true for E21 with text TRUE", {}, () => {
    assert.partialDeepStrictEqual(t.cell(sheet, "E21"), {
      type: "Boolean",
      value: true,
      text: "TRUE"
    });
  });
  it("should return both lines of text for the merged cell A11", {}, () => {
    assert.partialDeepStrictEqual(t.cell(sheet, "A11"), {
      text: "Line one\nLine two",
      merge: "A11:A12"
    });
  });
  it("should return no cell for G20, which is styled but empty", {}, () => {
    assert.equal(t.hasCell(sheet, "G20"), false);
  });
});

describe("spreadsheetCells formulas", () => {
  it("should return the formula C3*C4 and its cached 427.5 for C5", {}, () => {
    const c5 = t.cell(sheet, "C5");
    assert.partialDeepStrictEqual(c5, {
      type: "Number",
      value: 427.5,
      formula: "C3*C4",
      cached: true
    });
    assert.equal(c5.sharedFormula, undefined);
  });
  it("should return C3*2 for B16, the master of a shared formula", {}, () => {
    const b16 = t.cell(sheet, "B16");
    assert.partialDeepStrictEqual(b16, {
      type: "Number",
      value: 285,
      subtype: "Integer",
      formula: "C3*2",
      cached: true
    });
    assert.equal(b16.sharedFormula, undefined);
  });
  it(
    "should return C4*2 for B17, the master's formula moved one row down, and name B16",
    {},
    () => {
      assert.partialDeepStrictEqual(t.cell(sheet, "B17"), {
        type: "Text",
        value: "",
        text: "",
        formula: "C4*2",
        sharedFormula: "B16",
        cached: true
      });
    }
  );
  it(
    "should return Blank with formula C5*2 and cached false for B18, which stores no result",
    {},
    () => {
      const b18 = t.cell(sheet, "B18");
      assert.partialDeepStrictEqual(b18, {
        type: "Blank",
        text: "",
        formula: "C5*2",
        sharedFormula: "B16",
        cached: false
      });
      assert.equal("value" in b18, false);
    }
  );
});

describe("spreadsheetCells hidden cells", () => {
  it("should return B10 with hidden true by default", {}, () => {
    assert.partialDeepStrictEqual(t.cell(sheet, "B10"), {
      text: "hidden value",
      hidden: true
    });
  });
  it("should return no B10 with hiddenCells set to false", {}, () => {
    const visible = t.first(doc.spreadsheetCells({ hiddenCells: false }));
    assert.equal(t.hasCell(visible, "B10"), false);
    assert.equal(visible.cells.length, sheet.cells.length - 1);
  });
  it(
    "should return height 330 and a zero-height row 10 with hiddenCells set to false",
    {},
    () => {
      const visible = t.first(doc.spreadsheetCells({ hiddenCells: false }));
      assert.equal(visible.height, 330);
      assert.deepEqual(visible.rows, [
        { index: 9, height: 0, hidden: true },
        { index: 10, height: 30, hidden: false }
      ] satisfies SheetRow[]);
    }
  );
});

describe("spreadsheetCells geometry", () => {
  it("should return a 385.5 by 345 point frame for A1:F22", {}, () => {
    assert.partialDeepStrictEqual(sheet, {
      width: 385.5,
      height: 345,
      defaultColumnWidth: 48,
      defaultRowHeight: 15
    });
  });
  it(
    "should return only the three columns that differ from the default",
    {},
    () => {
      assert.deepEqual(sheet.columns, [
        { index: 0, width: 105, hidden: false },
        { index: 1, width: 63, hidden: false },
        { index: 2, width: 73.5, hidden: false }
      ] satisfies SheetColumn[]);
    }
  );
  it("should return the hidden row 10 and the taller row 11", {}, () => {
    assert.deepEqual(sheet.rows, [
      { index: 9, height: 15, hidden: true },
      { index: 10, height: 30, hidden: false }
    ] satisfies SheetRow[]);
  });
  it("should return the box of C3 in points, y up", {}, () => {
    assert.deepEqual(t.cell(sheet, "C3").box, {
      x0: 168,
      y0: 300,
      x1: 241.5,
      y1: 315
    } satisfies CellBox);
  });
  it("should return a box spanning A1:F1 for the merged anchor A1", {}, () => {
    assert.deepEqual(t.cell(sheet, "A1").box, {
      x0: 0,
      y0: 330,
      x1: 385.5,
      y1: 345
    } satisfies CellBox);
  });
  it(
    "should return a box spanning two rows for the merged anchor A11",
    {},
    () => {
      assert.deepEqual(t.cell(sheet, "A11").box, {
        x0: 0,
        y0: 150,
        x1: 105,
        y1: 195
      } satisfies CellBox);
    }
  );
  it("should return a box inside the sheet frame for every cell", {}, () => {
    for (const { ref, box } of sheet.cells) {
      assert.equal(
        box.x0 <= box.x1 &&
          box.y0 < box.y1 &&
          box.x1 <= sheet.width &&
          box.y1 <= sheet.height,
        true,
        `${ref} lies outside the frame`
      );
    }
  });
});

describe("spreadsheetCells other fixtures", () => {
  it(
    "should return 4 cells and no columns for minimal.xlsx, which has no shared strings or styles",
    {},
    () => {
      const minimal = t.first(spreadsheetCells(t.getFixtureBuff("minimal")));
      assert.equal(minimal.cells.length, 4);
      assert.deepEqual(minimal.columns, []);
    }
  );
  it(
    "should return Date 1904-01-01 for serial 0 in date1904.xlsx, a 1904-system workbook",
    {},
    () => {
      const dates = t.first(spreadsheetCells(t.getFixtureBuff("date1904")));
      assert.equal(dates.sheet, "Dates");
      assert.partialDeepStrictEqual(t.cell(dates, "A1"), {
        type: "Date",
        value: "1904-01-01",
        text: "1/1/1904",
        raw: "0",
        format: "m/d/yyyy"
      });
    }
  );
});

describe("spreadsheetCellsAsync and the free functions", () => {
  it(
    "should resolve to the same cells as spreadsheetCells on one XlsxDown",
    {},
    async () => {
      assert.deepEqual(
        await doc.spreadsheetCellsAsync(),
        doc.spreadsheetCells()
      );
    }
  );
  it("should pass its options through to the worker thread", {}, async () => {
    assert.deepEqual(
      t.sheetNames(await doc.spreadsheetCellsAsync({ hiddenSheets: true })),
      ["Form", "Notes"]
    );
  });
  it(
    "should return the same cells from the free function as from the class",
    {},
    () => {
      assert.deepEqual(spreadsheetCells(form), doc.spreadsheetCells());
      assert.deepEqual(
        spreadsheetCells(form, { hiddenSheets: true }),
        doc.spreadsheetCells({ hiddenSheets: true })
      );
    }
  );
  it(
    "should resolve to the same cells from the async free function as from the class",
    {},
    async () => {
      assert.deepEqual(
        await spreadsheetCellsAsync(form),
        doc.spreadsheetCells()
      );
    }
  );
  it(
    "should throw from the free function and reject from the async one for macro.xlsm",
    {},
    async () => {
      const message =
        "unsupported format: macro-enabled workbook (not supported in this release)";
      const macro = t.getFixtureBuff("macro");
      assert.throws(() => spreadsheetCells(macro), { message });
      await assert.rejects(spreadsheetCellsAsync(macro), { message });
    }
  );
  it(
    "should return the same cells to 32 readers running at once",
    {},
    async () => {
      const expected = doc.spreadsheetCells({ hiddenSheets: true });
      const readers = Array.of<Promise<SheetCells[]>>();
      for (let i = 0; i < 16; i++) {
        readers.push(doc.spreadsheetCellsAsync({ hiddenSheets: true }));
        readers.push(spreadsheetCellsAsync(form, { hiddenSheets: true }));
      }
      for (const result of await Promise.all(readers)) {
        assert.deepEqual(result, expected);
      }
    }
  );
  it(
    "should not be affected when the caller overwrites its buffer after the async call",
    {},
    async () => {
      const scratch = Buffer.from(form);
      const pending = spreadsheetCellsAsync(scratch);
      scratch.fill(0);
      assert.deepEqual(await pending, doc.spreadsheetCells());
    }
  );
});

describe("layout", () => {
  const pages = doc.layout();
  const [page] = pages;
  assert.ok(page, "the form workbook has one visible sheet");

  it(
    "should lay the visible sheet out as page 1 with the sheet's facts beside it",
    {},
    () => {
      assert.equal(pages.length, 1);
      assert.equal(page.page, 1);
      assert.equal(page.sheet, "Form");
      assert.equal(page.index, 0);
      assert.equal(page.hidden, false);
      assert.equal(page.dimension, "A1:H30");
      assert.equal(page.printArea, "A1:F12");
      assert.deepEqual(page.merges, ["A1:F1", "A11:A12"]);
      assert.equal(page.width, 385.5);
      assert.equal(page.height, 180);
    }
  );
  it("should name the cell every word was laid out from", {}, () => {
    assert.deepEqual(
      page.words.slice(0, 4).map((w) => [w.text, w.cell]),
      [
        ["Maintenance", "A1"],
        ["Reimbursement", "A1"],
        ["Form", "A1"],
        ["Rate", "B3"]
      ]
    );
    for (const word of page.words) {
      assert.match(word.cell, /^[A-Z]+[0-9]+$/);
    }
  });
  it("should carry the cell's font on each word", {}, () => {
    const [title] = page.words;
    assert.ok(title);
    assert.equal(title.fontName, "Arial");
    assert.equal(title.fontSize, 10);
    assert.equal(title.bold, true);
  });
  it(
    "should group the words into lines that each address a slice of the words",
    {},
    () => {
      assert.equal(page.lines.length, 10);
      const [first] = page.lines;
      assert.ok(first);
      assert.equal(first.text, "Maintenance Reimbursement Form");
      for (const [index, line] of page.lines.entries()) {
        const words = page.words.slice(
          line.firstWord,
          line.firstWord + line.wordCount
        );
        assert.equal(words.map((w) => w.text).join(" "), line.text);
        for (const word of words) {
          assert.equal(word.line, index);
        }
      }
    }
  );
  it("should keep every word inside the page", {}, () => {
    for (const word of page.words) {
      assert.ok(word.x0 >= 0 && word.x1 <= page.width, word.text);
      assert.ok(word.y0 >= 0 && word.y1 <= page.height, word.text);
    }
  });
  it("should warn about the cells the page leaves out", {}, () => {
    assert.ok(
      page.warnings.some((w) => w.includes("outside it are off the page"))
    );
    assert.ok(
      page.warnings.some((w) =>
        w.includes("hidden rows or columns are omitted")
      )
    );
  });
  it(
    "should put cells of hidden rows and columns on the page when hiddenCells is set",
    {},
    () => {
      const [withHidden] = doc.layout({ hiddenCells: true });
      assert.ok(withHidden);
      assert.ok(withHidden.words.length > page.words.length);
    }
  );
  it(
    "should number the pages in selection order when hidden sheets are included",
    {},
    () => {
      assert.deepEqual(
        doc.layout({ hiddenSheets: true }).map((p) => [p.sheet, p.page]),
        [
          ["Form", 1],
          ["Notes", 2]
        ]
      );
    }
  );
  it("layoutAsync should resolve to what layout returns", {}, async () => {
    assert.deepEqual(await doc.layoutAsync(), pages);
    assert.deepEqual(
      await doc.layoutAsync({ hiddenSheets: true }),
      doc.layout({ hiddenSheets: true })
    );
  });
});

describe("invoice", () => {
  const invoice = doc.invoice();

  it("should read the form's invoice fields from its cells", {}, () => {
    assert.equal(invoice.source, "Native");
    assert.equal(invoice.documentType, "Invoice");
    assert.equal(invoice.fields.invoiceId?.value, "000123");
    assert.equal(invoice.fields.invoiceDate?.value, "2015-08-31");
    assert.equal(invoice.fields.invoiceTotal?.value, 427.5);
    assert.equal(invoice.embeddedXml, undefined);
  });
  it("should give the same result with empty options", {}, () => {
    assert.deepEqual(doc.invoice({}), invoice);
  });
  it("invoiceAsync should resolve to what invoice returns", {}, async () => {
    assert.deepEqual(await doc.invoiceAsync(), invoice);
  });
});

describe("spreadsheetGrid", () => {
  const [grid] = doc.spreadsheetGrid();
  assert.ok(grid, "the form workbook has one visible sheet");

  it(
    "should hold exactly the cells spreadsheetCells returns, keyed by row then column",
    {},
    () => {
      let count = 0;
      for (const row of Object.values(grid.rows)) {
        count += Object.keys(row).length;
      }
      assert.equal(count, sheet.cells.length);
      for (const cell of sheet.cells) {
        const column = cell.ref.replace(/[0-9]+$/, "");
        const row = cell.ref.slice(column.length);
        assert.deepEqual(grid.rows[row]?.[column], cell);
      }
    }
  );
  it("should give a whole row in column order", {}, () => {
    const third = grid.rows["3"];
    assert.ok(third);
    assert.deepEqual(Object.keys(third), ["B", "C"]);
    assert.deepEqual(
      Object.values(third).map((c) => c.ref),
      ["B3", "C3"]
    );
  });
  it("should list the rows in sheet order and skip empty ones", {}, () => {
    const rows = Object.keys(grid.rows);
    assert.deepEqual(rows.slice(0, 3), ["1", "3", "4"]);
    assert.equal(grid.rows["2"], undefined);
  });
  it("should carry the sheet's facts", {}, () => {
    assert.equal(grid.sheet, "Form");
    assert.equal(grid.index, 0);
    assert.equal(grid.hidden, false);
    assert.equal(grid.dimension, "A1:H30");
    assert.deepEqual(grid.merges, ["A1:F1", "A11:A12"]);
  });
  it(
    "spreadsheetGridAsync should resolve to what spreadsheetGrid returns",
    {},
    async () => {
      assert.deepEqual(await doc.spreadsheetGridAsync(), [grid]);
    }
  );
});

describe("metadata", () => {
  const meta = doc.metadata();
  const everything = { hiddenSheets: true, hiddenCells: true } as const;

  it("should list every sheet, the hidden one included", {}, () => {
    assert.equal(meta.sheetCount, 2);
    assert.equal(meta.hiddenSheetCount, 1);
    assert.deepEqual(
      meta.sheets.map((s) => [s.name, s.index, s.hidden]),
      [
        ["Form", 0, false],
        ["Notes", 1, true]
      ]
    );
  });
  it("should count each sheet's cells, formulas and merged ranges", {}, () => {
    const [formSheet, notes] = meta.sheets;
    assert.ok(formSheet, "the form's first sheet is missing");
    assert.ok(notes, "the form's second sheet is missing");
    assert.equal(formSheet.cellCount, 23);
    assert.equal(formSheet.formulaCount, 4);
    assert.equal(formSheet.mergedRangeCount, 2);
    assert.equal(notes.cellCount, 1);
    assert.equal(notes.formulaCount, 0);
  });
  it(
    "should give the used range beside the dimension the file declares",
    {},
    () => {
      const [formSheet, notes] = meta.sheets;
      assert.ok(formSheet, "the form's first sheet is missing");
      assert.ok(notes, "the form's second sheet is missing");
      // The file declares more than it holds.
      assert.equal(formSheet.dimension, "A1:H30");
      assert.equal(formSheet.usedRange, "A1:E22");
      assert.equal(formSheet.rowCount, 22);
      assert.equal(formSheet.columnCount, 5);
      assert.equal(notes.usedRange, "A1:A1");
      assert.equal(notes.rowCount, 1);
      assert.equal(notes.columnCount, 1);
    }
  );
  it(
    "should count in total what spreadsheetCells returns with nothing held back",
    {},
    () => {
      const sheets = doc.spreadsheetCells(everything);
      assert.equal(
        meta.cellCount,
        sheets.reduce((total, s) => total + s.cells.length, 0)
      );
      assert.equal(
        meta.formulaCount,
        sheets.reduce(
          (total, s) =>
            total + s.cells.filter((c) => c.formula !== undefined).length,
          0
        )
      );
      assert.equal(
        meta.mergedRangeCount,
        sheets.reduce((total, s) => total + s.merges.length, 0)
      );
      assert.equal(meta.cellCount, 24);
    }
  );
  it(
    "should count the words and characters of the cells that hold text",
    {},
    () => {
      assert.equal(meta.wordCount, 18);
      assert.equal(meta.characterCount, 96);
    }
  );
  it("should count nothing the form does not hold", {}, () => {
    assert.equal(meta.tableCount, 0);
    assert.equal(meta.imageCount, 0);
    assert.equal(meta.chartCount, 0);
    assert.equal(meta.pivotTableCount, 0);
    assert.equal(meta.linkCount, 0);
    assert.equal(meta.commentCount, 0);
    assert.equal(meta.annotationCount, 0);
  });
  it(
    "should not count the print area among the names the workbook defines",
    {},
    () => {
      assert.equal(sheet.printArea, "A1:F12");
      assert.equal(meta.definedNameCount, 0);
    }
  );
  it("should leave out the properties the file does not state", {}, () => {
    assert.equal(meta.title, undefined);
    assert.equal(meta.author, undefined);
    assert.equal(meta.created, undefined);
    assert.equal(meta.application, undefined);
  });
  it("should say which date system the workbook counts in", {}, () => {
    assert.equal(meta.date1904, false);
    assert.equal(
      new XlsxDown(t.getFixtureBuff("date1904")).metadata().date1904,
      true
    );
  });
  it("should describe the minimal fixture's one sheet", {}, () => {
    const minimal = new XlsxDown(t.getFixtureBuff("minimal")).metadata();
    assert.equal(minimal.sheetCount, 1);
    assert.equal(minimal.hiddenSheetCount, 0);
    assert.equal(minimal.cellCount, 4);
    const [only] = minimal.sheets;
    assert.ok(only, "the minimal fixture's sheet is missing");
    assert.equal(only.usedRange, "A1:B2");
    assert.equal(only.rowCount, 2);
    assert.equal(only.columnCount, 2);
  });
  it("metadataAsync should resolve to what metadata returns", {}, async () => {
    assert.deepEqual(await doc.metadataAsync(), meta);
  });
});
