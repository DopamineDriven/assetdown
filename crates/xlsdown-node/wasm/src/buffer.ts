/**
 * A page has no `Buffer`. The runtime under the build asks for one whenever
 * the build gives bytes back, and throws when there is none. This stands in
 * for it with the three things that runtime asks of `Buffer`: bytes are a
 * `Uint8Array` in the worker, and cross to the page as one.
 *
 * It is imported before the build, so that it is in place when the runtime
 * looks, and it leaves a real `Buffer` alone where there is one.
 */
const standIn = Object.assign(
  function Buffer() {
    throw new TypeError("a Buffer is made with Buffer.from or Buffer.alloc");
  },
  {
    from: (source: ArrayBuffer) => new Uint8Array(source),
    alloc: (size: number) => new Uint8Array(size),
    isBuffer: (value: unknown) => value instanceof Uint8Array
  }
);

if (!("Buffer" in globalThis)) {
  Object.defineProperty(globalThis, "Buffer", {
    value: standIn,
    configurable: true,
    writable: true
  });
}
