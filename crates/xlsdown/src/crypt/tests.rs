use super::*;
use crate::testing::record;

// What LibreOffice 7.3 wrote into a workbook it locked with the password Excel itself uses:
// the salt, then the verifier and the verifier's hash as the file keeps them. The workbook
// was this repository's public form, saved as an `.xls`.
const SALT: [u8; 16] = [
  0x1D, 0x08, 0xBE, 0x7B, 0xC2, 0x12, 0x32, 0x54, 0x41, 0x30, 0x03, 0xAF, 0xE3, 0x96, 0xEA, 0xBC,
];
const VERIFIER: [u8; 16] = [
  0x5E, 0x39, 0x33, 0xF1, 0x67, 0xDA, 0x49, 0x15, 0xE3, 0x2A, 0x3E, 0x71, 0xB2, 0xD2, 0xC5, 0xEE,
];
const VERIFIER_HASH: [u8; 16] = [
  0xBB, 0xC5, 0x3E, 0xE0, 0xD5, 0x63, 0x58, 0xF5, 0x05, 0x4B, 0x7C, 0x3A, 0x4F, 0xA7, 0x4B, 0x19,
];
// The same three from the same workbook locked with the password `another one`.
const OWN_SALT: [u8; 16] = [
  0x06, 0x82, 0x33, 0x39, 0x70, 0xAA, 0xC5, 0x53, 0xE5, 0xB7, 0xDD, 0xBA, 0x20, 0x80, 0x44, 0x99,
];
const OWN_VERIFIER: [u8; 16] = [
  0xD3, 0xD9, 0xAF, 0x97, 0x3C, 0xC3, 0xC9, 0x1E, 0xBC, 0x69, 0x69, 0xDA, 0xC5, 0xFD, 0xAB, 0x09,
];
const OWN_VERIFIER_HASH: [u8; 16] = [
  0x17, 0x1D, 0x19, 0x92, 0xE1, 0xB0, 0xCD, 0x4B, 0xA0, 0x24, 0x0F, 0x54, 0xED, 0x49, 0x18, 0xD4,
];

/// What the record that states an RC4 lock holds: the kind, the version, and the three.
fn rc4_lock(salt: &[u8; 16], verifier: &[u8; 16], verifier_hash: &[u8; 16]) -> Vec<u8> {
  [&[1, 0, 1, 0, 1, 0][..], salt, verifier, verifier_hash].concat()
}

fn ciphered(key: &str, text: &str) -> Vec<u8> {
  let mut bytes = text.as_bytes().to_vec();
  Rc4::new(key.as_bytes()).apply(&mut bytes);
  bytes
}

#[test]
fn rc4_gives_the_bytes_its_published_examples_give() {
  assert_eq!(
    ciphered("Key", "Plaintext"),
    [0xBB, 0xF3, 0x16, 0xE8, 0xD9, 0x40, 0xAF, 0x0A, 0xD3]
  );
  assert_eq!(ciphered("Wiki", "pedia"), [0x10, 0x21, 0xBF, 0x04, 0x20]);
  assert_eq!(
    ciphered("Secret", "Attack at dawn"),
    [
      0x45, 0xA0, 0x1F, 0x64, 0x5F, 0xC3, 0x5B, 0x38, 0x35, 0x52, 0x54, 0x4B, 0x9B, 0xF5
    ]
  );
}

#[test]
fn the_password_excel_itself_uses_opens_what_libreoffice_locked_with_it_and_no_other_does() {
  let lock = Lock::read(&rc4_lock(&SALT, &VERIFIER, &VERIFIER_HASH)).unwrap();
  assert!(lock.key(DEFAULT_PASSWORD).is_some());
  assert!(lock.key("another one").is_none());
  assert!(lock.key("velvetsweatshop").is_none());
  assert!(lock.key("").is_none());

  let own = Lock::read(&rc4_lock(&OWN_SALT, &OWN_VERIFIER, &OWN_VERIFIER_HASH)).unwrap();
  assert!(own.key("another one").is_some());
  assert!(own.key(DEFAULT_PASSWORD).is_none());
}

#[test]
fn a_lock_this_release_does_not_open_is_not_read_as_one_it_does() {
  let whole = rc4_lock(&SALT, &VERIFIER, &VERIFIER_HASH);
  assert!(Lock::read(&whole).is_some());
  // The older way of hiding a workbook, which is not encryption at all.
  assert!(Lock::read(&[0, 0, 0x12, 0x34, 0x56, 0x78]).is_none());
  // A lock cut short, at every length it could be cut to.
  for length in 0..whole.len() {
    assert!(Lock::read(&whole[..length]).is_none(), "{length} bytes");
  }
  // A version no reader knows.
  let mut later = whole.clone();
  later[2] = 9;
  assert!(Lock::read(&later).is_none());
  // A kind no reader knows.
  let mut other = whole;
  other[0] = 7;
  assert!(Lock::read(&other).is_none());
}

