import assert from "node:assert/strict";

import { WorkupService } from "@/__test__/workup.ts";

/** One entry of a compound file under construction: a stream, or a storage of entries. */
export interface Entry {
  readonly name: string;
  readonly data?: Buffer;
  readonly children?: readonly Entry[];
}

/** An entry as the directory lists it. */
interface Placed {
  name: string;
  /** 5 for the root, 1 for a storage, 2 for a stream. */
  kind: number;
  child: number;
  right: number;
  data: Buffer | undefined;
}

const SECTOR = 512;
/** Where the text starts in the main stream, after the block that says where everything is. */
const TEXT_AT = 0x0800;
/** The last sector of a chain points here, and an entry with nothing beside it here. */
const END = 0xfffffffe;
const FREE = 0xffffffff;

/**
 * Builds `.doc` files in code, so that no fixture is needed: a document of Word 97 to 2003
 * is a compound file with its text in one stream and what describes the text in another.
 */
export class DocTest extends WorkupService {
  private u32(value: number) {
    const bytes = Buffer.alloc(4);
    bytes.writeUInt32LE(value);
    return bytes;
  }

  /** A stream filled out to 4,096 bytes and to whole sectors, so that it lies in its own. */
  private filled(data: Buffer) {
    const length = Math.max(4096, Math.ceil(data.length / SECTOR) * SECTOR);
    const filled = Buffer.alloc(length);
    data.copy(filled);
    return filled;
  }

  /** Lists `entries` after what is already listed, and gives the number of the first. */
  private place(entries: readonly Entry[], placed: Placed[]) {
    let first = FREE;
    let previous: Placed | undefined;
    for (const entry of entries) {
      const id = placed.length;
      const own = {
        name: entry.name,
        kind: entry.children ? 1 : 2,
        child: FREE,
        right: FREE,
        data: entry.children
          ? undefined
          : this.filled(entry.data ?? Buffer.alloc(0))
      } satisfies Placed;
      placed.push(own);
      if (entry.children) own.child = this.place(entry.children, placed);
      if (previous) previous.right = id;
      else first = id;
      previous = own;
    }
    return first;
  }

  private entry(placed: Placed, start: number, size: number) {
    const entry = Buffer.alloc(128);
    Buffer.from(placed.name, "utf16le").copy(entry);
    entry.writeUInt16LE((placed.name.length + 1) * 2, 0x40);
    entry.writeUInt8(placed.kind, 0x42);
    entry.writeUInt8(1, 0x43);
    entry.writeUInt32LE(FREE, 0x44);
    entry.writeUInt32LE(placed.right, 0x48);
    entry.writeUInt32LE(placed.child, 0x4c);
    entry.writeUInt32LE(start, 0x74);
    entry.writeUInt32LE(size, 0x78);
    return entry;
  }

