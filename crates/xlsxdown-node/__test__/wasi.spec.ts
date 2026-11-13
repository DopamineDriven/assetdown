import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { WorkupService } from "@/__test__/workup.ts";
import {
  XlsxDown,
  __napiBindingTarget,
  spreadsheetCells,
  spreadsheetCellsAsync
} from "@/index.js";
import type { CellBox, SheetCells } from "@/index.js";

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
  const form = t.getFixtureBuff("form");
  const first = (sheets: SheetCells[]) => {
    const [sheet] = sheets;
    assert.ok(sheet, "the workbook returned no sheets");
    return sheet;
  };
  const cell = (sheet: SheetCells, ref: string) => {
    const found = sheet.cells.find((c) => c.ref === ref);
    assert.ok(found, `${ref} is not among the cells of ${sheet.sheet}`);
    return found;
  };

  it("should load the wasm32-wasi binding and not the native addon", {}, () => {
    assert.equal(__napiBindingTarget, "wasm32-wasi");
  });
  it("should return an XlsxDown that reads the Form sheet", {}, () => {
    const doc = new XlsxDown(form);
    assert.equal(doc instanceof XlsxDown, true);
    assert.deepEqual(
      doc.spreadsheetCells().map((s) => [s.sheet, s.index, s.hidden]),
      [["Form", 0, false]]
    );
  });
  it(
    "should return the same numbers as the native build for sizes and boxes",
    {},
    () => {
      const sheet = first(spreadsheetCells(form));
      assert.partialDeepStrictEqual(sheet, { width: 385.5, height: 345 });
      assert.deepEqual(cell(sheet, "C3").box, {
        x0: 168,
        y0: 300,
        x1: 241.5,
        y1: 315
      } satisfies CellBox);
      assert.partialDeepStrictEqual(cell(sheet, "C5"), {
        type: "Number",
        value: 427.5,
        formula: "C3*C4"
      });
    }
  );
  it(
    "should return C4*2 for B17, a follower of the shared formula on B16",
    {},
    () => {
      assert.partialDeepStrictEqual(
        cell(first(spreadsheetCells(form)), "B17"),
        { formula: "C4*2", sharedFormula: "B16" }
      );
    }
  );
  it("should return Date 1904-01-01 for serial 0 in date1904.xlsx", {}, () => {
    const dates = first(spreadsheetCells(t.getFixtureBuff("date1904")));
    assert.partialDeepStrictEqual(cell(dates, "A1"), {
      type: "Date",
      value: "1904-01-01",
      text: "1/1/1904"
    });
  });
  it(
    "should resolve openAsync on a worker thread to the same cells as the constructor",
    {},
    async () => {
      const opened = await XlsxDown.openAsync(form);
      assert.equal(opened instanceof XlsxDown, true);
      assert.deepEqual(
        opened.spreadsheetCells(),
        new XlsxDown(form).spreadsheetCells()
      );
    }
  );
  it(
    "should resolve spreadsheetCellsAsync to the same cells as spreadsheetCells",
    {},
    async () => {
      const doc = new XlsxDown(form);
      assert.deepEqual(
        await doc.spreadsheetCellsAsync({ hiddenSheets: true }),
        doc.spreadsheetCells({ hiddenSheets: true })
      );
      assert.deepEqual(
        await spreadsheetCellsAsync(form),
        spreadsheetCells(form)
      );
    }
  );
  it(
    "should return the same cells to 32 readers running at once",
    {},
    async () => {
      const doc = new XlsxDown(form);
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
    "should throw from the constructor and reject from openAsync for macro.xlsm",
    {},
    async () => {
      const message =
        "unsupported format: macro-enabled workbook (not supported in this release)";
      const macro = t.getFixtureBuff("macro");
      assert.throws(() => new XlsxDown(macro), { message });
      await assert.rejects(XlsxDown.openAsync(macro), { message });
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
});