/// The start of a workbook's records under an RC4 lock of the first salt: the record that
/// opens them, and the one that states the lock. 78 bytes.
fn locked_start() -> Vec<u8> {
  [
    record(BOF, &[0u8; 16]),
    record(FILEPASS, &rc4_lock(&SALT, &VERIFIER, &VERIFIER_HASH)),
  ]
  .concat()
}

/// A record of no meaning that brings a stream of `so_far` bytes up to `until`.
fn filler(so_far: usize, until: usize) -> Vec<u8> {
  record(0x00FF, &vec![0xAA; until - so_far - 4])
}

#[test]
fn records_past_the_first_stretch_of_the_stream_come_out_as_libreoffice_put_them_in() {
  // Three records of that workbook as LibreOffice left them, each at the place in the
  // stream it was at, which is what its key is made from. The two that list the sheets:
  let form = [
    0x85, 0x00, 0x0C, 0x00, 0xE7, 0x06, 0x00, 0x00, 0x1A, 0x03, 0xA0, 0xF3, 0xA9, 0xBE, 0xF4, 0xDF,
  ];
  let notes = [
    0x85, 0x00, 0x0D, 0x00, 0x1C, 0x0C, 0x00, 0x00, 0xC7, 0x8B, 0x8C, 0x17, 0x99, 0x61, 0x68, 0xC4,
    0x2C,
  ];
  // And the one that says how far a sheet reaches, which lay across the end of the second
  // stretch of 1024 bytes and the start of the third.
  let extent = [
    0x00, 0x02, 0x0E, 0x00, 0xA5, 0x98, 0xE5, 0x76, 0x6C, 0x3F, 0x95, 0x89, 0x5F, 0x33, 0x0C, 0x29,
    0x35, 0x33,
  ];
  let mut stream = locked_start();
  stream.extend(filler(stream.len(), 1322));
  stream.extend(form);
  stream.extend(notes);
  stream.extend(filler(stream.len(), 2043));
  stream.extend(extent);
  stream.extend(record(EOF, &[]));

  let plain = unlocked(&stream, DEFAULT_PASSWORD).unwrap().unwrap();
  assert_eq!(plain.len(), stream.len());
  // Where a sheet's records start is never encrypted; how it is shown and its name are.
  assert_eq!(
    plain[1322..1338],
    [&[0x85, 0, 12, 0, 0xE7, 0x06, 0, 0, 0, 0, 4, 0][..], b"Form"].concat()
  );
  assert_eq!(
    plain[1338..1355],
    [
      &[0x85, 0, 13, 0, 0x1C, 0x0C, 0, 0, 1, 0, 5, 0][..],
      b"Notes"
    ]
    .concat()
  );
  // Rows 0 to 30 and columns 0 to 8.
  assert_eq!(
    plain[2043..2061],
    [0, 2, 14, 0, 0, 0, 0, 0, 30, 0, 0, 0, 0, 0, 8, 0, 0, 0]
  );
  // What opens the records and what states the lock are left as they were.
  assert_eq!(plain[..78], stream[..78]);
}

#[test]
fn a_stream_with_no_lock_is_left_alone_and_one_no_password_here_opens_is_refused() {
  let open = [record(BOF, &[0u8; 16]), record(EOF, &[])].concat();
  assert!(matches!(unlocked(&open, DEFAULT_PASSWORD), Ok(None)));
  // A record that states a lock after the workbook's own records have ended is a sheet's
  // business, not a lock on the workbook.
  let later = [open.clone(), record(FILEPASS, &[0u8; 6])].concat();
  assert!(matches!(unlocked(&later, DEFAULT_PASSWORD), Ok(None)));

  let refused = |stream: &[u8]| match unlocked(stream, DEFAULT_PASSWORD) {
    Err(SpreadsheetError::UnsupportedFormat(what)) => what,
    other => panic!("{other:?}"),
  };
  let own = [
    record(BOF, &[0u8; 16]),
    record(
      FILEPASS,
      &rc4_lock(&OWN_SALT, &OWN_VERIFIER, &OWN_VERIFIER_HASH),
    ),
    record(EOF, &[]),
  ]
  .concat();
  assert_eq!(refused(&own), "an encrypted workbook");
  let hidden = [
    record(BOF, &[0u8; 16]),
    record(FILEPASS, &[0u8; 6]),
    record(EOF, &[]),
  ]
  .concat();
  assert_eq!(refused(&hidden), "an encrypted workbook");
}
