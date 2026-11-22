import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { describe, it } from "node:test";
import { fileURLToPath } from "node:url";

import * as binding from "@d0paminedriven/rtfdown-wasm/threadless";
import { createInstance } from "@d0paminedriven/rtfdown-wasm/workerd";

import { RtfTest } from "@/__test__/rtf-test.ts";
import { wasmSuite } from "@/wasm/__test__/suite.ts";

wasmSuite(binding, binding.__napiBindingTarget satisfies "wasm32-wasip1");

describe("the entry for a worker runtime", () => {
  it("should start from a module its caller compiled", {}, async () => {
    // A worker runtime hands the module over already compiled, and reads no
    // file: here the file is read and compiled first, as that runtime would.
    const path = fileURLToPath(
      import.meta.resolve("@d0paminedriven/rtfdown-wasm/wasm.wasm")
    );
    const instance = await createInstance(
      await WebAssembly.compile(await readFile(path))
    );
    try {
      assert.equal(instance.exports.__napiBindingTarget, "wasm32-wasip1");
      const text = instance.exports.rtfText(new RtfTest().invoice);
      assert.equal(text.split("\n")[0], "INVOICE");
    } finally {
      await instance.dispose();
    }
  });
});
