import { crc32 } from "node:zlib";

import { WorkupService } from "@/__test__/workup.ts";
import type { Piece, Specs } from "@/index.js";

/** The verb that reads a file from pieces of it, from whichever build a test holds. */
type FromRanges = (pieces: Piece[], size?: number | null) => Specs;

/**
 * Reads the public fixtures, and builds small files in code where a test needs one that no
 * fixture is: a picture is a few dozen bytes when only its header is to be read.
 */
export class MimeTest extends WorkupService {
  /** The four Illustrator documents among the fixtures. Each is a PDF under another name. */
  public get illustrator() {
    return Object.keys(this.fixturePaths);
  }

  /** The start of a PNG of one pixel: its signature and its header chunk, checksum and all. */
  public get png() {
    return Buffer.from(
      "89504e470d0a1a0a0000000d49484452000000010000000108060000001f15c489",
      "hex"
    );
  }

  /** A WAVE file of half a second of silence: one channel, 16 bits, 8,000 samples a second. */
  public get wav() {
    return this.wavOf(0.5, 8000, 1);
  }

  /** A WAVE file of this many seconds of silence, 16 bits to a sample. */
  public wavOf(seconds: number, samplesPerSecond: number, channels: number) {
    const bytesPerSecond = samplesPerSecond * channels * 2;
    const sound = Math.round(seconds * bytesPerSecond);
    const file = Buffer.alloc(44 + sound);
    file.write("RIFF", 0, "latin1");
    file.writeUInt32LE(36 + sound, 4);
    file.write("WAVEfmt ", 8, "latin1");
    file.writeUInt32LE(16, 16);
    file.writeUInt16LE(1, 20);
    file.writeUInt16LE(channels, 22);
    file.writeUInt32LE(samplesPerSecond, 24);
    file.writeUInt32LE(bytesPerSecond, 28);
    file.writeUInt16LE(channels * 2, 32);
    file.writeUInt16LE(16, 34);
    file.write("data", 36, "latin1");
    file.writeUInt32LE(sound, 40);
    return file;
  }

  /**
   * What a fetcher ends with: the head of `file`, then each range the answer asks for, until
   * it asks for nothing. With it, how many ranges that took. `read` is the build's own verb.
   */
  public settle(read: FromRanges, file: Buffer, head: number) {
    const pieces = [{ offset: 0, bytes: file.subarray(0, head) }];
    let answer = read(pieces, file.byteLength);
    let fetches = 1;
    while (answer.needs !== null && fetches < 8) {
      const { offset, length } = answer.needs;
      pieces.push({ offset, bytes: file.subarray(offset, offset + length) });
      answer = read(pieces, file.byteLength);
      fetches += 1;
    }
    return { answer, fetches } as const;
  }

  private u16(value: number) {
    const bytes = Buffer.alloc(2);
    bytes.writeUInt16LE(value);
    return bytes;
  }

  private u32(value: number) {
    const bytes = Buffer.alloc(4);
    bytes.writeUInt32LE(value);
    return bytes;
  }

  private u32be(value: number) {
    const bytes = Buffer.alloc(4);
    bytes.writeUInt32BE(value);
    return bytes;
  }

  /** One chunk of a PNG: its length, its name, what it holds, and the checksum of the last two. */
  private pngChunk(kind: string, payload: Buffer) {
    const named = Buffer.concat([Buffer.from(kind, "latin1"), payload]);
    return Buffer.concat([
      this.u32be(payload.byteLength),
      named,
      this.u32be(crc32(named))
    ]);
  }

  /**
   * A PNG of one pixel. With `frames` it carries the chunk that makes it an animated one,
   * saying it has that many.
   */
  public pngOf(frames?: number) {
    const header = Buffer.concat([
      this.u32be(1),
      this.u32be(1),
      Buffer.from([8, 6, 0, 0, 0])
    ]);
    const animation =
      frames === undefined
        ? Array.of<Buffer>()
        : [
            this.pngChunk(
              "acTL",
              Buffer.concat([this.u32be(frames), this.u32be(0)])
            )
          ];
    return Buffer.concat([
      Buffer.from("89504e470d0a1a0a", "hex"),
      this.pngChunk("IHDR", header),
      ...animation,
      this.pngChunk("IDAT", Buffer.from([0x78, 0x9c, 0x63, 0, 0, 0, 1, 0, 1])),
      this.pngChunk("IEND", Buffer.alloc(0))
    ]);
  }

