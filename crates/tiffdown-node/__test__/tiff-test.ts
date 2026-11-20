import assert from "node:assert/strict";

import { WorkupService } from "@/__test__/workup.ts";

/** One tagged value of a directory, as the format writes it. */
interface Entry {
  readonly tag: number;
  /** The number the format gives the value's type: 2 text, 3 short, 4 long, 5 a fraction. */
  readonly kind: 2 | 3 | 4 | 5;
  readonly count: number;
  readonly bytes: Buffer;
}

/** A page of gray, and what its directory says of it. */
interface GrayPage {
  readonly width: number;
  readonly height: number;
  /** One byte to a pixel, a row at a time. Left out, the page is white. */
  readonly gray?: Buffer;
  /** Pixels to the inch, across and down. */
  readonly dpi?: { readonly x: number; readonly y: number };
  readonly orientation?: number;
  /** The number of the page's coding; 1, kept as it is, when left out. */
  readonly compression?: number;
  readonly software?: string;
  readonly dateTime?: string;
}

/**
 * The repository holds no TIFF, so every file here is written out by hand: a header, then a
 * chain of directories, each followed by the values too long for its entries and by its pixels.
 */
export class TiffTest extends WorkupService {
  private short(tag: number, value: number) {
    const bytes = Buffer.alloc(2);
    bytes.writeUInt16LE(value);
    return { tag, kind: 3, count: 1, bytes } as const satisfies Entry;
  }

  private long(tag: number, value: number) {
    const bytes = Buffer.alloc(4);
    bytes.writeUInt32LE(value);
    return { tag, kind: 4, count: 1, bytes } as const satisfies Entry;
  }

  private fraction(tag: number, over: number) {
    const bytes = Buffer.alloc(8);
    bytes.writeUInt32LE(over, 0);
    bytes.writeUInt32LE(1, 4);
    return { tag, kind: 5, count: 1, bytes } as const satisfies Entry;
  }

  private text(tag: number, value: string) {
    const bytes = Buffer.from(`${value}\0`, "latin1");
    return {
      tag,
      kind: 2,
      count: bytes.length,
      bytes
    } as const satisfies Entry;
  }

  /** A directory placed at `at` with what it points to after it, and where that ends. */
  private directory(page: GrayPage, at: number, last: boolean, back: boolean) {
    const gray = page.gray ?? Buffer.alloc(page.width * page.height, 255);
    const said = [
      this.long(256, page.width),
      this.long(257, page.height),
      this.short(258, 8),
      this.short(259, page.compression ?? 1),
      this.short(262, 1),
      this.short(277, 1),
      this.long(278, page.height),
      this.long(279, gray.length),
      ...(page.orientation === undefined
        ? []
        : [this.short(274, page.orientation)]),
      ...(page.dpi === undefined
        ? []
        : [
            this.fraction(282, page.dpi.x),
            this.fraction(283, page.dpi.y),
            this.short(296, 2)
          ]),
      ...(page.software === undefined ? [] : [this.text(305, page.software)]),
      ...(page.dateTime === undefined ? [] : [this.text(306, page.dateTime)])
    ];
    const even = (length: number) => length + (length % 2);
    const head = Buffer.alloc(2 + 12 * (said.length + 1) + 4);
    const long = said
      .filter((entry) => entry.bytes.length > 4)
      .reduce((sum, entry) => sum + even(entry.bytes.length), 0);
    const pixelsAt = at + head.length + long;
    const end = even(pixelsAt + gray.length);
    const entries = [...said, this.long(273, pixelsAt)].toSorted(
      (a, b) => a.tag - b.tag
    );
    const after = Array.of<Buffer>();
    let valueAt = at + head.length;
    head.writeUInt16LE(entries.length, 0);
    entries.forEach((entry, index) => {
      const offset = 2 + 12 * index;
      head.writeUInt16LE(entry.tag, offset);
      head.writeUInt16LE(entry.kind, offset + 2);
      head.writeUInt32LE(entry.count, offset + 4);
      if (entry.bytes.length <= 4) {
        entry.bytes.copy(head, offset + 8);
      } else {
        const padded = Buffer.alloc(even(entry.bytes.length));
        entry.bytes.copy(padded);
        head.writeUInt32LE(valueAt, offset + 8);
        after.push(padded);
        valueAt += padded.length;
      }
    });
    // The last directory points nowhere, or back at the first when the chain is to loop.
    head.writeUInt32LE(last ? (back ? 8 : 0) : end, head.length - 4);
    const pixels = Buffer.alloc(end - pixelsAt);
    gray.copy(pixels);
    return { bytes: Buffer.concat([head, ...after, pixels]), end };
  }

  /** A TIFF of these pages. With `loop`, its last directory leads back to its first. */
  public tiff(pages: readonly GrayPage[], loop = false) {
    const header = Buffer.from([0x49, 0x49, 42, 0, 8, 0, 0, 0]);
    const parts = Array.of<Buffer>(header);
    let at = header.length;
    pages.forEach((page, index) => {
      const last = index === pages.length - 1;
      const directory = this.directory(page, at, last, loop);
      parts.push(directory.bytes);
      at = directory.end;
    });
    return Buffer.concat(parts);
  }

  /** A letter page at 20 pixels to the inch, and a smaller page after it. */
  public get twoPages() {
    return this.tiff([
      {
        width: 170,
        height: 220,
        dpi: { x: 20, y: 20 },
        software: "a scanner",
        dateTime: "2015:03:04 10:22:33"
      },
      { width: 4, height: 2 }
    ]);
  }

  /** What a PNG's header says: its size, and how its samples are kept. */
  public pngHeader(png: Buffer) {
    assert.equal(png.subarray(0, 8).toString("hex"), "89504e470d0a1a0a");
    assert.equal(png.subarray(12, 16).toString("latin1"), "IHDR");
    return {
      width: png.readUInt32BE(16),
      height: png.readUInt32BE(20),
      bitDepth: png.readUInt8(24),
      // 0 is gray.
      colorType: png.readUInt8(25)
    } as const;
  }
}
