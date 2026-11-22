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
export class RtfWorker implements PageVerbs {
  /** Resolves with the build's name once the worker can answer; rejects if it cannot start. */
  public readonly ready: Promise<Target>;
  private readonly worker: Worker;
  private readonly waiting: Waiting = {
    rtfText: new Map<number, Settle<BindingRT<"rtfText">>>(),
    rtfLayout: new Map<number, Settle<BindingRT<"rtfLayout">>>(),
    rtfInvoice: new Map<number, Settle<BindingRT<"rtfInvoice">>>(),
    rtfEmbedded: new Map<number, Settle<BindingRT<"rtfEmbedded">>>(),
    rtfPictures: new Map<number, Settle<BindingRT<"rtfPictures">>>(),
    rtfDocument: new Map<number, Settle<BindingRT<"rtfDocument">>>()
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
  public rtfTextAsync(
    ...args: BindingArgs<"rtfTextAsync">
  ): Promise<BindingRT<"rtfTextAsync">> {
    return this.ask(this.waiting.rtfText, (id) => ({
      type: "call",
      id,
      verb: "rtfText",
      args
    }));
  }

  public rtfLayoutAsync(
    ...args: BindingArgs<"rtfLayoutAsync">
  ): Promise<BindingRT<"rtfLayoutAsync">> {
    return this.ask(this.waiting.rtfLayout, (id) => ({
      type: "call",
      id,
      verb: "rtfLayout",
      args
    }));
  }

  public rtfInvoiceAsync(
    ...args: BindingArgs<"rtfInvoiceAsync">
  ): Promise<BindingRT<"rtfInvoiceAsync">> {
    return this.ask(this.waiting.rtfInvoice, (id) => ({
      type: "call",
      id,
      verb: "rtfInvoice",
      args
    }));
  }

  public rtfEmbeddedAsync(
    ...args: BindingArgs<"rtfEmbeddedAsync">
  ): Promise<BindingRT<"rtfEmbeddedAsync">> {
    return this.ask(this.waiting.rtfEmbedded, (id) => ({
      type: "call",
      id,
      verb: "rtfEmbedded",
      args
    }));
  }

  public rtfPicturesAsync(
    ...args: BindingArgs<"rtfPicturesAsync">
  ): Promise<BindingRT<"rtfPicturesAsync">> {
    return this.ask(this.waiting.rtfPictures, (id) => ({
      type: "call",
      id,
      verb: "rtfPictures",
      args
    }));
  }

  public rtfDocumentAsync(
    ...args: BindingArgs<"rtfDocumentAsync">
  ): Promise<BindingRT<"rtfDocumentAsync">> {
    return this.ask(this.waiting.rtfDocument, (id) => ({
      type: "call",
      id,
      verb: "rtfDocument",
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
      case "rtfText":
        return this.take(this.waiting.rtfText, data.id)?.resolve(data.value);
      case "rtfLayout":
        return this.take(this.waiting.rtfLayout, data.id)?.resolve(data.value);
      case "rtfInvoice":
        return this.take(this.waiting.rtfInvoice, data.id)?.resolve(data.value);
      case "rtfEmbedded":
        return this.take(this.waiting.rtfEmbedded, data.id)?.resolve(
          data.value
        );
      case "rtfPictures":
        return this.take(this.waiting.rtfPictures, data.id)?.resolve(
          data.value
        );
      case "rtfDocument":
        return this.take(this.waiting.rtfDocument, data.id)?.resolve(
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
