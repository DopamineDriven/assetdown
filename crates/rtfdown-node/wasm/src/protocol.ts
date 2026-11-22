/** The build the worker runs: the one without threads, which a plain page can load. */
type Binding = typeof import("@d0paminedriven/rtfdown-wasm/threadless");

type Rm<T, P extends keyof T = keyof T> = {
  [S in keyof T as Exclude<S, P>]: T[S];
};

/**
 * A value as it reaches the page. The build calls its bytes a `Buffer`, which
 * a page does not have: they arrive as a plain `Uint8Array`.
 */
type OnThePage<T> = T extends Uint8Array
  ? Uint8Array
  : T extends readonly (infer U)[]
    ? OnThePage<U>[]
    : T extends object
      ? { [K in keyof T]: OnThePage<T[K]> }
      : T;

/** A verb's arguments, each as the page gives it. A tuple stays the tuple it is. */
type EachOnThePage<T extends readonly unknown[]> = {
  [I in keyof T]: OnThePage<T[I]>;
};

/** The build's verbs: its functions, without its class and its name for itself. */
type BindingNarrowed = Rm<Binding, "RtfDown" | "__napiBindingTarget">;

export type BindingKeys = keyof BindingNarrowed;

type AsyncBindingKeysWorkup<T> = T extends `${string}Async` ? T : never;

/** The verbs that answer with a promise: the ones a page calls on the worker. */
export type AsyncBindingKeys = AsyncBindingKeysWorkup<BindingKeys>;

/** The verbs that answer at once: the ones the worker runs. */
export type SyncBindingKeys = Exclude<BindingKeys, AsyncBindingKeys>;

/**
 * What a verb gives, as it reaches the page. For a verb that gives a promise
 * this is what the promise holds: `Awaited` leaves anything else as it is, so
 * a verb and its `Async` twin give the same.
 */
export type BindingRT<K extends BindingKeys> = OnThePage<
  Awaited<ReturnType<BindingNarrowed[K]>>
>;

/** What a verb takes, as the page gives it. */
export type BindingArgs<K extends BindingKeys> = EachOnThePage<
  Parameters<BindingNarrowed[K]>
>;

/**
 * The build's own promise verbs, as a page has them. The worker's client is
 * one of these, so code written against the build's promises runs on it.
 */
export type PageVerbs = {
  [K in AsyncBindingKeys]: (...args: BindingArgs<K>) => Promise<BindingRT<K>>;
};

/** Which build answered. */
export type Target = Binding["__napiBindingTarget"];

export namespace Wire {
  export namespace PageToWorker {
    /** One call for each verb the worker runs, so that the verb says what `args` are. */
    export type Call = {
      [K in SyncBindingKeys]: {
        type: "call";
        /** Pairs the answer with the call: the worker gives it back unchanged. */
        id: number;
        verb: K;
        args: BindingArgs<K>;
      };
    }[SyncBindingKeys];
  }

  export namespace WorkerToPage {
    /** The build is loaded, and calls will be answered. */
    export interface Ready {
      type: "ready";
      target: Target;
    }
    /** The build did not load: no call will be answered. */
    export interface Broken {
      type: "broken";
      message: string;
    }
    /** One answer for each verb, so that the verb says what `value` is. */
    export type Answer = {
      [K in SyncBindingKeys]: {
        type: "answer";
        id: number;
        verb: K;
        value: BindingRT<K>;
      };
    }[SyncBindingKeys];
    export interface Failed {
      type: "failed";
      id: number;
      message: string;
    }
  }

  export type PageToWorker = PageToWorker.Call;

  export type WorkerToPage =
    | WorkerToPage.Ready
    | WorkerToPage.Broken
    | WorkerToPage.Answer
    | WorkerToPage.Failed;
}
