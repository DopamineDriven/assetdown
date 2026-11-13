import { Fs } from "@d0paminedriven/fs";
import { resolve } from "node:path";

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

  public readAbsPath(
    paths: string[]
  ){
    return resolve(...paths);
  }

  public get workspaceRoot() {
    return this.readAbsPath([process.cwd(), "../.."])
  }

  public get fixturePaths() {
    return {
  "date1904": "__test__/fixtures/date1904.xlsx",
  "form": "__test__/fixtures/form.xlsx",
  "macro": "__test__/fixtures/macro.xlsm",
  "minimal": "__test__/fixtures/minimal.xlsx",
  "not-a-workbook": "__test__/fixtures/not-a-workbook.docx",
  "ole": "__test__/fixtures/ole.bin",
  "strict": "__test__/fixtures/strict.xlsx"
} as const;
  }

  public narrowFixture = (s: string) => {
    return (
      s === "date1904" ||
      s === "form" ||
      s === "macro" ||
      s === "minimal" ||
      s === "not-a-workbook" ||
      s === "ole" ||
      s === "strict"
    );
  };

  public getFixtureBuff<const T extends keyof typeof this.fixturePaths | (string & {}) = "form">(
    t: T
  ) {
    if (this.narrowFixture(t)) {
      return this.fileToBuffer(
        this.fixturePaths[t]
      ) satisfies Buffer<ArrayBuffer>;
    } else throw new Error(`invalid fixture name passed into getFixutreBuff ${t}`);
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

Valid Options: "date1904" | "form" | "macro" | "minimal" | "not-a-workbook" | "ole" | "strict"`
    );
  }
}
