import assert from "node:assert/strict";

import { WorkupService } from "@/__test__/workup.ts";
import type { SheetCells } from "@/index.js";

/** One sheet of a workbook under construction. */
export interface SheetSpec {
  readonly name: string;
  readonly hidden?: boolean;
  readonly records: readonly Buffer[];
}

const SECTOR = 512;
/** The last sector of a chain points here, and an entry with nothing beside it here. */
const END = 0xfffffffe;
const FREE = 0xffffffff;

/**
 * Builds `.xls` files in code, a record at a time, so that no fixture is needed: a workbook of
 * Excel 97 to 2003 is a compound file whose `Workbook` stream is a run of records.
 */
export class XlsTest extends WorkupService {
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

  /** One record: its number, the length of its data, and the data. */
  public record(id: number, data: Buffer) {
    return Buffer.concat([this.u16(id), this.u16(data.length), data]);
  }

  /** A string's characters, two bytes to each, after the byte that says so. */
  private characters(text: string) {
    return Buffer.concat([Buffer.from([1]), Buffer.from(text, "utf16le")]);
  }

  private text(text: string) {
    return Buffer.concat([this.u16(text.length), this.characters(text)]);
  }

  private shortText(text: string) {
    return Buffer.concat([Buffer.from([text.length]), this.characters(text)]);
  }

  /** The record that opens a run of records: 5 for the workbook's own, 16 for a sheet's. */
  private bof(kind: number) {
    return this.record(
      0x0809,
      Buffer.concat([this.u16(0x0600), this.u16(kind), Buffer.alloc(12)])
    );
  }

  private get eof() {
    return this.record(0x000a, Buffer.alloc(0));
  }

  private cellRecord(
    id: number,
    row: number,
    col: number,
    xf: number,
    value: Buffer
  ) {
    return this.record(
      id,
      Buffer.concat([this.u16(row), this.u16(col), this.u16(xf), value])
    );
  }

  private double(value: number) {
    const bytes = Buffer.alloc(8);
    bytes.writeDoubleLE(value);
    return bytes;
  }

  public number(row: number, col: number, xf: number, value: number) {
    return this.cellRecord(0x0203, row, col, xf, this.double(value));
  }

  /** A cell that holds the string at `index` of the shared strings. */
  public shared(row: number, col: number, xf: number, index: number) {
    return this.cellRecord(0x00fd, row, col, xf, this.u32(index));
  }

  /** A formula cell left with the number `value`. Its tokens are the whole number 1. */
  public formula(row: number, col: number, xf: number, value: number) {
    const tokens = Buffer.from([0x1e, 1, 0]);
    return this.cellRecord(
      0x0006,
      row,
      col,
      xf,
      Buffer.concat([
        this.double(value),
        Buffer.alloc(6),
        this.u16(tokens.length),
        tokens
      ])
    );
  }

  /** The table of the strings cells share. */
  public sharedStrings(strings: readonly string[]) {
    return this.record(
      0x00fc,
      Buffer.concat([
        this.u32(strings.length),
        this.u32(strings.length),
        ...strings.map((text) => this.text(text))
      ])
    );
  }

  /** A cell format that selects the number format numbered `format`. */
  public xf(format: number) {
    const data = Buffer.alloc(20);
    data.writeUInt16LE(format, 2);
    return this.record(0x00e0, data);
  }

  /** The record a workbook saved with a password carries. */
  public get encrypted() {
    return this.record(0x002f, Buffer.alloc(6));
  }

  /** The stream of records: the workbook's own, and then each sheet's. */
  private stream(globals: readonly Buffer[], sheets: readonly SheetSpec[]) {
    const bound = (sheet: SheetSpec, at: number) =>
      this.record(
        0x0085,
        Buffer.concat([
          this.u32(at),
          Buffer.from([sheet.hidden ? 1 : 0, 0]),
          this.shortText(sheet.name)
        ])
      );
    const head = Buffer.concat([this.bof(0x0005), ...globals]);
    const listed = sheets.reduce(
      (sum, sheet) => sum + bound(sheet, 0).length,
      0
    );
    let at = head.length + listed + this.eof.length;
    const bounds = Array.of<Buffer>();
    const bodies = Array.of<Buffer>();
    for (const sheet of sheets) {
      bounds.push(bound(sheet, at));
      const body = Buffer.concat([
        this.bof(0x0010),
        ...sheet.records,
        this.eof
      ]);
      at += body.length;
      bodies.push(body);
    }
    return Buffer.concat([head, ...bounds, this.eof, ...bodies]);
  }

