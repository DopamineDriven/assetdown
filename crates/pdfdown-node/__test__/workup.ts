import { resolve } from "node:path";

import { Fs } from "@d0paminedriven/fs";
// const fs = new Fs(process.cwd());

// const dir = fs
//   .readDir("__test__/fixtures", { recursive: true })
//   .filter((v) => v.endsWith(".pdf") && v.lastIndexOf(".") !== -1)
//   .map((o) => `__test__/fixtures/${o}`);


// const getKeys = dir
//   .map((o) => o.replace("__test__/fixtures/", ""))
//   .map((t) => t.slice(0, t.lastIndexOf(".")));
// console.log(getKeys)
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
      "ascii85-dct-scan": "__test__/fixtures/ascii85-dct-scan.pdf",
      "ccitt-g4-tiny": "__test__/fixtures/ccitt-g4-tiny.pdf",
      "einvoice/einvoice-broken": "__test__/fixtures/einvoice/einvoice-broken.pdf",
      "einvoice/facturx-minimal": "__test__/fixtures/einvoice/facturx-minimal.pdf",
      "einvoice/ubl-attachments": "__test__/fixtures/einvoice/ubl-attachments.pdf",
      "forms/acroform-basic-aes256":
        "__test__/fixtures/forms/acroform-basic-aes256.pdf",
      "forms/acroform-basic-rc4": "__test__/fixtures/forms/acroform-basic-rc4.pdf",
      "forms/acroform-basic": "__test__/fixtures/forms/acroform-basic.pdf",
      "forms/acroform-cycle": "__test__/fixtures/forms/acroform-cycle.pdf",
      "forms/acroform-dangling": "__test__/fixtures/forms/acroform-dangling.pdf",
      "forms/acroform-hostile": "__test__/fixtures/forms/acroform-hostile.pdf",
      "forms/acroform-inherited": "__test__/fixtures/forms/acroform-inherited.pdf",
      "forms/acroform-many": "__test__/fixtures/forms/acroform-many.pdf",
      "forms/acroform-orphans": "__test__/fixtures/forms/acroform-orphans.pdf",
      "invoices/Downloadable-PDF-Invoices-Add-On-Samples":
        "__test__/fixtures/invoices/Downloadable-PDF-Invoices-Add-On-Samples.pdf",
      "invoices/cluster-a": "__test__/fixtures/invoices/cluster-a.pdf",
      "invoices/cluster-b": "__test__/fixtures/invoices/cluster-b.pdf",
      "invoices/cluster-c": "__test__/fixtures/invoices/cluster-c.pdf",
      "invoices/credit-memo": "__test__/fixtures/invoices/credit-memo.pdf",
      "invoices/wordpress-pdf-invoice-plugin-sample":
        "__test__/fixtures/invoices/wordpress-pdf-invoice-plugin-sample.pdf",
      "layout/fonts-sample-II": "__test__/fixtures/layout/fonts-sample-II.pdf",
      "layout/fonts-sample": "__test__/fixtures/layout/fonts-sample.pdf",
      "layout/layout-sample": "__test__/fixtures/layout/layout-sample.pdf",
      "layout/sparse-native": "__test__/fixtures/layout/sparse-native.pdf"
    } as const;
  }

  public narrowFixture = (s: string) => {
    return (
      s === "ascii85-dct-scan" ||
      s === "ccitt-g4-tiny" ||
      s === "einvoice/einvoice-broken" ||
      s === "einvoice/facturx-minimal" ||
      s === "einvoice/ubl-attachments" ||
      s === "forms/acroform-basic-aes256" ||
      s === "forms/acroform-basic-rc4" ||
      s === "forms/acroform-basic" ||
      s === "forms/acroform-cycle" ||
      s === "forms/acroform-dangling" ||
      s === "forms/acroform-hostile" ||
      s === "forms/acroform-inherited" ||
      s === "forms/acroform-many" ||
      s === "forms/acroform-orphans" ||
      s === "invoices/Downloadable-PDF-Invoices-Add-On-Samples" ||
      s === "invoices/cluster-a" ||
      s === "invoices/cluster-b" ||
      s === "invoices/cluster-c" ||
      s === "invoices/credit-memo" ||
      s === "invoices/wordpress-pdf-invoice-plugin-sample" ||
      s === "layout/fonts-sample-II" ||
      s === "layout/fonts-sample" ||
      s === "layout/layout-sample" ||
      s === "layout/sparse-native"
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

Valid Options: "ascii85-dct-scan" | "ccitt-g4-tiny" | "einvoice/einvoice-broken" | "einvoice/facturx-minimal" | "einvoice/ubl-attachments" | "forms/acroform-basic" | "forms/acroform-basic-aes256" | "forms/acroform-basic-rc4" | "forms/acroform-basic" | "forms/acroform-cycle" | "forms/acroform-dangling" | "forms/acroform-hostile" | "forms/acroform-inherited" | "forms/acroform-many" | "forms/acroform-orphans" | "invoices/Downloadable-PDF-Invoices-Add-On-Samples" | "invoices/cluster-a" | "invoices/cluster-b" | "invoices/cluster-c" | "invoices/credit-memo" | "invoices/wordpress-pdf-invoice-plugin-sample" | "layout/fonts-sample" | "layout/fonts-sample-II" | "layout/layout-sample" | "layout/sparse-native"`
    );
  }
}
