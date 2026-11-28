import type {
  BindingArgs,
  BindingRT,
  PageVerbs,
  SyncBindingKeys,
  Target,
  Wire
} from "./protocol.ts";

export type {
  AsyncBindingKeys,
  BindingArgs,
  BindingKeys,
  BindingRT,
  PageVerbs,
  SyncBindingKeys,
  Target,
  Wire
} from "./protocol.ts";

/** The two ends of one call's promise. */
interface Settle<T> {
  resolve: (value: T) => void;
  reject: (reason: Error) => void;
}

/** The calls still waiting, by the verb the worker runs, so that an answer's verb says what it settles. */
type Waiting = { [K in SyncBindingKeys]: Map<number, Settle<BindingRT<K>>> };

/**
 * The parser in a worker of its own: the same verbs, each answered by a
 * promise, with the page left free while a document is read.
 *
 * The worker holds its own copy of the WebAssembly build and its memory.
 * `terminate()` ends it and gives all of that back, which is the only way
 * WebAssembly memory is ever given back; calls still waiting, and any made
 * after, are rejected.
 *
 * Its verbs are the build's own promise verbs, by the same names and with the
 * same answers, so that code written against the build runs on this.
 */
export class DocxWorker implements PageVerbs {
  /** Resolves with the build's name once the worker can answer; rejects if it cannot start. */
  public readonly ready: Promise<Target>;
  private readonly worker: Worker;
  private readonly waiting: Waiting = {
    docxAnnotations: new Map<number, Settle<BindingRT<"docxAnnotations">>>(),
    docxBlocks: new Map<number, Settle<BindingRT<"docxBlocks">>>(),
    docxDocument: new Map<number, Settle<BindingRT<"docxDocument">>>(),
    docxFormFields: new Map<number, Settle<BindingRT<"docxFormFields">>>(),
    docxImages: new Map<number, Settle<BindingRT<"docxImages">>>(),
    docxInvoice: new Map<number, Settle<BindingRT<"docxInvoice">>>(),
    docxLayout: new Map<number, Settle<BindingRT<"docxLayout">>>(),
    docxMarkdown: new Map<number, Settle<BindingRT<"docxMarkdown">>>(),
    docxMetadata: new Map<number, Settle<BindingRT<"docxMetadata">>>(),
    docxNotes: new Map<number, Settle<BindingRT<"docxNotes">>>(),
    docxTables: new Map<number, Settle<BindingRT<"docxTables">>>(),
    docxText: new Map<number, Settle<BindingRT<"docxText">>>(),
    docxTextPerPage: new Map<number, Settle<BindingRT<"docxTextPerPage">>>()
  };
  private next = 0;
  private ended: Error | undefined;

  /**
   * `url` is where the worker's file is served from. It is found beside this
   * one unless told otherwise; an app that copies it elsewhere says where.
   */
  constructor(url: string | URL = new URL("./worker.js", import.meta.url)) {
    const starting = Promise.withResolvers<Target>();
    this.ready = starting.promise;
    this.worker = new Worker(url, { type: "module" });
    this.worker.addEventListener(
      "message",
      ({ data }: MessageEvent<Wire.WorkerToPage>) => {
        switch (data.type) {
          case "ready":
            starting.resolve(data.target);
            break;
          case "broken":
            this.end(new Error(data.message));
            starting.reject(new Error(data.message));
            break;
          case "answer":
            this.settle(data);
            break;
          case "failed":
            this.fail(data.id, new Error(data.message));
            break;
        }
      }
    );
    this.worker.addEventListener("error", (event) => {
      const error = new Error(event.message || "the worker could not start");
      this.end(error);
      starting.reject(error);
    });
  }

  // Each asks the worker for the verb of the same name without `Async`. The answers are
  // named by the verb, and not written out: written out, each is the whole shape of
  // what the build gives, and the declarations run to a thousand lines.
  public docxAnnotationsAsync(
    ...args: BindingArgs<"docxAnnotationsAsync">
  ): Promise<BindingRT<"docxAnnotationsAsync">> {
    return this.ask(this.waiting.docxAnnotations, (id) => ({
      type: "call",
      id,
      verb: "docxAnnotations",
      args
    }));
  }

  public docxBlocksAsync(
    ...args: BindingArgs<"docxBlocksAsync">
  ): Promise<BindingRT<"docxBlocksAsync">> {
    return this.ask(this.waiting.docxBlocks, (id) => ({
      type: "call",
      id,
      verb: "docxBlocks",
      args
    }));
  }

  public docxDocumentAsync(
    ...args: BindingArgs<"docxDocumentAsync">
  ): Promise<BindingRT<"docxDocumentAsync">> {
    return this.ask(this.waiting.docxDocument, (id) => ({
      type: "call",
      id,
      verb: "docxDocument",
      args
    }));
  }

  public docxFormFieldsAsync(
    ...args: BindingArgs<"docxFormFieldsAsync">
  ): Promise<BindingRT<"docxFormFieldsAsync">> {
    return this.ask(this.waiting.docxFormFields, (id) => ({
      type: "call",
      id,
      verb: "docxFormFields",
      args
    }));
  }

  public docxImagesAsync(
    ...args: BindingArgs<"docxImagesAsync">
  ): Promise<BindingRT<"docxImagesAsync">> {
    return this.ask(this.waiting.docxImages, (id) => ({
      type: "call",
      id,
      verb: "docxImages",
      args
    }));
  }