  private entry(
    name: string,
    kind: number,
    child: number,
    start: number,
    size: number
  ) {
    const entry = Buffer.alloc(128);
    Buffer.from(name, "utf16le").copy(entry);
    entry.writeUInt16LE((name.length + 1) * 2, 0x40);
    entry.writeUInt8(kind, 0x42);
    entry.writeUInt8(1, 0x43);
    entry.writeUInt32LE(FREE, 0x44);
    entry.writeUInt32LE(FREE, 0x48);
    entry.writeUInt32LE(child, 0x4c);
    entry.writeUInt32LE(start, 0x74);
    entry.writeUInt32LE(size, 0x78);
    return entry;
  }

  /**
   * A compound file with `stream` as its `Workbook`. The stream is filled out to 4,096 bytes,
   * so that it lies in sectors of its own: the table that chains the sectors comes first
   * after the header, in as many sectors as it needs, then the directory in one, and the
   * stream is the rest.
   */
  private compound(stream: Buffer) {
    const length = Math.max(4096, Math.ceil(stream.length / SECTOR) * SECTOR);
    const padded = Buffer.alloc(length);
    stream.copy(padded);
    const sectors = length / SECTOR;
    // The table chains its own sectors too, so they are counted until the count holds.
    let tableSectors = 1;
    while (tableSectors * (SECTOR / 4) < tableSectors + 1 + sectors) {
      tableSectors += 1;
    }
    // The header lists the table's sectors, and has room for 109 of them.
    assert.ok(tableSectors <= 109, "the test workbook is too large");

    const header = Buffer.alloc(SECTOR);
    Buffer.from([0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1]).copy(header);
    header.writeUInt16LE(0x3e, 0x18);
    header.writeUInt16LE(3, 0x1a);
    header.writeUInt16LE(0xfffe, 0x1c);
    header.writeUInt16LE(9, 0x1e);
    header.writeUInt16LE(6, 0x20);
    header.writeUInt32LE(tableSectors, 0x2c);
    header.writeUInt32LE(tableSectors, 0x30);
    header.writeUInt32LE(4096, 0x38);
    header.writeUInt32LE(END, 0x3c);
    header.writeUInt32LE(END, 0x44);
    header.fill(0xff, 0x4c);
    for (let sector = 0; sector < tableSectors; sector++) {
      header.writeUInt32LE(sector, 0x4c + sector * 4);
    }

    const table = Buffer.alloc(tableSectors * SECTOR, 0xff);
    for (let sector = 0; sector < tableSectors; sector++) {
      table.writeUInt32LE(0xfffffffd, sector * 4);
    }
    // The directory is the sector after the table, and the stream starts after that.
    table.writeUInt32LE(END, tableSectors * 4);
    const first = tableSectors + 1;
    for (let sector = 0; sector < sectors; sector++) {
      const next = sector + 1 === sectors ? END : first + sector + 1;
      table.writeUInt32LE(next, (first + sector) * 4);
    }

    const unused = Buffer.alloc(128);
    unused.fill(0xff, 0x44, 0x50);
    const directory = Buffer.concat([
      this.entry("Root Entry", 5, 1, END, 0),
      this.entry("Workbook", 2, FREE, first, length),
      unused,
      unused
    ]);
    return Buffer.concat([header, table, directory, padded]);
  }

  /** An `.xls` file of these sheets, with `globals` among the workbook's own records. */
  public file(globals: readonly Buffer[], sheets: readonly SheetSpec[]) {
    return this.compound(this.stream(globals, sheets));
  }

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

  /**
   * A workbook of two sheets, the second hidden. The first holds a label, a date, an amount,
   * and a formula left with 8; the second holds one label.
   */
  public get costs() {
    return this.file(
      [
        this.sharedStrings(["Total", "kept back"]),
        this.xf(0),
        this.xf(14),
        this.xf(4)
      ],
      [
        {
          name: "Costs",
          records: [
            this.shared(0, 0, 0, 0),
            this.number(0, 1, 1, 46349),
            this.number(1, 1, 2, 1234.5),
            this.formula(2, 1, 0, 8)
          ]
        },
        { name: "Notes", hidden: true, records: [this.shared(0, 0, 0, 1)] }
      ]
    );
  }

  /** A workbook of one sheet with enough cells that reading them takes a while. */
  public get long() {
    const records = Array.of<Buffer>();
    for (let row = 0; row < 8000; row++) {
      for (let col = 0; col < 5; col++) {
        records.push(this.number(row, col, 0, row * 5 + col + 0.25));
      }
    }
    return this.file([this.xf(0)], [{ name: "Many", records }]);
  }

  /** The same kind of workbook, saved with a password. */
  public get locked() {
    return this.file(
      [this.encrypted],
      [{ name: "Costs", records: [this.number(0, 0, 0, 1)] }]
    );
  }
}
