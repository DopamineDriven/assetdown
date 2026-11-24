import assert from "node:assert/strict";
import { crc32 } from "node:zlib";

import { WorkupService } from "@/__test__/workup.ts";
import type { SheetCells } from "@/index.js";

/** One file of a package under construction. */
interface Entry {
  readonly name: string;
  readonly xml: string;
}

const MAIN =
  "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml";
const SHEET_NAMESPACES =
  'xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"';
const RELATIONSHIPS =
  "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

/**
 * Reads the public fixtures, and builds an `.xlsx` file in code where a test needs one that
 * no fixture is: a workbook of 2007 or later is a zip of XML parts.
 */
export class XlsxTest extends WorkupService {
  /** The first sheet of a result. */
  public first(sheets: SheetCells[]) {
    const [sheet] = sheets;
    assert.ok(sheet, "the workbook returned no sheets");
    return sheet;
  }

  /** The cell at `ref`, failing the test when the sheet has none there. */
  public cell(sheet: SheetCells, ref: string) {
    const found = sheet.cells.find((c) => c.ref === ref);
    assert.ok(found, `${ref} is not among the cells of ${sheet.sheet}`);
    return found;
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

  /** A zip of these files, each kept as it is, without compression. */
  private zip(entries: readonly Entry[]) {
    const files = Array.of<Buffer>();
    const directory = Array.of<Buffer>();
    let offset = 0;
    for (const { name, xml } of entries) {
      const named = Buffer.from(name, "utf8");
      const data = Buffer.from(xml, "utf8");
      // What a file's own header and its line in the directory both state: the version,
      // flags, the method, the time, the date, the checksum, the two sizes, and the lengths
      // of the name and of the extra field.
      const stated = Buffer.concat([
        Buffer.from([20, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
        this.u32(crc32(data)),
        this.u32(data.length),
        this.u32(data.length),
        this.u16(named.length),
        this.u16(0)
      ]);
      const file = Buffer.concat([
        Buffer.from([0x50, 0x4b, 3, 4]),
        stated,
        named,
        data
      ]);
      // The directory's line adds the version that wrote the file in front, and after the
      // lengths a comment's length, the disk, two sets of attributes, and where the file
      // starts.
      directory.push(
        Buffer.concat([
          Buffer.from([0x50, 0x4b, 1, 2]),
          this.u16(20),
          stated,
          Buffer.alloc(10),
          this.u32(offset),
          named
        ])
      );
      files.push(file);
      offset += file.length;
    }
    const listed = Buffer.concat(directory);
    return Buffer.concat([
      ...files,
      listed,
      Buffer.from([0x50, 0x4b, 5, 6]),
      Buffer.alloc(4),
      this.u16(entries.length),
      this.u16(entries.length),
      this.u32(listed.length),
      this.u32(offset),
      this.u16(0)
    ]);
  }

  /** The part that lists what one part points at. */
  private pointsAt(kind: string, target: string) {
    return `<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="${RELATIONSHIPS}/${kind}" Target="/${target}"/></Relationships>`;
  }

  /**
   * An `.xlsx` file of one sheet named `name`, whose rows are `rows`: what stands inside the
   * sheet's `sheetData`.
   */
  public workbook(name: string, rows: string) {
    return this.zip([
      {
        name: "[Content_Types].xml",
        xml: `<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="${MAIN}"/></Types>`
      },
      {
        name: "_rels/.rels",
        xml: this.pointsAt("officeDocument", "xl/workbook.xml")
      },
      {
        name: "xl/workbook.xml",
        xml: `<workbook ${SHEET_NAMESPACES}><sheets><sheet name="${name}" sheetId="1" r:id="rId1"/></sheets></workbook>`
      },
      {
        name: "xl/_rels/workbook.xml.rels",
        xml: this.pointsAt("worksheet", "xl/worksheets/sheet1.xml")
      },
      {
        name: "xl/worksheets/sheet1.xml",
        xml: `<worksheet ${SHEET_NAMESPACES}><sheetData>${rows}</sheetData></worksheet>`
      }
    ]);
  }

  /** So many rows of five numbers, as a sheet keeps them. */
  private numbers(rows: number) {
    const written = Array.of<string>();
    for (let row = 0; row < rows; row++) {
      const cells = ["A", "B", "C", "D", "E"].map(
        (column, col) =>
          `<c r="${column}${row + 1}"><v>${row * 5 + col + 0.25}</v></c>`
      );
      written.push(`<row r="${row + 1}">${cells.join("")}</row>`);
    }
    return written.join("");
  }

  /** A workbook of three rows, built here and not kept as a fixture. */
  public get short() {
    return this.workbook("Few", this.numbers(3));
  }

  /** A workbook with cells enough that reading them takes a while: 40,000 of them. */
  public get long() {
    return this.workbook("Many", this.numbers(8000));
  }
}
