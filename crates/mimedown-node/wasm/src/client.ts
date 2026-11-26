import type {
  BindingArgs,
  BindingRT,
  PageVerbs,
  Target,
  Wire,
  WorkerBindingKeys
} from "./protocol.ts";

export type {
  AsyncBindingKeys,
  BindingArgs,
  BindingKeys,
  BindingRT,
  PageVerbs,
  SyncBindingKeys,
  Target,
  Wire,
  WorkerBindingKeys
} from "./protocol.ts";

/** The two ends of one call's promise. */
interface Settle<T> {
  resolve: (value: T) => void;
  reject: (reason: Error) => void;
}

/** The calls still waiting, by the verb the worker runs, so that an answer's verb says what it settles. */
type Waiting = {
  [K in WorkerBindingKeys]: Map<number, Settle<BindingRT<K>>>;
};

/**
 * The sniffer in a worker of its own: the same verbs, each answered by a
 * promise, with the page left free while a file is read.
 *
 * The worker holds its own copy of the WebAssembly build and its memory.
 * `terminate()` ends it and gives all of that back, which is the only way
 * WebAssembly memory is ever given back; calls still waiting, and any made
 * after, are rejected.
 *
 * Its verbs are the build's own promise verbs, by the same names and with the
 * same answers, so that code written against the build runs on this.
 */