  public docxInvoiceAsync(
    ...args: BindingArgs<"docxInvoiceAsync">
  ): Promise<BindingRT<"docxInvoiceAsync">> {
    return this.ask(this.waiting.docxInvoice, (id) => ({
      type: "call",
      id,
      verb: "docxInvoice",
      args
    }));
  }

  public docxLayoutAsync(
    ...args: BindingArgs<"docxLayoutAsync">
  ): Promise<BindingRT<"docxLayoutAsync">> {
    return this.ask(this.waiting.docxLayout, (id) => ({
      type: "call",
      id,
      verb: "docxLayout",
      args
    }));
  }

  public docxMarkdownAsync(
    ...args: BindingArgs<"docxMarkdownAsync">
  ): Promise<BindingRT<"docxMarkdownAsync">> {
    return this.ask(this.waiting.docxMarkdown, (id) => ({
      type: "call",
      id,
      verb: "docxMarkdown",
      args
    }));
  }

  public docxMetadataAsync(
    ...args: BindingArgs<"docxMetadataAsync">
  ): Promise<BindingRT<"docxMetadataAsync">> {
    return this.ask(this.waiting.docxMetadata, (id) => ({
      type: "call",
      id,
      verb: "docxMetadata",
      args
    }));
  }

  public docxNotesAsync(
    ...args: BindingArgs<"docxNotesAsync">
  ): Promise<BindingRT<"docxNotesAsync">> {
    return this.ask(this.waiting.docxNotes, (id) => ({
      type: "call",
      id,
      verb: "docxNotes",
      args
    }));
  }

  public docxTablesAsync(
    ...args: BindingArgs<"docxTablesAsync">
  ): Promise<BindingRT<"docxTablesAsync">> {
    return this.ask(this.waiting.docxTables, (id) => ({
      type: "call",
      id,
      verb: "docxTables",
      args
    }));
  }

  public docxTextAsync(
    ...args: BindingArgs<"docxTextAsync">
  ): Promise<BindingRT<"docxTextAsync">> {
    return this.ask(this.waiting.docxText, (id) => ({
      type: "call",
      id,
      verb: "docxText",
      args
    }));
  }

  public docxTextPerPageAsync(
    ...args: BindingArgs<"docxTextPerPageAsync">
  ): Promise<BindingRT<"docxTextPerPageAsync">> {
    return this.ask(this.waiting.docxTextPerPage, (id) => ({
      type: "call",
      id,
      verb: "docxTextPerPage",
      args
    }));
  }

  /** Ends the worker and gives back its memory. Calls still waiting are rejected. */
  public terminate() {
    this.worker.terminate();
    this.end(new Error("the worker was ended"));
  }

  /** Sends one call, and gives the promise its answer settles. */
  private ask<T>(
    calls: Map<number, Settle<T>>,
    call: (id: number) => Wire.PageToWorker
  ) {
    const { promise, resolve, reject } = Promise.withResolvers<T>();
    if (this.ended) {
      reject(this.ended);
      return promise;
    }
    const id = this.next++;
    calls.set(id, { resolve, reject });
    // The arguments are copied to the worker: the caller keeps its own bytes.
    this.worker.postMessage(call(id));
    return promise;
  }

  private settle(data: Wire.WorkerToPage.Answer) {
    switch (data.verb) {
      case "docxAnnotations":
        return this.take(this.waiting.docxAnnotations, data.id)?.resolve(
          data.value
        );
      case "docxBlocks":
        return this.take(this.waiting.docxBlocks, data.id)?.resolve(data.value);
      case "docxDocument":
        return this.take(this.waiting.docxDocument, data.id)?.resolve(
          data.value
        );
      case "docxFormFields":
        return this.take(this.waiting.docxFormFields, data.id)?.resolve(
          data.value
        );
      case "docxImages":
        return this.take(this.waiting.docxImages, data.id)?.resolve(data.value);
      case "docxInvoice":
        return this.take(this.waiting.docxInvoice, data.id)?.resolve(
          data.value
        );
      case "docxLayout":
        return this.take(this.waiting.docxLayout, data.id)?.resolve(data.value);
      case "docxMarkdown":
        return this.take(this.waiting.docxMarkdown, data.id)?.resolve(
          data.value
        );
      case "docxMetadata":
        return this.take(this.waiting.docxMetadata, data.id)?.resolve(
          data.value
        );
      case "docxNotes":
        return this.take(this.waiting.docxNotes, data.id)?.resolve(data.value);
      case "docxTables":
        return this.take(this.waiting.docxTables, data.id)?.resolve(data.value);
      case "docxText":
        return this.take(this.waiting.docxText, data.id)?.resolve(data.value);
      case "docxTextPerPage":
        return this.take(this.waiting.docxTextPerPage, data.id)?.resolve(
          data.value
        );
    }
  }

  private take<T>(calls: Map<number, Settle<T>>, id: number) {
    const call = calls.get(id);
    calls.delete(id);
    return call;
  }

  private fail(id: number, error: Error) {
    for (const calls of Object.values(this.waiting)) {
      calls.get(id)?.reject(error);
      calls.delete(id);
    }
  }

  private end(error: Error) {
    this.ended = error;
    for (const calls of Object.values(this.waiting)) {
      for (const call of calls.values()) call.reject(error);
      calls.clear();
    }
  }
}
