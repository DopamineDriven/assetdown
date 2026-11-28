import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { describe, it } from "node:test";
import { fileURLToPath } from "node:url";

import * as binding from "@d0paminedriven/docxdown-wasm/threadless";
import { createInstance } from "@d0paminedriven/docxdown-wasm/workerd";

import { WorkupService } from "@/__test__/workup.ts";
import { wasmSuite } from "@/wasm/__test__/suite.ts";

wasmSuite(binding, binding.__napiBindingTarget satisfies "wasm32-wasip1");

describe("the entry for a worker runtime", () => {
  it("should start from a module its caller compiled", {}, async () => {
    // A worker runtime hands the module over already compiled, and reads no
    // file: here the file is read and compiled first, as that runtime would.
    const path = fileURLToPath(
      import.meta.resolve("@d0paminedriven/docxdown-wasm/wasm.wasm")
    );
    const instance = await createInstance(
      await WebAssembly.compile(await readFile(path))
    );
    try {
      assert.equal(instance.exports.__napiBindingTarget, "wasm32-wasip1");
      const text = instance.exports.docxText(
        new WorkupService().getFixtureBuff("lo-pandoc.docx")
      );
      assert.equal(text.split("\n").at(-1), "Total due: 25.50 USD");
    } finally {
      await instance.dispose();
    }
  });
});