  /**
   * A compound file of these entries. The table that chains the sectors comes first after
   * the header, in as many sectors as it needs, the directory follows it, and each stream
   * follows that in sectors of its own.
   */
  public compound(entries: readonly Entry[]) {
    const root = {
      name: "Root Entry",
      kind: 5,
      child: FREE,
      right: FREE,
      data: undefined
    } satisfies Placed;
    const placed = Array.of<Placed>(root);
    root.child = this.place(entries, placed);

    const directorySectors = Math.ceil(placed.length / 4);
    const streamSectors = placed.reduce(
      (sum, listed) => sum + (listed.data ? listed.data.length / SECTOR : 0),
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
    assert.ok(tableSectors <= 109, "the test document is too large");
    const table = Buffer.alloc(tableSectors * SECTOR, 0xff);
    for (let sector = 0; sector < tableSectors; sector++) {
      table.writeUInt32LE(0xfffffffd, sector * 4);
    }
    for (let sector = 0; sector < directorySectors; sector++) {
      const at = tableSectors + sector;
      const next = sector + 1 === directorySectors ? END : at + 1;
      table.writeUInt32LE(next, at * 4);
    }
    const directory = Buffer.alloc(directorySectors * SECTOR);
    const bodies = Array.of<Buffer>();
    let next = tableSectors + directorySectors;
    placed.forEach((listed, index) => {
      const data = listed.data;
      if (data) {
        const sectors = data.length / SECTOR;
        for (let sector = 0; sector < sectors; sector++) {
          const after = sector + 1 === sectors ? END : next + sector + 1;
          table.writeUInt32LE(after, (next + sector) * 4);
        }
        this.entry(listed, next, data.length).copy(directory, index * 128);
        next += sectors;
        bodies.push(data);
      } else {
        const start = listed.kind === 5 ? END : 0;
        this.entry(listed, start, 0).copy(directory, index * 128);
      }
    });
    // An entry of the directory that is not in use leads nowhere.
    for (let index = placed.length; index < directorySectors * 4; index++) {
      directory.fill(0xff, index * 128 + 0x44, index * 128 + 0x50);
    }

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
    return Buffer.concat([header, table, directory, ...bodies]);
  }

  /**
   * The two streams of a document whose text is `text`, a carriage return ending each
   * paragraph: the main stream, with the text one byte to a character, and the table stream,
   * with the one piece the text is in. `flags` are set in the opening block beside the one
   * that names the table stream, and `mark` and `version` are what the file says it is.
   */
  public streams(text: string, flags = 0, mark = 0xa5ec, version = 0x00c1) {
    const characters = Buffer.from(text, "latin1");
    const main = Buffer.alloc(TEXT_AT + characters.length);
    main.writeUInt16LE(mark, 0);
    main.writeUInt16LE(version, 2);
    main.writeUInt16LE(0x0200 | flags, 0x0a);
    main.writeUInt16LE(0x000e, 0x20);
    main.writeUInt16LE(0x0016, 0x3e);
    main.writeUInt32LE(characters.length, 0x4c);
    main.writeUInt16LE(0x005d, 0x98);
    characters.copy(main, TEXT_AT);
    // The pieces: a mark, a length, where the one piece starts and ends among the
    // characters, and where it is kept, doubled, with the bit that says a byte to each.
    const pieces = Buffer.concat([
      Buffer.from([2]),
      this.u32(16),
      this.u32(0),
      this.u32(characters.length),
      Buffer.alloc(2),
      this.u32(TEXT_AT * 2 + 0x40000000),
      Buffer.alloc(2)
    ]);
    const slot = 0x9a + 33 * 8;
    main.writeUInt32LE(0, slot);
    main.writeUInt32LE(pieces.length, slot + 4);
    return { main, table: pieces };
  }

  /** A `.doc` file of this text, carrying `carried` beside its own two streams. */
  public file(text: string, carried: readonly Entry[] = []) {
    const { main, table } = this.streams(text);
    return this.compound([
      { name: "WordDocument", data: main },
      { name: "1Table", data: table },
      ...carried
    ]);
  }

  public get invoiceText() {
    return [
      "Acme Tooling Ltd",
      "Invoice Number: INV-2026-0042",
      "Invoice Date: 2026-11-23",
      "Total Due: 1,250.00",
      ""
    ].join("\r");
  }

  /** An invoice of four paragraphs. */
  public get invoice() {
    return this.file(this.invoiceText);
  }

  /** The same invoice, carrying a PDF as Acrobat embeds one. */
  public get carrying() {
    return this.file(this.invoiceText, [
      {
        name: "ObjectPool",
        children: [
          {
            name: "_1234",
            children: [{ name: "CONTENTS", data: Buffer.from("%PDF-1.4 held") }]
          }
        ]
      }
    ]);
  }

  /** A document long enough that setting it on pages takes a while. */
  public get long() {
    const lines = Array.from(
      { length: 4000 },
      (_, n) => `Line ${n} of a long document, with words enough to wrap.`
    );
    return this.file(`${lines.join("\r")}\r`);
  }

  /** The same invoice, in a file that says it is saved with a password. */
  public get locked() {
    const { main, table } = this.streams(this.invoiceText, 0x0100);
    return this.compound([
      { name: "WordDocument", data: main },
      { name: "1Table", data: table }
    ]);
  }

  /** The same invoice, in a file that says Word 95 wrote it. */
  public get older() {
    const { main, table } = this.streams(this.invoiceText, 0, 0xa5dc, 0x0068);
    return this.compound([
      { name: "WordDocument", data: main },
      { name: "1Table", data: table }
    ]);
  }
}
