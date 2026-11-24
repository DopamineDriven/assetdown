import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { XlsTest } from "@/__test__/xls-test.ts";
import {
  XlsDown,
  __napiBindingTarget,
  spreadsheetCells,
  spreadsheetCellsAsync
} from "@/index.js";
import type { SheetCells } from "@/index.js";

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
const t = new XlsTest();
const skip =
  __napiBindingTarget === "native"
    ? "the native addon is loaded; run yarn test:wasi"
    : false;

describe("wasm32-wasi build", { skip }, () => {
  const costs = t.costs;
  it("should load the wasm binding, not the native addon", {}, () => {
    assert.notEqual(__napiBindingTarget, "native");
  });
  it("should read the cells of a workbook", {}, () => {
    const sheet = t.first(spreadsheetCells(costs));
    assert.partialDeepStrictEqual(t.cell(sheet, "B1"), {
      type: "Date",
      value: "2026-11-23"
    });
    assert.partialDeepStrictEqual(t.cell(sheet, "B3"), {
      type: "Number",
      value: 2469,
      formula: "B2*2"
    });
  });
  it(
    "should read the same cells on a worker as on this thread, many at once",
    {},
    async () => {
      const doc = new XlsDown(costs);
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
    "should count what the workbook holds, on this thread and on a worker",
    {},
    async () => {
      const doc = await XlsDown.openAsync(costs);
      const meta = doc.metadata();
      assert.equal(meta.sheetCount, 2);
      assert.equal(meta.formulaCount, 1);
      assert.deepEqual(await doc.metadataAsync(), meta);
    }
  );
  it(
    "should throw from the constructor and reject from openAsync for a locked workbook",
    {},
    async () => {
      const message = "unsupported format: an encrypted workbook";
      assert.throws(() => new XlsDown(t.locked), { message });
      await assert.rejects(XlsDown.openAsync(t.locked), { message });
      assert.throws(() => new XlsDown(t.lockedByItsOwner), { message });
    }
  );
  it(
    "should read a workbook Excel locked with its own password like any other",
    {},
    async () => {
      const every = { hiddenSheets: true };
      const open = new XlsDown(t.ledger).spreadsheetCells(every);
      assert.equal(open.length, 3);
      assert.deepEqual(
        new XlsDown(t.lockedByExcel).spreadsheetCells(every),
        open
      );
      const later = await XlsDown.openAsync(t.lockedByExcel);
      assert.deepEqual(await later.spreadsheetCellsAsync(every), open);
    }
  );
});
