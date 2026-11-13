import { resolve } from "node:path";

import { Fs } from "@d0paminedriven/fs";
import { Bench } from "tinybench";

/**
 *
 * For `Fs` derived methods
 *
 * to access any path, use the *Tmp styled methods
 * to leverage the cwd, use the standard methods
 *
 * eg
 *
 * `readTmp` &rarr; Absolute, uses `resolve(...paths)` where `typeof paths = string[]`
 *
 * `fileToBuffer`&rarr; Standard, uses `relative(process.cwd(), path_arg_is_what_falls_in_here)`
 */
class BenchIt extends Fs {
  constructor(public b: Bench) {
    super(process.cwd());
  }

  protected readAbsPath(paths: string[]) {
    return resolve(...paths);
  }

  public get workspaceRoot() {
    return this.readAbsPath([process.cwd(), "../.."]);
  }
}

console.log(new BenchIt(new Bench()).workspaceRoot);
