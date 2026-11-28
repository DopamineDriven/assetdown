import { createRequire } from "node:module";
import { arch, platform } from "node:os";
import { dirname, join, relative } from "node:path";

import { wasm } from "rolldown-plugin-wasm";
import type { UserConfig } from "tsdown";
import { defineConfig } from "tsdown";

// TypeScript 7 ships its native compiler as `tsc`, in a package named for the platform. That
// package is a dependency of `typescript` and not of this one, so it is found the way Node
// finds it: from where `typescript` really is. This holds whether packages are hoisted into
// one folder or each is linked to exactly what it declares.
const executable = platform() === "win32" ? "tsc.exe" : "tsc";
const native = `@typescript/typescript-${platform()}-${arch()}/package.json`;
const fromHere = createRequire(join(process.cwd(), "package.json"));
const fromTypescript = createRequire(
  fromHere.resolve("typescript/package.json")
);
const path = join(dirname(fromTypescript.resolve(native)), "lib", executable);

export default defineConfig((options) => [
  // What a page imports: left as the files they are.
  {
    ...options,
    entry: ["src/client.ts", "src/protocol.ts"],
    dts: { tsgo: { path } },
    platform: "neutral",
    fixedExtension: false,
    target: ["esnext", "node26"],
    format: ["esm"],
    tsconfig: relative(process.cwd(), "tsconfig.json"),
    cwd: process.cwd(),
    clean: true,
    sourcemap: false,
    outDir: "dist",
    unbundle: true,
    // The declarations name the package's own build by the package's name, as a caller's
    // code would, and do not carry a copy of its declarations.
    deps: { neverBundle: [/^@d0paminedriven\/pdfdown-wasm/] }
  } satisfies UserConfig,
  // What runs in the worker: one file that holds the loader and its runtime, because a
  // worker is loaded by its address and has no bundler behind it. `browser` is what
  // picks the runtime a page can run.
  {
    ...options,
    entry: ["src/worker.ts"],
    dts: false,
    platform: "browser",
    fixedExtension: false,
    target: ["esnext"],
    format: ["esm"],
    tsconfig: relative(process.cwd(), "tsconfig.json"),
    cwd: process.cwd(),
    clean: false,
    sourcemap: false,
    outDir: "dist",
    unbundle: false,
    deps: { alwaysBundle: [/./], onlyBundle: false },
    // The build is put in `dist` by the bundler, which tells the worker where.
    plugins: [wasm({ maxFileSize: 0, fileName: "worker[extname]" })]
  } satisfies UserConfig
]);