  /** A GIF of this many frames, each of one pixel and shown for a tenth of a second. */
  public gifOf(frames: number) {
    // The header, a screen of 16 by 16, and a palette of four colors shared by the frames.
    const start = Buffer.concat([
      Buffer.from("GIF89a", "latin1"),
      this.u16(16),
      this.u16(16),
      Buffer.from([0x81, 0, 0]),
      Buffer.alloc(12)
    ]);
    // How the frame is shown, where it sits, a code size, one run of pixels, and its end.
    const frame = Buffer.from([
      0x21, 0xf9, 4, 0, 10, 0, 0, 0, 0x2c, 0, 0, 0, 0, 1, 0, 1, 0, 0, 2, 2,
      0x4c, 0x01, 0
    ]);
    return Buffer.concat([
      start,
      ...Array.from({ length: frames }, () => frame),
      Buffer.from([0x3b])
    ]);
  }

  /** One chunk of a WebP: its name, its length, what it holds, and a byte to make it even. */
  private webpChunk(kind: string, payload: Buffer) {
    return Buffer.concat([
      Buffer.from(kind, "latin1"),
      this.u32(payload.byteLength),
      payload,
      Buffer.alloc(payload.byteLength % 2)
    ]);
  }

  /** A lossless picture's chunk, of 16 by 16: a signature byte, then the size in bits. */
  private get webpPicture() {
    return this.webpChunk(
      "VP8L",
      Buffer.concat([
        Buffer.from([0x2f]),
        this.u32(15 | (15 << 14)),
        Buffer.alloc(5)
      ])
    );
  }

  private webp(chunks: readonly Buffer[]) {
    const body = Buffer.concat(chunks);
    return Buffer.concat([
      Buffer.from("RIFF", "latin1"),
      this.u32(body.byteLength + 4),
      Buffer.from("WEBP", "latin1"),
      body
    ]);
  }

  /**
   * A WebP of 16 by 16. Without `frames` it is the one picture and nothing else; with it,
   * the extended form with its animation flag set and that many frames.
   */
  public webpOf(frames?: number) {
    if (frames === undefined) return this.webp([this.webpPicture]);
    const canvas = this.u32(15).subarray(0, 3);
    const frame = this.webpChunk(
      "ANMF",
      Buffer.concat([
        Buffer.alloc(12),
        this.u32(100).subarray(0, 3),
        Buffer.alloc(1),
        this.webpPicture
      ])
    );
    return this.webp([
      // What the file carries (an animation), and the canvas.
      this.webpChunk(
        "VP8X",
        Buffer.concat([Buffer.from([0x02, 0, 0, 0]), canvas, canvas])
      ),
      this.webpChunk("ANIM", Buffer.alloc(6)),
      ...Array.from({ length: frames }, () => frame)
    ]);
  }

  /** An icon that holds one picture at each of these sizes, as bitmaps. */
  public icoOf(sizes: readonly number[]) {
    // A bitmap's header of 40 bytes, and nothing behind it.
    const picture = Buffer.concat([this.u32(40), Buffer.alloc(36)]);
    const entries = sizes.map((size, at) =>
      Buffer.concat([
        // A side of 256 is written as zero.
        Buffer.from([size % 256, size % 256, 0, 0, 1, 0, 32, 0]),
        this.u32(picture.byteLength),
        this.u32(6 + 16 * sizes.length + at * picture.byteLength)
      ])
    );
    return Buffer.concat([
      Buffer.from([0, 0, 1, 0]),
      this.u16(sizes.length),
      ...entries,
      ...sizes.map(() => picture)
    ]);
  }

  /** A TIFF of this many pages, each a gray picture of 8 by 8. */
  public tiffOf(pages: number) {
    const entry = (tag: number, kind: 3 | 4, value: number) =>
      Buffer.concat([
        this.u16(tag),
        this.u16(kind),
        this.u32(1),
        // A short value stands at the start of its field.
        kind === 3
          ? Buffer.concat([this.u16(value), Buffer.alloc(2)])
          : this.u32(value)
      ]);
    // The width, the height, the bits to a sample, how it is packed, what a sample means,
    // and how many samples a pixel has.
    const entries = [
      entry(256, 4, 8),
      entry(257, 4, 8),
      entry(258, 3, 8),
      entry(259, 3, 1),
      entry(262, 3, 1),
      entry(277, 3, 1)
    ];
    const directory = 2 + 12 * entries.length + 4;
    const directories = Array.from({ length: pages }, (_, at) =>
      Buffer.concat([
        this.u16(entries.length),
        ...entries,
        // Where the next page's directory is: nowhere, after the last.
        this.u32(at + 1 < pages ? 8 + directory * (at + 1) : 0)
      ])
    );
    return Buffer.concat([
      Buffer.from("II", "latin1"),
      this.u16(42),
      this.u32(8),
      ...directories
    ]);
  }
}
