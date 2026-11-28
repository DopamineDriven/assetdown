/**
 * Runs in a worker: loads the build without threads and answers one call at a
 * time. Everything it needs is bundled into this one file, and the `.wasm`
 * sits beside it, so the two can be served from anywhere together.
 */
// First, so that it is in place before the runtime under the build looks for a `Buffer`.
import "./buffer.ts";
import { instantiate } from "@d0paminedriven/pdfdown-wasm/workerd";

// The bundler puts the build beside this file and says where.
import wasmUrl from "../pdfdown-node-wasm.wasm32-wasip1.wasm?url";
import type { BindingArgs, BindingRT, Wire } from "./protocol.ts";

/**
 * The verbs as a worker calls them, with a document's bytes as a `Uint8Array`:
 * a page has no `Buffer`. The build's declarations say `Buffer` because Node
 * has one, and the build takes either. These are written as methods so that a
 * build declared that way fits.
 */
interface OnAPage {
  attachments(...args: BindingArgs<"attachments">): BindingRT<"attachments">;
  capabilities(...args: BindingArgs<"capabilities">): BindingRT<"capabilities">;
  extractAnnotationsPerPage(
    ...args: BindingArgs<"extractAnnotationsPerPage">
  ): BindingRT<"extractAnnotationsPerPage">;
  extractFormFieldsPerPage(
    ...args: BindingArgs<"extractFormFieldsPerPage">
  ): BindingRT<"extractFormFieldsPerPage">;
  extractImagesPerPage(
    ...args: BindingArgs<"extractImagesPerPage">
  ): BindingRT<"extractImagesPerPage">;
  extractInvoice(
    ...args: BindingArgs<"extractInvoice">
  ): BindingRT<"extractInvoice">;
  extractStructuredTextPerPage(
    ...args: BindingArgs<"extractStructuredTextPerPage">
  ): BindingRT<"extractStructuredTextPerPage">;
  extractTextPerPage(
    ...args: BindingArgs<"extractTextPerPage">
  ): BindingRT<"extractTextPerPage">;
  invoiceFromLayout(
    ...args: BindingArgs<"invoiceFromLayout">
  ): BindingRT<"invoiceFromLayout">;
  layoutPerPage(
    ...args: BindingArgs<"layoutPerPage">
  ): BindingRT<"layoutPerPage">;
  pdfDocument(...args: BindingArgs<"pdfDocument">): BindingRT<"pdfDocument">;
  pdfImageCount(
    ...args: BindingArgs<"pdfImageCount">
  ): BindingRT<"pdfImageCount">;
  pdfMetadata(...args: BindingArgs<"pdfMetadata">): BindingRT<"pdfMetadata">;
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
    case "attachments":
      return {
        type: "answer",
        id,
        verb: call.verb,
        value: verbs.attachments(...call.args)
      } as const;
    case "capabilities":
      return {
        type: "answer",
        id,
        verb: call.verb,
        value: verbs.capabilities(...call.args)
      } as const;
    case "extractAnnotationsPerPage":
      return {
        type: "answer",
        id,
        verb: call.verb,
        value: verbs.extractAnnotationsPerPage(...call.args)
      } as const;
    case "extractFormFieldsPerPage":
      return {
        type: "answer",
        id,
        verb: call.verb,
        value: verbs.extractFormFieldsPerPage(...call.args)
      } as const;
    case "extractImagesPerPage":
      return {
        type: "answer",
        id,
        verb: call.verb,
        value: verbs.extractImagesPerPage(...call.args)
      } as const;
    case "extractInvoice":
      return {
        type: "answer",
        id,
        verb: call.verb,
        value: verbs.extractInvoice(...call.args)
      } as const;
    case "extractStructuredTextPerPage":
      return {
        type: "answer",
        id,
        verb: call.verb,
        value: verbs.extractStructuredTextPerPage(...call.args)
      } as const;
    case "extractTextPerPage":
      return {
        type: "answer",
        id,
        verb: call.verb,
        value: verbs.extractTextPerPage(...call.args)
      } as const;
    case "invoiceFromLayout":
      return {
        type: "answer",
        id,
        verb: call.verb,
        value: verbs.invoiceFromLayout(...call.args)
      } as const;
    case "layoutPerPage":
      return {
        type: "answer",
        id,
        verb: call.verb,
        value: verbs.layoutPerPage(...call.args)
      } as const;
    case "pdfDocument":
      return {
        type: "answer",
        id,
        verb: call.verb,
        value: verbs.pdfDocument(...call.args)
      } as const;
    case "pdfImageCount":
      return {
        type: "answer",
        id,
        verb: call.verb,
        value: verbs.pdfImageCount(...call.args)
      } as const;
    case "pdfMetadata":
      return {
        type: "answer",
        id,
        verb: call.verb,
        value: verbs.pdfMetadata(...call.args)
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
