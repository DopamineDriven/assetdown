import { resolve } from "node:path";

import { Fs } from "@d0paminedriven/fs";
// const fs = new Fs(process.cwd());

// const dir = fs
//   .readDir("ocr/__test__/fixtures", { recursive: true })
//   .filter((v) => v.endsWith(".pdf") && v.lastIndexOf(".") !== -1)
//   .map((o) => `__test__/fixtures/${o}`);

// const getKeys = dir
//   .map((o) => o.replace("__test__/fixtures/", ""))
//   .map((t) => t.slice(0, t.lastIndexOf(".")));
// console.log(getKeys);
/**
 *
 * For `Fs` derived methods
 *
 * to access any path, use the *Tmp styled methods
 *
 * to leverage the cwd, use the standard methods
 *
 * eg
 *
 * `readTmp` &rarr; Absolute, uses `resolve(...paths)` where `typeof paths = string[]`
 *
 * `fileToBuffer`&rarr; Standard, uses `relative(process.cwd(), path_arg_is_what_falls_in_here)`
 */
export class WorkupService extends Fs {
  constructor() {
    super(process.cwd());
  }
  public safeErrMsg(err: unknown) {
    if (err instanceof Error) {
      return err.message;
    } else if (typeof err === "object" && err != null) {
      return JSON.stringify(err, Object.getOwnPropertyNames(err), 2);
    } else if (typeof err === "string") {
      return err;
    } else if (typeof err === "number") {
      return err.toPrecision(5);
    } else if (typeof err === "boolean") {
      return `${err}`;
    } else return String(err);
  }

  public readAbsPath(paths: string[]) {
    return resolve(...paths);
  }

  public get workspaceRoot() {
    return this.readAbsPath([process.cwd(), "../.."]);
  }
  public get fixturePaths() {
    return {
      advanced_organic_chem_presentation_autumn_2014:
        "ocr/__test__fixtures/advanced_organic_chem_presentation_autumn_2014.pdf",
      "hello-ocr": "ocr/__test__/fixtures/hello-ocr.pdf",
      "multipage-ocr": "ocr/__test__/fixtures/multipage-ocr.pdf",
      "rotated-scan": "ocr/__test__/fixtures/rotated-scan.pdf",
      "stamped-scan": "ocr/__test__/fixtures/stamped-scan.pdf"
    } as const;
  }

  public narrowFixture = (s: string) => {
    return (
      s === "advanced_organic_chem_presentation_autumn_2014" ||
      s === "hello-ocr" ||
      s === "multipage-ocr" ||
      s === "rotated-scan" ||
      s === "stamped-scan"
    );
  };

  public getFixtureBuff<
    const T extends keyof typeof this.fixturePaths | (string & {}) = "form"
  >(t: T) {
    if (this.narrowFixture(t)) {
      return this.fileToBuffer(
        this.fixturePaths[t]
      ) satisfies Buffer<ArrayBuffer>;
    } else
      throw new Error(`invalid fixture name passed into getFixutreBuff ${t}`);
  }
}

if (process.argv[2] === "inspect" && process.argv[3]) {
  const workup = new WorkupService();
  if (workup.narrowFixture(process.argv[3])) {
    const fixtureBuff = workup.getFixtureBuff(process.argv[3]);
    console.log(fixtureBuff.toString("utf-8"));
  } else {
    console.warn(
      `${process.argv[3]} is an invalid argument.

Valid Options: "advanced_organic_chem_presentation_autumn_2014" | "hello-ocr" | "multipage-ocr" | "rotated-scan" | "stamped-scan"`
    );
  }
}
