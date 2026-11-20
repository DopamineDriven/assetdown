import { WorkupService } from "@/__test__/workup.ts";

/**
 * The repository holds no RTF file, and needs none: RTF is text, so every document here is
 * written out as the string it is.
 */
export class RtfTest extends WorkupService {
  /** A document with this after its opening. */
  public document(body: string) {
    return Buffer.from(`{\\rtf1\\ansi\\deff0 ${body}}`, "latin1");
  }

  /** Bytes as RTF writes them: two hex digits to a byte. */
  public hex(bytes: Buffer) {
    return bytes.toString("hex");
  }

  /**
   * What an object's data holds: the header that goes before an embedded object's own data
   * (a version, that it is embedded, its class, two names left empty, and the data's length).
   */
  public objectData(className: string, native: Buffer) {
    const number = (value: number) => {
      const bytes = Buffer.alloc(4);
      bytes.writeUInt32LE(value);
      return bytes;
    };
    const name = Buffer.from(`${className}\0`, "latin1");
    return Buffer.concat([
      number(0x0501),
      number(2),
      number(name.length),
      name,
      number(0),
      number(0),
      number(native.length),
      native
    ]);
  }

  /** An embedded object of this class, with a picture standing for it on the page. */
  public object(className: string, native: Buffer) {
    const data = this.hex(this.objectData(className, native));
    return `{\\object\\objemb{\\*\\objclass ${className}}{\\*\\objdata ${data}}{\\result{\\pict\\wmetafile8 0102}}}`;
  }

  /** A small invoice: a heading, two labeled lines, a table of one item, and a total. */
  public get invoice() {
    const row = (item: string, amount: string) =>
      `\\trowd\\cellx4320\\cellx7200\\pard\\intbl ${item}\\cell\\pard\\intbl ${amount}\\cell\\row`;
    return this.document(
      `\\b INVOICE\\b0\\par Invoice Number: INV-1001\\par Invoice Date: 03/14/2024\\par ${row("Description", "Amount")}${row("Engine inspection", "$1,234.56")}\\pard Total: $1,234.56\\par`
    );
  }

  /** The first bytes of a PNG, which is all a picture's format is told from. */
  public get png() {
    return Buffer.from("89504e470d0a1a0a0000000d49484452", "hex");
  }
}
