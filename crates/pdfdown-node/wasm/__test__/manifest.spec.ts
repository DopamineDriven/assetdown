import assert from "node:assert/strict";
import { existsSync, readFileSync, rmSync } from "node:fs";
import { after, describe, it } from "node:test";

import { NapiCli } from "@napi-rs/cli";

import manifest from "@/wasm/package.json" with { type: "json" };

/**
 * The package's manifest is kept by hand, where the main package's parts are
 * written for it. These hold it to the two things a hand can get wrong: a
 * path that names a file the package does not ship, and a dependency that is
 * not the one the loaders were written against.
 */

/** Every file a value of the manifest's map of entries names. */
function named(entry: unknown): string[] {
  if (typeof entry === "string") return [entry];
  if (typeof entry !== "object" || entry === null) return Array.of<string>();
  return Object.values(entry).flatMap((inner) => named(inner));
}

describe("the WebAssembly package's manifest", () => {
  const shipped = new Set(manifest.files.map((file) => `./${file}`));

  it("should name no file that it does not ship", {}, () => {
    const paths = [
      `./${manifest.main}`,
      `./${manifest.types}`,
      `./${manifest.browser}`,
      ...named(manifest.exports)
    ];
    const missing = paths.filter(
      (path) => path !== "./package.json" && !shipped.has(path)
    );
    assert.deepEqual(missing, []);
  });

  it("should ship only files that are there once it is built", {}, () => {
    const absent = manifest.files.filter((file) => !existsSync(`wasm/${file}`));
    assert.deepEqual(absent, []);
  });

  it("should not ship the entry that looks for a native addon", {}, () => {
    for (const file of ["index.js", "index.d.ts", "browser.js"]) {
      assert.equal(manifest.files.includes(file), false, file);
    }
  });

  describe("against what the tool that writes the loaders would depend on", () => {
    const npmDir = "wasm/npm";
    after(() => rmSync(npmDir, { recursive: true, force: true }));

    it(
      "should depend on the same packages at the same versions",
      {},
      async () => {
        await new NapiCli().createNpmDirs({
          packageJsonPath: "wasm/package.json",
          npmDir
        });
        for (const flavor of ["wasm32-wasi", "wasm32-wasip1"]) {
          const written: unknown = JSON.parse(
            readFileSync(`${npmDir}/${flavor}/package.json`, "utf8")
          );
          assert.ok(typeof written === "object" && written !== null);
          assert.ok("dependencies" in written);
          assert.deepEqual(written.dependencies, manifest.dependencies, flavor);
        }
      }
    );
  });
});
