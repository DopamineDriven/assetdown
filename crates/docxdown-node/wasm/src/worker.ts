/**
 * Runs in a worker: loads the build without threads and answers one call at a
 * time. Everything it needs is bundled into this one file, and the `.wasm`
 * sits beside it, so the two can be served from anywhere together.
 */
// First, so that it is in place before the runtime under the build looks for a `Buffer`.
import "./buffer.ts";
import { instantiate } from "@d0paminedriven/docxdown-wasm/workerd";

// The bundler puts the build beside this file and says where.
import wasmUrl from "../docxdown-node-wasm.wasm32-wasip1.wasm?url";
import type { BindingArgs, BindingRT, Wire } from "./protocol.ts";

/**
 * The verbs as a worker calls them, with a document's bytes as a `Uint8Array`:
 * a page has no `Buffer`. The build's declarations say `Buffer` because Node
 * has one, and the build takes either. These are written as methods so that a
 * build declared that way fits.
 */
interface OnAPage {
  docxAnnotations(
    ...args: BindingArgs<"docxAnnotations">
  ): BindingRT<"docxAnnotations">;
  docxBlocks(...args: BindingArgs<"docxBlocks">): BindingRT<"docxBlocks">;
  docxDocument(...args: BindingArgs<"docxDocument">): BindingRT<"docxDocument">;
  docxFormFields(
    ...args: BindingArgs<"docxFormFields">
  ): BindingRT<"docxFormFields">;
  docxImages(...args: BindingArgs<"docxImages">): BindingRT<"docxImages">;
  docxInvoice(...args: BindingArgs<"docxInvoice">): BindingRT<"docxInvoice">;
  docxLayout(...args: BindingArgs<"docxLayout">): BindingRT<"docxLayout">;
  docxMarkdown(...args: BindingArgs<"docxMarkdown">): BindingRT<"docxMarkdown">;
  docxMetadata(...args: BindingArgs<"docxMetadata">): BindingRT<"docxMetadata">;
  docxNotes(...args: BindingArgs<"docxNotes">): BindingRT<"docxNotes">;
  docxTables(...args: BindingArgs<"docxTables">): BindingRT<"docxTables">;
  docxText(...args: BindingArgs<"docxText">): BindingRT<"docxText">;
  docxTextPerPage(
    ...args: BindingArgs<"docxTextPerPage">
  ): BindingRT<"docxTextPerPage">;
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
    case "docxAnnotations":
      return {
        type: "answer",
        id,
        verb: call.verb,
        value: verbs.docxAnnotations(...call.args)
      } as const;
    case "docxBlocks":
      return {
        type: "answer",
        id,
        verb: call.verb,
        value: verbs.docxBlocks(...call.args)
      } as const;
    case "docxDocument":
      return {
        type: "answer",
        id,
        verb: call.verb,
        value: verbs.docxDocument(...call.args)
      } as const;
    case "docxFormFields":
      return {
        type: "answer",
        id,
        verb: call.verb,
        value: verbs.docxFormFields(...call.args)
      } as const;
    case "docxImages":
      return {
        type: "answer",
        id,
        verb: call.verb,
        value: verbs.docxImages(...call.args)
      } as const;
    case "docxInvoice":
      return {
        type: "answer",
        id,
        verb: call.verb,
        value: verbs.docxInvoice(...call.args)
      } as const;
    case "docxLayout":
      return {
        type: "answer",
        id,
        verb: call.verb,
        value: verbs.docxLayout(...call.args)
      } as const;
    case "docxMarkdown":
      return {
        type: "answer",
        id,
        verb: call.verb,
        value: verbs.docxMarkdown(...call.args)
      } as const;
    case "docxMetadata":
      return {
        type: "answer",
        id,
        verb: call.verb,
        value: verbs.docxMetadata(...call.args)
      } as const;
    case "docxNotes":
      return {
        type: "answer",
        id,
        verb: call.verb,
        value: verbs.docxNotes(...call.args)
      } as const;
    case "docxTables":
      return {
        type: "answer",
        id,
        verb: call.verb,
        value: verbs.docxTables(...call.args)
      } as const;
    case "docxText":
      return {
        type: "answer",
        id,
        verb: call.verb,
        value: verbs.docxText(...call.args)
      } as const;
    case "docxTextPerPage":
      return {
        type: "answer",
        id,
        verb: call.verb,
        value: verbs.docxTextPerPage(...call.args)
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
