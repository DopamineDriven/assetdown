import assert from "node:assert/strict";

import { WorkupService } from "@/__test__/workup.ts";

/**
 * The repository holds no JPEG, so the files here start from one small picture kept as a
 * literal, and are changed a segment at a time to say what a test needs said.
 */
export class JpegTest extends WorkupService {
  /** A white picture of gray, 16 pixels by 8, that states 200 pixels to the inch. */
  public get tiny() {
    return Buffer.from(
      "ffd8ffe000104a4649460001010100c800c80000ffdb00430003020203020203030303040303" +
        "04050805050404050a070706080c0a0c0c0b0a0b0b0d0e12100d0e110e0b0b10161011131415" +
        "15150c0f171816141812141514ffc0000b080008001001011100ffc400150001010000000000" +
        "0000000000000000000009ffc40014100100000000000000000000000000000000ffda000801" +
        "0100003f00aa63ffd9",
      "hex"
    );
  }

  /** `jpeg` with one more segment put in straight after its opening marker. */
  public withSegment(jpeg: Buffer, segment: Buffer) {
    return Buffer.concat([jpeg.subarray(0, 2), segment, jpeg.subarray(2)]);
  }

  /** An Exif segment whose one entry states how the picture is to be turned. */
  public orientation(turn: number) {
    const tiff = Buffer.alloc(8 + 2 + 12 + 4);
    tiff.write("II", 0, "latin1");
    tiff.writeUInt16LE(42, 2);
    tiff.writeUInt32LE(8, 4);
    tiff.writeUInt16LE(1, 8);
    tiff.writeUInt16LE(0x0112, 10);
    tiff.writeUInt16LE(3, 12);
    tiff.writeUInt32LE(1, 14);
    tiff.writeUInt16LE(turn, 18);
    const payload = Buffer.concat([Buffer.from("Exif\0\0", "latin1"), tiff]);
    const head = Buffer.from([0xff, 0xe1, 0, 0]);
    head.writeUInt16BE(payload.length + 2, 2);
    return Buffer.concat([head, payload]);
  }

  /**
   * `tiny` with the unit of its density changed: 0 is no unit, 1 the inch, 2 the centimeter.
   * The unit is the fourteenth byte of a file that opens with a JFIF segment.
   */
  public withUnit(unit: number) {
    const jpeg = Buffer.from(this.tiny);
    assert.equal(jpeg.subarray(6, 10).toString("latin1"), "JFIF");
    jpeg.writeUInt8(unit, 13);
    return jpeg;
  }

  /** `tiny` with its frame marked as one kept without loss, which is not read. */
  public get lossless() {
    const jpeg = Buffer.from(this.tiny);
    const frame = jpeg.indexOf(Buffer.from([0xff, 0xc0]));
    assert.ok(frame > 0);
    jpeg.writeUInt8(0xc3, frame + 1);
    return jpeg;
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
