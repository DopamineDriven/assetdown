//! Files for the tests, built by hand.

const SECTOR: usize = 512;
const MINI: usize = 64;
const CUTOFF: usize = 4096;
const END: u32 = 0xFFFF_FFFE;
const FREE: u32 = 0xFFFF_FFFF;

/// A compound file of these streams, each in the root storage: sectors of 512 bytes, with the
/// streams under 4,096 bytes kept in the mini stream as the format has it.
pub(crate) fn compound(streams: &[(&str, &[u8])]) -> Vec<u8> {
  let sectors = |length: usize, size: usize| length.div_ceil(size);
  let small: Vec<usize> = (0..streams.len())
    .filter(|i| streams[*i].1.len() < CUTOFF)
    .collect();
  // The mini stream: every small stream, each starting on a mini sector.
  let mut mini_stream = Vec::new();
  let mut mini_fat: Vec<u32> = Vec::new();
  let mut mini_start = vec![END; streams.len()];
  for &index in &small {
    let bytes = streams[index].1;
    if bytes.is_empty() {
      continue;
    }
    let first = mini_fat.len();
    mini_start[index] = u32::try_from(first).unwrap();
    let count = sectors(bytes.len(), MINI);
    for sector in 0..count {
      let last = sector + 1 == count;
      mini_fat.push(if last {
        END
      } else {
        u32::try_from(first + sector + 1).unwrap()
      });
    }
    mini_stream.extend_from_slice(bytes);
    mini_stream.resize(mini_fat.len() * MINI, 0);
  }
  // Sectors, in order: the FAT, the directory, the mini FAT, the mini stream, the large streams.
  let directory_sectors = sectors((streams.len() + 1) * 128, SECTOR);
  let mini_fat_sectors = sectors(mini_fat.len() * 4, SECTOR);
  let mini_stream_sectors = sectors(mini_stream.len(), SECTOR);
  let mut fat: Vec<u32> = vec![0xFFFF_FFFD];
  let chain = |count: usize, fat: &mut Vec<u32>| -> u32 {
    if count == 0 {
      return END;
    }
    let first = fat.len();
    for sector in 0..count {
      let last = sector + 1 == count;
      fat.push(if last {
        END
      } else {
        u32::try_from(first + sector + 1).unwrap()
      });
    }
    u32::try_from(first).unwrap()
  };
  let directory_at = chain(directory_sectors, &mut fat);
  let mini_fat_at = chain(mini_fat_sectors, &mut fat);
  let mini_stream_at = chain(mini_stream_sectors, &mut fat);
  let mut large_start = vec![END; streams.len()];
  for (index, (_, bytes)) in streams.iter().enumerate() {
    if bytes.len() >= CUTOFF {
      large_start[index] = chain(sectors(bytes.len(), SECTOR), &mut fat);
    }
  }
  assert!(
    fat.len() <= SECTOR / 4,
    "the test file needs more than one sector of FAT"
  );
  fat.resize(SECTOR / 4, FREE);

  let mut header = vec![0u8; SECTOR];
  header[..8].copy_from_slice(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]);
  header[0x18..0x1A].copy_from_slice(&0x3Eu16.to_le_bytes());
  header[0x1A..0x1C].copy_from_slice(&3u16.to_le_bytes());
  header[0x1C..0x1E].copy_from_slice(&0xFFFEu16.to_le_bytes());
  header[0x1E..0x20].copy_from_slice(&9u16.to_le_bytes());
  header[0x20..0x22].copy_from_slice(&6u16.to_le_bytes());
  header[0x2C..0x30].copy_from_slice(&1u32.to_le_bytes());
  header[0x30..0x34].copy_from_slice(&directory_at.to_le_bytes());
  header[0x38..0x3C].copy_from_slice(&u32::try_from(CUTOFF).unwrap().to_le_bytes());
  header[0x3C..0x40].copy_from_slice(&mini_fat_at.to_le_bytes());
  header[0x40..0x44].copy_from_slice(&u32::try_from(mini_fat_sectors).unwrap().to_le_bytes());
  header[0x44..0x48].copy_from_slice(&END.to_le_bytes());
  header[0x4C..0x50].copy_from_slice(&0u32.to_le_bytes());
  for slot in 1..109 {
    header[0x4C + slot * 4..0x50 + slot * 4].copy_from_slice(&FREE.to_le_bytes());
  }

  let entry = |name: &str, kind: u8, next: u32, child: u32, start: u32, size: usize| {
    let mut entry = vec![0u8; 128];
    let units: Vec<u16> = name.encode_utf16().chain([0]).collect();
    for (at, unit) in units.iter().enumerate() {
      entry[at * 2..at * 2 + 2].copy_from_slice(&unit.to_le_bytes());
    }
    entry[0x40..0x42].copy_from_slice(&u16::try_from(units.len() * 2).unwrap().to_le_bytes());
    entry[0x42] = kind;
    entry[0x44..0x48].copy_from_slice(&FREE.to_le_bytes());
    entry[0x48..0x4C].copy_from_slice(&next.to_le_bytes());
    entry[0x4C..0x50].copy_from_slice(&child.to_le_bytes());
    entry[0x74..0x78].copy_from_slice(&start.to_le_bytes());
    entry[0x78..0x80].copy_from_slice(&u64::try_from(size).unwrap().to_le_bytes());
    entry
  };
  let mut directory = entry(
    "Root Entry",
    5,
    FREE,
    if streams.is_empty() { FREE } else { 1 },
    mini_stream_at,
    mini_stream.len(),
  );
  for (index, (name, bytes)) in streams.iter().enumerate() {
    let next = if index + 1 == streams.len() {
      FREE
    } else {
      u32::try_from(index + 2).unwrap()
    };
    let start = if bytes.len() < CUTOFF {
      mini_start[index]
    } else {
      large_start[index]
    };
    directory.extend(entry(name, 2, next, FREE, start, bytes.len()));
  }
  directory.resize(directory_sectors * SECTOR, 0);

  let mut file = header;
  file.extend(fat.iter().flat_map(|next| next.to_le_bytes()));
  file.extend(directory);
  let mut table: Vec<u8> = mini_fat
    .iter()
    .flat_map(|next| next.to_le_bytes())
    .collect();
  table.resize(mini_fat_sectors * SECTOR, 0xFF);
  file.extend(table);
  mini_stream.resize(mini_stream_sectors * SECTOR, 0);
  file.extend(mini_stream);
  for (_, bytes) in streams.iter().filter(|(_, bytes)| bytes.len() >= CUTOFF) {
    let mut padded = bytes.to_vec();
    padded.resize(sectors(bytes.len(), SECTOR) * SECTOR, 0);
    file.extend(padded);
  }
  file
}