export class MimeWorker implements PageVerbs {
  /** Resolves with the build's name once the worker can answer; rejects if it cannot start. */
  public readonly ready: Promise<Target>;
  private readonly worker: Worker;
  private readonly waiting: Waiting = {
    sniff: new Map<number, Settle<BindingRT<"sniff">>>(),
    sniffFromRanges: new Map<number, Settle<BindingRT<"sniffFromRanges">>>(),
    specs: new Map<number, Settle<BindingRT<"specs">>>(),
    specsFromRanges: new Map<number, Settle<BindingRT<"specsFromRanges">>>(),
    waveform: new Map<number, Settle<BindingRT<"waveform">>>(),
    poster: new Map<number, Settle<BindingRT<"poster">>>(),
    keyframes: new Map<number, Settle<BindingRT<"keyframes">>>(),
    mediaTypesOf: new Map<number, Settle<BindingRT<"mediaTypesOf">>>(),
    mediaTypeOfPath: new Map<number, Settle<BindingRT<"mediaTypeOfPath">>>(),
    extensionsOf: new Map<number, Settle<BindingRT<"extensionsOf">>>(),
    builtWith: new Map<number, Settle<BindingRT<"builtWith">>>()
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
  // what the build gives, and the declarations run to six hundred lines.
  public sniffAsync(
    ...[data, options]: BindingArgs<"sniffAsync">
  ): Promise<BindingRT<"sniffAsync">> {
    return this.ask(this.waiting.sniff, (id) => ({
      type: "call",
      id,
      verb: "sniff",
      args: [this.own(data), options]
    }));
  }

  /**
   * From pieces of a file: its head, or its head and whatever an earlier
   * answer's `needs` asked for. Only the pieces go to the worker, so a large
   * file is named without ever being read whole.
   */
  public sniffFromRangesAsync(
    ...[pieces, size, options]: BindingArgs<"sniffFromRangesAsync">
  ): Promise<BindingRT<"sniffFromRangesAsync">> {
    return this.ask(this.waiting.sniffFromRanges, (id) => ({
      type: "call",
      id,
      verb: "sniffFromRanges",
      args: [this.each(pieces), size, options]
    }));
  }

  public specsAsync(
    ...[data, options]: BindingArgs<"specsAsync">
  ): Promise<BindingRT<"specsAsync">> {
    return this.ask(this.waiting.specs, (id) => ({
      type: "call",
      id,
      verb: "specs",
      args: [this.own(data), options]
    }));
  }

  public specsFromRangesAsync(
    ...[pieces, size, options]: BindingArgs<"specsFromRangesAsync">
  ): Promise<BindingRT<"specsFromRangesAsync">> {
    return this.ask(this.waiting.specsFromRanges, (id) => ({
      type: "call",
      id,
      verb: "specsFromRanges",
      args: [this.each(pieces), size, options]
    }));
  }

  public waveformAsync(
    ...[data, options]: BindingArgs<"waveformAsync">
  ): Promise<BindingRT<"waveformAsync">> {
    return this.ask(this.waiting.waveform, (id) => ({
      type: "call",
      id,
      verb: "waveform",
      args: [this.own(data), options]
    }));
  }

  public posterAsync(
    ...[data, options]: BindingArgs<"posterAsync">
  ): Promise<BindingRT<"posterAsync">> {
    return this.ask(this.waiting.poster, (id) => ({
      type: "call",
      id,
      verb: "poster",
      args: [this.own(data), options]
    }));
  }

  public keyframesAsync(
    ...[data]: BindingArgs<"keyframesAsync">
  ): Promise<BindingRT<"keyframesAsync">> {
    return this.ask(this.waiting.keyframes, (id) => ({
      type: "call",
      id,
      verb: "keyframes",
      args: [this.own(data)]
    }));
  }

  // The tables read no file. They are here so that a page that has the worker needs no
  // second copy of the build on its own thread to ask them.
  public mediaTypesOfAsync(
    ...args: BindingArgs<"mediaTypesOfAsync">
  ): Promise<BindingRT<"mediaTypesOfAsync">> {
    return this.ask(this.waiting.mediaTypesOf, (id) => ({
      type: "call",
      id,
      verb: "mediaTypesOf",
      args
    }));
  }

  public mediaTypeOfPathAsync(
    ...args: BindingArgs<"mediaTypeOfPathAsync">
  ): Promise<BindingRT<"mediaTypeOfPathAsync">> {
    return this.ask(this.waiting.mediaTypeOfPath, (id) => ({
      type: "call",
      id,
      verb: "mediaTypeOfPath",
      args
    }));
  }

  public extensionsOfAsync(
    ...args: BindingArgs<"extensionsOfAsync">
  ): Promise<BindingRT<"extensionsOfAsync">> {
    return this.ask(this.waiting.extensionsOf, (id) => ({
      type: "call",
      id,
      verb: "extensionsOf",
      args
    }));
  }

  public builtWithAsync(
    ...args: BindingArgs<"builtWithAsync">
  ): Promise<BindingRT<"builtWithAsync">> {
    return this.ask(this.waiting.builtWith, (id) => ({
      type: "call",
      id,
      verb: "builtWith",
      args
    }));
  }

  /** Ends the worker and gives back its memory. Calls still waiting are rejected. */
  public terminate() {
    this.worker.terminate();
    this.end(new Error("the worker was ended"));
  }

  /**
   * Bytes as they are sent: their own, and no more. A message carries the
   * whole of the buffer a view looks onto, so a view of a part of a larger
   * buffer (the head of a file held whole, say) is copied out first, and the
   * rest of that buffer stays where it is.
   */
  private own(bytes: Uint8Array) {
    return bytes.byteLength === bytes.buffer.byteLength ? bytes : bytes.slice();
  }

  /** Each piece of a file, with its bytes as they are sent. */
  private each(pieces: BindingArgs<"specsFromRangesAsync">[0]) {
    return pieces.map(({ offset, bytes }) => ({
      offset,
      bytes: this.own(bytes)
    }));
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
      case "sniff":
        return this.take(this.waiting.sniff, data.id)?.resolve(data.value);
      case "sniffFromRanges":
        return this.take(this.waiting.sniffFromRanges, data.id)?.resolve(
          data.value
        );
      case "specs":
        return this.take(this.waiting.specs, data.id)?.resolve(data.value);
      case "specsFromRanges":
        return this.take(this.waiting.specsFromRanges, data.id)?.resolve(
          data.value
        );
      case "waveform":
        return this.take(this.waiting.waveform, data.id)?.resolve(data.value);
      case "poster":
        return this.take(this.waiting.poster, data.id)?.resolve(data.value);
      case "keyframes":
        return this.take(this.waiting.keyframes, data.id)?.resolve(data.value);
      case "mediaTypesOf":
        return this.take(this.waiting.mediaTypesOf, data.id)?.resolve(
          data.value
        );
      case "mediaTypeOfPath":
        return this.take(this.waiting.mediaTypeOfPath, data.id)?.resolve(
          data.value
        );
      case "extensionsOf":
        return this.take(this.waiting.extensionsOf, data.id)?.resolve(
          data.value
        );
      case "builtWith":
        return this.take(this.waiting.builtWith, data.id)?.resolve(data.value);
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
