/**
 * Runs in a worker: loads the build without threads and answers one call at a
 * time. Everything it needs is bundled into this one file, and the `.wasm`
 * sits beside it, so the two can be served from anywhere together.
 */
// First, so that it is in place before the runtime under the build looks for a `Buffer`.
import "./buffer.ts";
import { instantiate } from "@d0paminedriven/mimedown-wasm/workerd";

// The bundler puts the build beside this file and says where.
import wasmUrl from "../mimedown-node-wasm.wasm32-wasip1.wasm?url";
import type { BindingArgs, BindingRT, Wire } from "./protocol.ts";

/**
 * The verbs as a worker calls them, with a file's bytes as a `Uint8Array`,
 * which is what a page has and what the build takes. These are written as
 * methods so that the build, however its declarations put it, fits.
 */
interface OnAPage {
  sniff(...args: BindingArgs<"sniff">): BindingRT<"sniff">;
  sniffFromRanges(
    ...args: BindingArgs<"sniffFromRanges">
  ): BindingRT<"sniffFromRanges">;
  specs(...args: BindingArgs<"specs">): BindingRT<"specs">;
  specsFromRanges(
    ...args: BindingArgs<"specsFromRanges">
  ): BindingRT<"specsFromRanges">;
  waveform(...args: BindingArgs<"waveform">): BindingRT<"waveform">;
  poster(...args: BindingArgs<"poster">): BindingRT<"poster">;
  keyframes(...args: BindingArgs<"keyframes">): BindingRT<"keyframes">;
  mediaTypesOf(...args: BindingArgs<"mediaTypesOf">): BindingRT<"mediaTypesOf">;
  mediaTypeOfPath(
    ...args: BindingArgs<"mediaTypeOfPath">
  ): BindingRT<"mediaTypeOfPath">;
  extensionsOf(...args: BindingArgs<"extensionsOf">): BindingRT<"extensionsOf">;
  builtWith(...args: BindingArgs<"builtWith">): BindingRT<"builtWith">;
}

function onAPage(verbs: OnAPage) {
  return verbs;
}

/** Says a thing to the page, checked against what the page expects to hear. */
function tell(said: Wire.WorkerToPage) {
  globalThis.postMessage(said);
}

function message(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}

/**
 * The build, compiled. A browser compiles a `.wasm` as it arrives only when the
 * server calls it `application/wasm`; from a server that does not, the whole
 * file is taken first and compiled then.
 */
async function compiled() {
  const response = await fetch(wasmUrl);
  if (!response.ok) {
    throw new Error(
      `the build was not found beside the worker (${response.status} for ${wasmUrl})`
    );
  }
  const type = response.headers.get("content-type") ?? "";
  return type.startsWith("application/wasm")
    ? WebAssembly.compileStreaming(response)
    : WebAssembly.compile(await response.arrayBuffer());
}

/**
 * The build, compiled and started, with the page told which it is; or nothing,
 * with the page told why.
 */
async function start() {
  try {
    const binding = await instantiate(await compiled());
    tell({ type: "ready", target: binding.__napiBindingTarget });
    return onAPage(binding);
  } catch (error) {
    tell({ type: "broken", message: message(error) });
    return undefined;
  }
}

function answer(verbs: OnAPage, call: Wire.PageToWorker.Call) {
  const { id } = call;
  switch (call.verb) {
    case "sniff":
      return {
        type: "answer",
        id,
        verb: call.verb,
        value: verbs.sniff(...call.args)
      } as const;
    case "sniffFromRanges":
      return {
        type: "answer",
        id,
        verb: call.verb,
        value: verbs.sniffFromRanges(...call.args)
      } as const;
    case "specs":
      return {
        type: "answer",
        id,
        verb: call.verb,
        value: verbs.specs(...call.args)
      } as const;
    case "specsFromRanges":
      return {
        type: "answer",
        id,
        verb: call.verb,
        value: verbs.specsFromRanges(...call.args)
      } as const;
    case "waveform":
      return {
        type: "answer",
        id,
        verb: call.verb,
        value: verbs.waveform(...call.args)
      } as const;
    case "poster":
      return {
        type: "answer",
        id,
        verb: call.verb,
        value: verbs.poster(...call.args)
      } as const;
    case "keyframes":
      return {
        type: "answer",
        id,
        verb: call.verb,
        value: verbs.keyframes(...call.args)
      } as const;
    case "mediaTypesOf":
      return {
        type: "answer",
        id,
        verb: call.verb,
        value: verbs.mediaTypesOf(...call.args)
      } as const;
    case "mediaTypeOfPath":
      return {
        type: "answer",
        id,
        verb: call.verb,
        value: verbs.mediaTypeOfPath(...call.args)
      } as const;
    case "extensionsOf":
      return {
        type: "answer",
        id,
        verb: call.verb,
        value: verbs.extensionsOf(...call.args)
      } as const;
    case "builtWith":
      return {
        type: "answer",
        id,
        verb: call.verb,
        value: verbs.builtWith(...call.args)
      } as const;
  }
}

/** The build, started once. Every call waits on it. */
const starting = start();

// Listening begins here, before the build is loaded: a call that arrives while it loads
// is answered when it is ready, in the order the calls came, and is not lost.
globalThis.addEventListener(
  "message",
  ({ data }: MessageEvent<Wire.PageToWorker>) => {
    void starting.then((verbs) => {
      if (!verbs) {
        tell({
          type: "failed",
          id: data.id,
          message: "the build did not start"
        });
        return;
      }
      try {
        tell(answer(verbs, data));
      } catch (error) {
        tell({ type: "failed", id: data.id, message: message(error) });
      }
    });
  }
);