/// What `\objdata` holds for an embedded object: the header that goes before its own data.
pub(crate) fn object_data(class: &str, native: &[u8]) -> Vec<u8> {
  let text = |text: &str| {
    let mut out = u32::try_from(text.len() + 1)
      .unwrap()
      .to_le_bytes()
      .to_vec();
    out.extend(text.as_bytes());
    out.push(0);
    out
  };
  let mut out = 0x0501u32.to_le_bytes().to_vec();
  out.extend(2u32.to_le_bytes());
  out.extend(text(class));
  out.extend(0u32.to_le_bytes());
  out.extend(0u32.to_le_bytes());
  out.extend(u32::try_from(native.len()).unwrap().to_le_bytes());
  out.extend(native);
  out
}

/// Bytes as RTF writes them: two hex digits to a byte, broken into lines.
pub(crate) fn hex(bytes: &[u8]) -> String {
  let digits: Vec<String> = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
  digits
    .chunks(32)
    .map(|line| line.concat())
    .collect::<Vec<_>>()
    .join("\r\n")
}

/// A document with this after its opening.
pub(crate) fn document(body: &str) -> Vec<u8> {
  format!("{{\\rtf1\\ansi\\deff0 {body}}}").into_bytes()
}

/// An embedded object of this class with this in its `\objdata`, and a picture standing for it.
pub(crate) fn object(class: &str, data: &[u8]) -> String {
  format!(
    "{{\\object\\objemb{{\\*\\objclass {class}}}\\objw100\\objh100{{\\*\\objdata {}}}{{\\result{{\\pict\\wmetafile8 0102}}}}}}",
    hex(data)
  )
}
