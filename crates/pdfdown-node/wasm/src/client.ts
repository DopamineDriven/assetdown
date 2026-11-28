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
export class PdfWorker implements PageVerbs {
  /** Resolves with the build's name once the worker can answer; rejects if it cannot start. */
  public readonly ready: Promise<Target>;
  private readonly worker: Worker;
  private readonly waiting: Waiting = {
    attachments: new Map<number, Settle<BindingRT<"attachments">>>(),
    capabilities: new Map<number, Settle<BindingRT<"capabilities">>>(),
    extractAnnotationsPerPage: new Map<
      number,
      Settle<BindingRT<"extractAnnotationsPerPage">>
    >(),
    extractFormFieldsPerPage: new Map<
      number,
      Settle<BindingRT<"extractFormFieldsPerPage">>
    >(),
    extractImagesPerPage: new Map<
      number,
      Settle<BindingRT<"extractImagesPerPage">>
    >(),
    extractInvoice: new Map<number, Settle<BindingRT<"extractInvoice">>>(),
    extractStructuredTextPerPage: new Map<
      number,
      Settle<BindingRT<"extractStructuredTextPerPage">>
    >(),
    extractTextPerPage: new Map<
      number,
      Settle<BindingRT<"extractTextPerPage">>
    >(),
    invoiceFromLayout: new Map<
      number,
      Settle<BindingRT<"invoiceFromLayout">>
    >(),
    layoutPerPage: new Map<number, Settle<BindingRT<"layoutPerPage">>>(),
    pdfDocument: new Map<number, Settle<BindingRT<"pdfDocument">>>(),
    pdfImageCount: new Map<number, Settle<BindingRT<"pdfImageCount">>>(),
    pdfMetadata: new Map<number, Settle<BindingRT<"pdfMetadata">>>()
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
  public attachmentsAsync(
    ...args: BindingArgs<"attachmentsAsync">
  ): Promise<BindingRT<"attachmentsAsync">> {
    return this.ask(this.waiting.attachments, (id) => ({
      type: "call",
      id,
      verb: "attachments",
      args
    }));
  }

  public capabilitiesAsync(
    ...args: BindingArgs<"capabilitiesAsync">
  ): Promise<BindingRT<"capabilitiesAsync">> {
    return this.ask(this.waiting.capabilities, (id) => ({
      type: "call",
      id,
      verb: "capabilities",
      args
    }));
  }

  public extractAnnotationsPerPageAsync(
    ...args: BindingArgs<"extractAnnotationsPerPageAsync">
  ): Promise<BindingRT<"extractAnnotationsPerPageAsync">> {
    return this.ask(this.waiting.extractAnnotationsPerPage, (id) => ({
      type: "call",
      id,
      verb: "extractAnnotationsPerPage",
      args
    }));
  }

  public extractFormFieldsPerPageAsync(
    ...args: BindingArgs<"extractFormFieldsPerPageAsync">
  ): Promise<BindingRT<"extractFormFieldsPerPageAsync">> {
    return this.ask(this.waiting.extractFormFieldsPerPage, (id) => ({
      type: "call",
      id,
      verb: "extractFormFieldsPerPage",
      args
    }));
  }

  public extractImagesPerPageAsync(
    ...args: BindingArgs<"extractImagesPerPageAsync">
  ): Promise<BindingRT<"extractImagesPerPageAsync">> {
    return this.ask(this.waiting.extractImagesPerPage, (id) => ({
      type: "call",
      id,
      verb: "extractImagesPerPage",
      args
    }));
  }

  public extractInvoiceAsync(
    ...args: BindingArgs<"extractInvoiceAsync">
  ): Promise<BindingRT<"extractInvoiceAsync">> {
    return this.ask(this.waiting.extractInvoice, (id) => ({
      type: "call",
      id,
      verb: "extractInvoice",
      args
    }));
  }

  public extractStructuredTextPerPageAsync(
    ...args: BindingArgs<"extractStructuredTextPerPageAsync">
  ): Promise<BindingRT<"extractStructuredTextPerPageAsync">> {
    return this.ask(this.waiting.extractStructuredTextPerPage, (id) => ({
      type: "call",
      id,
      verb: "extractStructuredTextPerPage",
      args
    }));
  }

  public extractTextPerPageAsync(
    ...args: BindingArgs<"extractTextPerPageAsync">
  ): Promise<BindingRT<"extractTextPerPageAsync">> {
    return this.ask(this.waiting.extractTextPerPage, (id) => ({
      type: "call",
      id,
      verb: "extractTextPerPage",
      args
    }));
  }

  public invoiceFromLayoutAsync(
    ...args: BindingArgs<"invoiceFromLayoutAsync">
  ): Promise<BindingRT<"invoiceFromLayoutAsync">> {
    return this.ask(this.waiting.invoiceFromLayout, (id) => ({
      type: "call",
      id,
      verb: "invoiceFromLayout",
      args
    }));
  }

  public layoutPerPageAsync(
    ...args: BindingArgs<"layoutPerPageAsync">
  ): Promise<BindingRT<"layoutPerPageAsync">> {
    return this.ask(this.waiting.layoutPerPage, (id) => ({
      type: "call",
      id,
      verb: "layoutPerPage",
      args
    }));
  }

  public pdfDocumentAsync(
    ...args: BindingArgs<"pdfDocumentAsync">
  ): Promise<BindingRT<"pdfDocumentAsync">> {
    return this.ask(this.waiting.pdfDocument, (id) => ({
      type: "call",
      id,
      verb: "pdfDocument",
      args
    }));
  }

  public pdfImageCountAsync(
    ...args: BindingArgs<"pdfImageCountAsync">
  ): Promise<BindingRT<"pdfImageCountAsync">> {
    return this.ask(this.waiting.pdfImageCount, (id) => ({
      type: "call",
      id,
      verb: "pdfImageCount",
      args
    }));
  }

  public pdfMetadataAsync(
    ...args: BindingArgs<"pdfMetadataAsync">
  ): Promise<BindingRT<"pdfMetadataAsync">> {
    return this.ask(this.waiting.pdfMetadata, (id) => ({
      type: "call",
      id,
      verb: "pdfMetadata",
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
      case "attachments":
        return this.take(this.waiting.attachments, data.id)?.resolve(
          data.value
        );
      case "capabilities":
        return this.take(this.waiting.capabilities, data.id)?.resolve(
          data.value
        );
      case "extractAnnotationsPerPage":
        return this.take(
          this.waiting.extractAnnotationsPerPage,
          data.id
        )?.resolve(data.value);
      case "extractFormFieldsPerPage":
        return this.take(
          this.waiting.extractFormFieldsPerPage,
          data.id
        )?.resolve(data.value);
      case "extractImagesPerPage":
        return this.take(this.waiting.extractImagesPerPage, data.id)?.resolve(
          data.value
        );
      case "extractInvoice":
        return this.take(this.waiting.extractInvoice, data.id)?.resolve(
          data.value
        );
      case "extractStructuredTextPerPage":
        return this.take(
          this.waiting.extractStructuredTextPerPage,
          data.id
        )?.resolve(data.value);
      case "extractTextPerPage":
        return this.take(this.waiting.extractTextPerPage, data.id)?.resolve(
          data.value
        );
      case "invoiceFromLayout":
        return this.take(this.waiting.invoiceFromLayout, data.id)?.resolve(
          data.value
        );
      case "layoutPerPage":
        return this.take(this.waiting.layoutPerPage, data.id)?.resolve(
          data.value
        );
      case "pdfDocument":
        return this.take(this.waiting.pdfDocument, data.id)?.resolve(
          data.value
        );
      case "pdfImageCount":
        return this.take(this.waiting.pdfImageCount, data.id)?.resolve(
          data.value
        );
      case "pdfMetadata":
        return this.take(this.waiting.pdfMetadata, data.id)?.resolve(
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
