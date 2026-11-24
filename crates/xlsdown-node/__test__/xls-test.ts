import assert from "node:assert/strict";
import { createHash } from "node:crypto";

import { WorkupService } from "@/__test__/workup.ts";
import type { SheetCells } from "@/index.js";

/** One stream of a compound file under construction. */
export interface StreamSpec {
  readonly name: string;
  readonly data: Buffer;
}

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

  /**
   * The second cell of the second column, times two: the cell as two bytes of row and two of
   * column, with the bits that say both move, then a whole number, and then the sign that
   * joins them.
   */
  private get doubled() {
    return Buffer.from([0x44, 1, 0, 0x01, 0xc0, 0x1e, 2, 0, 0x05]);
  }

  /** One cell times another, each given as its row and column from nought. */
  public product(
    first: readonly [number, number],
    second: readonly [number, number]
  ) {
    const cell = ([row, col]: readonly [number, number]) => [
      0x44,
      row,
      0,
      col,
      0xc0
    ];
    return Buffer.from([...cell(first), ...cell(second), 0x05]);
  }

  /** The sum of the rows `from` to `to` of one column: the range, then the mark that sums it. */
  public sum(col: number, from: number, to: number) {
    return Buffer.from([
      0x25,
      from,
      0,
      to,
      0,
      col,
      0xc0,
      col,
      0xc0,
      0x19,
      0x10,
      0,
      0
    ]);
  }

  /**
   * A formula cell left with the number `value`. Its formula is `tokens`, or failing them the
   * second cell of the second column, times two.
   */
  public formula(
    row: number,
    col: number,
    xf: number,
    value: number,
    tokens = this.doubled
  ) {
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

  /** The password Excel locks a workbook with when it is given none. */
  public get excelsOwnPassword() {
    return "VelvetSweatshop";
  }

  private md5(data: Buffer) {
    return createHash("md5").update(data).digest();
  }

  /** The RC4 cipher's stream under `key`, laid over `data`. */
  private rc4(key: Buffer, data: Buffer) {
    const state = Uint8Array.from({ length: 256 }, (_, at) => at);
    const swap = (i: number, j: number) => {
      const held = state[i] ?? 0;
      state[i] = state[j] ?? 0;
      state[j] = held;
    };
    let j = 0;
    for (let i = 0; i < 256; i++) {
      j = (j + (state[i] ?? 0) + (key[i % key.length] ?? 0)) & 255;
      swap(i, j);
    }
    const out = Buffer.from(data);
    let i = 0;
    j = 0;
    for (let at = 0; at < out.length; at++) {
      i = (i + 1) & 255;
      j = (j + (state[i] ?? 0)) & 255;
      swap(i, j);
      const over = state[((state[i] ?? 0) + (state[j] ?? 0)) & 255] ?? 0;
      out[at] = (out[at] ?? 0) ^ over;
    }
    return out;
  }

  /** The salt of every lock made here. */
  private get salt() {
    return Buffer.from(Array.from({ length: 16 }, (_, at) => 0x40 + at));
  }

  /**
   * The stream laid over one stretch of 1024 bytes of a workbook's records locked with
   * `password`: the key is the MD5 of the password, mixed with the salt sixteen times over,
   * and then with the number of the stretch.
   */
  private streamOf(password: string, stretch: number) {
    const ofPassword = this.md5(Buffer.from(password, "utf16le"));
    const salted = Buffer.concat(
      Array.from({ length: 16 }, () =>
        Buffer.concat([ofPassword.subarray(0, 5), this.salt])
      )
    );
    const mixed = this.md5(salted).subarray(0, 5);
    const key = this.md5(Buffer.concat([mixed, this.u32(stretch)]));
    return this.rc4(key, Buffer.alloc(1024));
  }

  /**
   * The record that states a lock `password` opens: the salt, and then a verifier and its
   * hash, encrypted one after the other under the key of the first stretch.
   */
  private lockStated(password: string) {
    const verifier = Buffer.from(
      Array.from({ length: 16 }, (_, at) => 0xa0 ^ (at * 7))
    );
    const over = this.streamOf(password, 0);
    const kept = Buffer.concat([verifier, this.md5(verifier)]).map(
      (byte, at) => byte ^ (over[at] ?? 0)
    );
    return this.record(
      0x002f,
      Buffer.concat([Buffer.from([1, 0, 1, 0, 1, 0]), this.salt, kept])
    );
  }

  /**
   * A workbook's records, encrypted as the format says to: every byte of every record's data
   * after the record that states the lock, under the key of the stretch of 1024 bytes of the
   * stream the byte is in. Left as they are: the four bytes that head each record, seven
   * records whole, and the four bytes of the record that lists a sheet that say where the
   * sheet's records start.
   */
  private encrypt(stream: Buffer, password: string) {
    const out = Buffer.from(stream);
    const streams = new Map<number, Buffer>();
    const never = [0x0809, 0x002f, 0x00e1, 0x0194, 0x0195, 0x0196, 0x0138];
    let locked = false;
    let at = 0;
    while (at + 4 <= out.length) {
      const id = out.readUInt16LE(at);
      const end = Math.min(at + 4 + out.readUInt16LE(at + 2), out.length);
      if (locked && !never.includes(id)) {
        const from = id === 0x0085 ? at + 8 : at + 4;
        for (let place = from; place < end; place++) {
          const stretch = Math.floor(place / 1024);
          const over = streams.get(stretch) ?? this.streamOf(password, stretch);
          streams.set(stretch, over);
          out[place] = (out[place] ?? 0) ^ (over[place % 1024] ?? 0);
        }
      }
      locked ||= id === 0x002f;
      at = end;
    }
    return out;
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
    right: number,
    start: number,
    size: number
  ) {
    const entry = Buffer.alloc(128);
    Buffer.from(name, "utf16le").copy(entry);
    entry.writeUInt16LE((name.length + 1) * 2, 0x40);
    entry.writeUInt8(kind, 0x42);
    entry.writeUInt8(1, 0x43);
    entry.writeUInt32LE(FREE, 0x44);
    entry.writeUInt32LE(right, 0x48);
    entry.writeUInt32LE(child, 0x4c);
    entry.writeUInt32LE(start, 0x74);
    entry.writeUInt32LE(size, 0x78);
    return entry;
  }

  /**
   * A compound file of these streams, each in the root. Every stream is filled out to 4,096
   * bytes, so that it lies in sectors of its own: the table that chains the sectors comes
   * first after the header, in as many sectors as it needs, then the directory, and then
   * the streams one after another.
   */
  private compound(streams: readonly StreamSpec[]) {
    const filled = streams.map(({ name, data }) => {
      const length = Math.max(4096, Math.ceil(data.length / SECTOR) * SECTOR);
      const padded = Buffer.alloc(length);
      data.copy(padded);
      return { name, padded };
    });
    const directorySectors = Math.ceil((filled.length + 1) / 4);
    const streamSectors = filled.reduce(
      (sum, { padded }) => sum + padded.length / SECTOR,
      0
    );
    // The table chains its own sectors too, so they are counted until the count holds.
    let tableSectors = 1;
    while (
      tableSectors * (SECTOR / 4) <
      tableSectors + directorySectors + streamSectors
    ) {
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
    for (let sector = 0; sector < directorySectors; sector++) {
      const at = tableSectors + sector;
      const next = sector + 1 === directorySectors ? END : at + 1;
      table.writeUInt32LE(next, at * 4);
    }
    // The root holds the first stream, and each stream leads to the one after it.
    const directory = Buffer.alloc(directorySectors * SECTOR);
    this.entry(
      "Root Entry",
      5,
      filled.length > 0 ? 1 : FREE,
      FREE,
      END,
      0
    ).copy(directory);
    let next = tableSectors + directorySectors;
    filled.forEach(({ name, padded }, index) => {
      const sectors = padded.length / SECTOR;
      for (let sector = 0; sector < sectors; sector++) {
        const after = sector + 1 === sectors ? END : next + sector + 1;
        table.writeUInt32LE(after, (next + sector) * 4);
      }
      const right = index + 1 === filled.length ? FREE : index + 2;
      this.entry(name, 2, FREE, right, next, padded.length).copy(
        directory,
        (index + 1) * 128
      );
      next += sectors;
    });
    // An entry of the directory that is not in use leads nowhere.
    for (let index = filled.length + 1; index < directorySectors * 4; index++) {
      directory.fill(0xff, index * 128 + 0x44, index * 128 + 0x50);
    }
    return Buffer.concat([
      header,
      table,
      directory,
      ...filled.map(({ padded }) => padded)
    ]);
  }

  /**
   * A property stream of one set, in the Western code page: each property a number and its
   * value, which is text, a whole number, or a moment in tenths of a microsecond since 1601.
   */
  public properties(
    stated: readonly (readonly [number, string | number | bigint])[]
  ) {
    const page = Buffer.alloc(8);
    page.writeUInt32LE(2, 0);
    page.writeUInt16LE(1252, 4);
    const values = Array.of<Buffer>(page);
    for (const [, value] of stated) {
      if (typeof value === "string") {
        const text = Buffer.from(`${value}\0`, "latin1");
        const kept = Buffer.alloc(Math.ceil((8 + text.length) / 4) * 4);
        kept.writeUInt32LE(0x1e, 0);
        kept.writeUInt32LE(text.length, 4);
        text.copy(kept, 8);
        values.push(kept);
      } else if (typeof value === "bigint") {
        const kept = Buffer.alloc(12);
        kept.writeUInt32LE(0x40, 0);
        kept.writeBigUInt64LE(value, 4);
        values.push(kept);
      } else {
        const kept = Buffer.alloc(8);
        kept.writeUInt32LE(3, 0);
        kept.writeInt32LE(value, 4);
        values.push(kept);
      }
    }
    // The set: its length, how many properties, where each is, and then the values. The
    // first property is the code page.
    const ids = [1, ...stated.map(([id]) => id)];
    const listed = Buffer.alloc(values.length * 8);
    let at = 8 + listed.length;
    values.forEach((value, index) => {
      listed.writeUInt32LE(ids[index] ?? 0, index * 8);
      listed.writeUInt32LE(at, index * 8 + 4);
      at += value.length;
    });
    // The stream: its byte order, a version, who wrote it, a count of sets, and for the one
    // set what it is and where it starts.
    const head = Buffer.alloc(48);
    head.writeUInt16LE(0xfffe, 0);
    head.writeUInt32LE(1, 24);
    head.writeUInt32LE(48, 44);
    return Buffer.concat([
      head,
      this.u32(at),
      this.u32(values.length),
      listed,
      ...values
    ]);
  }

  /**
   * An `.xls` file of these sheets, with `globals` among the workbook's own records, and
   * `beside` as streams beside the workbook's. With a `password`, the records are locked
   * with it: the record that states the lock comes first, and the rest are encrypted.
   */
  public file(
    globals: readonly Buffer[],
    sheets: readonly SheetSpec[],
    beside: readonly StreamSpec[] = [],
    password?: string
  ) {
    const data =
      password === undefined
        ? this.stream(globals, sheets)
        : this.encrypt(
            this.stream([this.lockStated(password), ...globals], sheets),
            password
          );
    return this.compound([{ name: "Workbook", data }, ...beside]);
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
   * and a formula that doubles the amount; the second holds one label.
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
            this.formula(2, 1, 0, 2469)
          ]
        },
        { name: "Notes", hidden: true, records: [this.shared(0, 0, 0, 1)] }
      ]
    );
  }

  /**
   * A workbook that bills two things and adds them up: two bolts at ten each, worked out by a
   * formula, labor typed in, and under them a formula that sums the two. Nothing on the sheet
   * is labeled as a total, so only the formulas say which amount is one.
   */
  public get billed() {
    return this.file(
      [this.sharedStrings(["Bolt", "Labor", "In all"]), this.xf(0), this.xf(4)],
      [
        {
          name: "Bill",
          records: [
            this.shared(0, 0, 0, 0),
            this.number(0, 1, 0, 2),
            this.number(0, 2, 1, 10),
            this.formula(0, 3, 1, 20, this.product([0, 1], [0, 2])),
            this.shared(1, 0, 0, 1),
            this.number(1, 3, 1, 24.5),
            this.shared(2, 0, 0, 2),
            this.formula(2, 3, 1, 44.5, this.sum(3, 0, 1))
          ]
        }
      ]
    );
  }

  /** The moment the described workbook says it was made: 2026-11-15 09:30:00. */
  public get aMorning() {
    return 134_392_086_000_000_000n;
  }

  /**
   * The same two sheets, in a file that says what it is: a title, who wrote it and who saved
   * it last, when, and with what.
   */
  public get described() {
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
            this.formula(2, 1, 0, 2469)
          ]
        },
        { name: "Notes", hidden: true, records: [this.shared(0, 0, 0, 1)] }
      ],
      [
        {
          name: "\u0005SummaryInformation",
          data: this.properties([
            [2, "Order form"],
            [3, "Parts"],
            [4, "A. Clerk"],
            [5, "orders; parts"],
            [6, "For the spring"],
            [8, "B. Checker"],
            [9, "7"],
            [12, this.aMorning],
            // An hour later.
            [13, this.aMorning + 36_000_000_000n],
            [18, "Microsoft Excel"]
          ])
        },
        {
          name: "\u0005DocumentSummaryInformation",
          data: this.properties([
            [2, "Forms"],
            [15, "Acme Tooling"],
            // Two halves of one number: twelve, and nought.
            [23, 0x000c0000],
            [28, "en-US"]
          ])
        }
      ]
    );
  }

  /** A workbook of one sheet with enough cells that reading them takes a while. */
  /** A sheet's worth of numbers: so many rows of five. */
  private numbers(rows: number) {
    const records = Array.of<Buffer>();
    for (let row = 0; row < rows; row++) {
      for (let col = 0; col < 5; col++) {
        records.push(this.number(row, col, 0, row * 5 + col + 0.25));
      }
    }
    return records;
  }

  public get long() {
    return this.file(
      [this.xf(0)],
      [{ name: "Many", records: this.numbers(8000) }]
    );
  }

  /**
   * A ledger: text the cells share, a date, an amount, a formula, a hidden sheet, and a sheet
   * of numbers long enough for the records to run to several stretches of 1024 bytes. Locked
   * with `password` where one is given.
   */
  private ledgerLockedWith(password?: string) {
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
            this.formula(2, 1, 0, 2469)
          ]
        },
        { name: "Notes", hidden: true, records: [this.shared(0, 0, 0, 1)] },
        { name: "Many", records: this.numbers(60) }
      ],
      [],
      password
    );
  }

  /** The ledger, not locked. */
  public get ledger() {
    return this.ledgerLockedWith();
  }

  /**
   * The ledger as Excel locks a workbook when it is given no password, which it does for one
   * whose structure is protected: with a password of its own, which it never asks for.
   */
  public get lockedByExcel() {
    return this.ledgerLockedWith(this.excelsOwnPassword);
  }

  /** The ledger locked with a password of its owner's choosing. */
  public get lockedByItsOwner() {
    return this.ledgerLockedWith("known to its owner");
  }

  /** The same kind of workbook, saved with a password. */
  public get locked() {
    return this.file(
      [this.encrypted],
      [{ name: "Costs", records: [this.number(0, 0, 0, 1)] }]
    );
  }
}
