/** The build the worker runs: the one without threads, which a plain page can load. */
type Binding = typeof import("@d0paminedriven/mimedown-wasm/threadless");

type Rm<T, P extends keyof T = keyof T> = {
  [S in keyof T as Exclude<S, P>]: T[S];
};

/**
 * A value as it reaches the page. Bytes cross to the page as a plain
 * `Uint8Array`, whatever the build calls them.
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
type BindingNarrowed = Rm<Binding, "MimeDown" | "__napiBindingTarget">;

export type BindingKeys = keyof BindingNarrowed;

type AsyncBindingKeysWorkup<T> = T extends `${string}Async` ? T : never;

/** The verbs that answer with a promise: the ones a page calls on the worker. */
export type AsyncBindingKeys = AsyncBindingKeysWorkup<BindingKeys>;

/** The verbs that answer at once. */
export type SyncBindingKeys = Exclude<BindingKeys, AsyncBindingKeys>;

type WorkerBindingKeysWorkup<T> = T extends `${infer K}Async`
  ? K extends SyncBindingKeys
    ? K
    : never
  : never;

/**
 * The verbs the worker runs: each one that answers at once and has a twin
 * that answers with a promise, which is the one a page calls. Every verb the
 * build has as a function has such a twin, the tables included. One added
 * without it would not be among these: the build would have no promise of
 * it for the worker to keep.
 */
export type WorkerBindingKeys = WorkerBindingKeysWorkup<AsyncBindingKeys>;

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
      [K in WorkerBindingKeys]: {
        type: "call";
        /** Pairs the answer with the call: the worker gives it back unchanged. */
        id: number;
        verb: K;
        args: BindingArgs<K>;
      };
    }[WorkerBindingKeys];
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
      [K in WorkerBindingKeys]: {
        type: "answer";
        id: number;
        verb: K;
        value: BindingRT<K>;
      };
    }[WorkerBindingKeys];
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
