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
export class DocWorker implements PageVerbs {
  /** Resolves with the build's name once the worker can answer; rejects if it cannot start. */
  public readonly ready: Promise<Target>;
  private readonly worker: Worker;
  private readonly waiting: Waiting = {
    docText: new Map<number, Settle<BindingRT<"docText">>>(),
    docLayout: new Map<number, Settle<BindingRT<"docLayout">>>(),
    docInvoice: new Map<number, Settle<BindingRT<"docInvoice">>>(),
    docEmbedded: new Map<number, Settle<BindingRT<"docEmbedded">>>(),
    docPictures: new Map<number, Settle<BindingRT<"docPictures">>>(),
    docDocument: new Map<number, Settle<BindingRT<"docDocument">>>()
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
  public docTextAsync(
    ...args: BindingArgs<"docTextAsync">
  ): Promise<BindingRT<"docTextAsync">> {
    return this.ask(this.waiting.docText, (id) => ({
      type: "call",
      id,
      verb: "docText",
      args
    }));
  }

  public docLayoutAsync(
    ...args: BindingArgs<"docLayoutAsync">
  ): Promise<BindingRT<"docLayoutAsync">> {
    return this.ask(this.waiting.docLayout, (id) => ({
      type: "call",
      id,
      verb: "docLayout",
      args
    }));
  }

  public docInvoiceAsync(
    ...args: BindingArgs<"docInvoiceAsync">
  ): Promise<BindingRT<"docInvoiceAsync">> {
    return this.ask(this.waiting.docInvoice, (id) => ({
      type: "call",
      id,
      verb: "docInvoice",
      args
    }));
  }

  public docEmbeddedAsync(
    ...args: BindingArgs<"docEmbeddedAsync">
  ): Promise<BindingRT<"docEmbeddedAsync">> {
    return this.ask(this.waiting.docEmbedded, (id) => ({
      type: "call",
      id,
      verb: "docEmbedded",
      args
    }));
  }

  public docPicturesAsync(
    ...args: BindingArgs<"docPicturesAsync">
  ): Promise<BindingRT<"docPicturesAsync">> {
    return this.ask(this.waiting.docPictures, (id) => ({
      type: "call",
      id,
      verb: "docPictures",
      args
    }));
  }

  public docDocumentAsync(
    ...args: BindingArgs<"docDocumentAsync">
  ): Promise<BindingRT<"docDocumentAsync">> {
    return this.ask(this.waiting.docDocument, (id) => ({
      type: "call",
      id,
      verb: "docDocument",
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
      case "docText":
        return this.take(this.waiting.docText, data.id)?.resolve(data.value);
      case "docLayout":
        return this.take(this.waiting.docLayout, data.id)?.resolve(data.value);
      case "docInvoice":
        return this.take(this.waiting.docInvoice, data.id)?.resolve(data.value);
      case "docEmbedded":
        return this.take(this.waiting.docEmbedded, data.id)?.resolve(
          data.value
        );
      case "docPictures":
        return this.take(this.waiting.docPictures, data.id)?.resolve(
          data.value
        );
      case "docDocument":
        return this.take(this.waiting.docDocument, data.id)?.resolve(
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
