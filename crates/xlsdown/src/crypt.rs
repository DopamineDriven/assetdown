//! A workbook saved with a password: how its records are locked, and unlocking them.
//!
//! Excel locks a workbook with a password of its own, `VelvetSweatshop`, whenever it is asked
//! to encrypt one without being given a password: when a workbook's structure is protected,
//! or it is saved to be opened read-only. Such a file opens in Excel without a question, so
//! it is read here without one too. A workbook locked with a password of its owner's choosing
//! stays an unsupported format.
//!
//! The record that states the lock (`FILEPASS`) comes straight after the one that opens the
//! workbook's records. Two kinds of lock are opened, both RC4, which differ in how the key is
//! made from the password: by MD5 (Excel 97 and 2000), and by SHA-1 (the "CryptoAPI" kind of
//! Excel 2002 and 2003). The older way of hiding a workbook (an XOR with a pattern) is not.
//!
//! The stream is encrypted as if all of it were: a fresh key for every 1024 bytes of it, made
//! from the password and the number of the stretch. But the four bytes that head each record
//! are left as they are, as are a few records whole, and where each sheet's records start.
//! Their places in the key's stream are passed over, not closed up.

use md5::{Digest, Md5};
use sha1::Sha1;
use xlsxdown::SpreadsheetError;

use crate::record::{BOF, EOF};

/// The password Excel locks a workbook with when it is given none.
pub(crate) const DEFAULT_PASSWORD: &str = "VelvetSweatshop";

/// The record that states how a workbook is locked.
pub(crate) const FILEPASS: u16 = 0x002F;
/// The record that lists a sheet. Its first four bytes say where the sheet's records start,
/// and are never encrypted.
const BOUNDSHEET: u16 = 0x0085;
/// The records that are never encrypted: the one that opens a run of records, the one that
/// states the lock, the one that opens the workbook's header, and four that say who has the
/// file open and what was changed in it.
const NEVER_ENCRYPTED: [u16; 7] = [BOF, FILEPASS, 0x00E1, 0x0194, 0x0195, 0x0196, 0x0138];
/// How many bytes of the stream one key encrypts.
const STRETCH: usize = 1024;
/// The RC4 cipher and the SHA-1 hash, as a lock of the later kind names them.
const RC4: u32 = 0x6801;
const SHA1: u32 = 0x8004;

/// The RC4 cipher: a stream of bytes made from a key, which is laid over the data.
pub(crate) struct Rc4 {
  state: [u8; 256],
  i: u8,
  j: u8,
}

impl Rc4 {
  /// The cipher under `key`, which must not be empty.
  pub fn new(key: &[u8]) -> Rc4 {
    let mut state = [0u8; 256];
    for (at, byte) in (0..=u8::MAX).zip(state.iter_mut()) {
      *byte = at;
    }
    let mut j = 0u8;
    for (i, key_byte) in (0..256).zip(key.iter().cycle()) {
      j = j.wrapping_add(state[i]).wrapping_add(*key_byte);
      state.swap(i, usize::from(j));
    }
    Rc4 { state, i: 0, j: 0 }
  }

  /// Lay the next bytes of the cipher's stream over `data`: encrypting and decrypting are
  /// the same.
  pub fn apply(&mut self, data: &mut [u8]) {
    for byte in data {
      self.i = self.i.wrapping_add(1);
      self.j = self.j.wrapping_add(self.state[usize::from(self.i)]);
      self.state.swap(usize::from(self.i), usize::from(self.j));
      let at = self.state[usize::from(self.i)].wrapping_add(self.state[usize::from(self.j)]);
      *byte ^= self.state[usize::from(at)];
    }
  }
}

fn md5(parts: &[&[u8]]) -> [u8; 16] {
  let mut hash = Md5::new();
  for part in parts {
    hash.update(part);
  }
  hash.finalize().into()
}

fn sha1(parts: &[&[u8]]) -> [u8; 20] {
  let mut hash = Sha1::new();
  for part in parts {
    hash.update(part);
  }
  hash.finalize().into()
}

/// A password as both kinds of lock hash it: two bytes to a unit, the low one first.
fn typed(password: &str) -> Vec<u8> {
  password.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

/// How a workbook is locked, as its `FILEPASS` record states it: a salt, and a verifier with
/// its hash, both encrypted, by which a password is known to be the right one.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Lock {
  /// RC4 under a key made by MD5.
  Rc4 {
    salt: [u8; 16],
    verifier: [u8; 16],
    verifier_hash: [u8; 16],
  },
  /// RC4 under a key of `key_bytes` bytes made by SHA-1.
  CryptoApi {
    key_bytes: usize,
    salt: [u8; 16],
    verifier: [u8; 16],
    verifier_hash: [u8; 20],
  },
}

/// What a password and a lock's salt make, from which the key of each stretch is made.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Key {
  Rc4 { hash: [u8; 5] },
  CryptoApi { hash: [u8; 20], key_bytes: usize },
}

impl Key {
  /// The key an RC4 lock makes of a password: MD5 of the password, then MD5 of the first five
  /// bytes of that with the salt, sixteen times over.
  pub fn rc4(password: &str, salt: &[u8; 16]) -> Key {
    let of_password = md5(&[&typed(password)]);
    let salted: Vec<u8> = [&of_password[..5], &salt[..]].concat().repeat(16);
    let mixed = md5(&[&salted]);
    let mut hash = [0u8; 5];
    hash.copy_from_slice(&mixed[..5]);
    Key::Rc4 { hash }
  }

  /// The key a lock of the later kind makes of a password: SHA-1 of the salt and the
  /// password.
  pub fn crypto_api(password: &str, salt: &[u8; 16], key_bytes: usize) -> Key {
    Key::CryptoApi {
      hash: sha1(&[salt, &typed(password)]),
      key_bytes,
    }
  }

  /// The stream of bytes laid over one stretch of 1024 bytes of the workbook's records.
  pub fn stream_of(&self, stretch: u32) -> [u8; STRETCH] {
    let number = stretch.to_le_bytes();
    let key: Vec<u8> = match self {
      Key::Rc4 { hash } => md5(&[hash, &number]).to_vec(),
      Key::CryptoApi { hash, key_bytes } => {
        let whole = sha1(&[hash, &number]);
        let mut key = whole[..(*key_bytes).min(whole.len())].to_vec();
        // A key of forty bits is made up to 128 with noughts.
        if key.len() == 5 {
          key.resize(16, 0);
        }
        key
      }
    };
    let mut stream = [0u8; STRETCH];
    Rc4::new(&key).apply(&mut stream);
    stream
  }
}

fn u16_at(data: &[u8], at: usize) -> Option<u16> {
  Some(u16::from_le_bytes(data.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
  Some(u32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

fn bytes_at<const N: usize>(data: &[u8], at: usize) -> Option<[u8; N]> {
  data.get(at..at.checked_add(N)?)?.try_into().ok()
}

impl Lock {
  /// The lock a `FILEPASS` record states. `None` for one this release does not open: the
  /// older hiding by XOR, a version or a cipher it does not know, and a record cut short.
  pub fn read(filepass: &[u8]) -> Option<Lock> {
    // The kind: 0 hides by XOR, 1 encrypts with RC4.
    if u16_at(filepass, 0)? != 1 {
      return None;
    }
    match (u16_at(filepass, 2)?, u16_at(filepass, 4)?) {
      (1, 1) => Some(Lock::Rc4 {
        salt: bytes_at(filepass, 6)?,
        verifier: bytes_at(filepass, 22)?,
        verifier_hash: bytes_at(filepass, 38)?,
      }),
      (2..=4, 2) => {
        // Flags, then the size of a header that names the cipher, the hash, and the size of
        // the key, then the header, then the salt and the verifier.
        let header_size = usize::try_from(u32_at(filepass, 10)?).ok()?;
        let header = filepass.get(14..14usize.checked_add(header_size)?)?;
        if u32_at(header, 8)? != RC4 || u32_at(header, 12)? != SHA1 {
          return None;
        }
        // No size given is forty bits.
        let key_bytes = match u32_at(header, 16)? {
          0 => 5,
          bits @ 40..=128 if bits % 8 == 0 => usize::try_from(bits / 8).ok()?,
          _ => return None,
        };
        let rest = filepass.get(14 + header_size..)?;
        if u32_at(rest, 0)? != 16 || u32_at(rest, 36)? != 20 {
          return None;
        }
        Some(Lock::CryptoApi {
          key_bytes,
          salt: bytes_at(rest, 4)?,
          verifier: bytes_at(rest, 20)?,
          verifier_hash: bytes_at(rest, 40)?,
        })
      }
      _ => None,
    }
  }

  /// What a password makes under this lock, whether or not the password is the right one.
  pub fn key_of(&self, password: &str) -> Key {
    match self {
      Lock::Rc4 { salt, .. } => Key::rc4(password, salt),
      Lock::CryptoApi {
        salt, key_bytes, ..
      } => Key::crypto_api(password, salt, *key_bytes),
    }
  }

  /// Whether a key opens the lock: decrypted with it, the verifier's hash is the hash of the
  /// verifier.
  fn opened_by(&self, key: &Key) -> bool {
    let stream = key.stream_of(0);
    let plain = |kept: &[u8], from: usize| -> Vec<u8> {
      kept
        .iter()
        .zip(&stream[from..])
        .map(|(byte, over)| byte ^ over)
        .collect()
    };
    match self {
      Lock::Rc4 {
        verifier,
        verifier_hash,
        ..
      } => md5(&[&plain(verifier, 0)])[..] == plain(verifier_hash, 16)[..],
      Lock::CryptoApi {
        verifier,
        verifier_hash,
        ..
      } => sha1(&[&plain(verifier, 0)])[..] == plain(verifier_hash, 16)[..],
    }
  }

  /// The key a password makes, when the password is the right one.
  pub fn key(&self, password: &str) -> Option<Key> {
    let key = self.key_of(password);
    self.opened_by(&key).then_some(key)
  }
}

/// The record at `at`: its number, and where its data starts and ends. A record that would
/// run past the end of the stream ends there.
fn record_at(stream: &[u8], at: usize) -> Option<(u16, std::ops::Range<usize>)> {
  let id = u16_at(stream, at)?;
  let length = usize::from(u16_at(stream, at.checked_add(2)?)?);
  let start = at + 4;
  Some((id, start..(start + length).min(stream.len())))
}

/// The record that states the workbook's lock: its data, and where the record after it
/// starts. `None` when the workbook's own records end without one.
fn stated_lock(stream: &[u8]) -> Option<(&[u8], usize)> {
  let mut at = 0;
  loop {
    let (id, data) = record_at(stream, at)?;
    match id {
      FILEPASS => return Some((&stream[data.clone()], data.end)),
      EOF => return None,
      _ => at = data.end,
    }
  }
}

/// Lay the key's stream over every encrypted byte of the records from `from` on.
fn decrypt(stream: &mut [u8], from: usize, key: &Key) {
  // The stream of the stretch being read, with which stretch it is.
  let mut over: Option<(usize, [u8; STRETCH])> = None;
  let mut at = from;
  while let Some((id, data)) = record_at(stream, at) {
    at = data.end;
    let encrypted = match id {
      _ if NEVER_ENCRYPTED.contains(&id) => continue,
      BOUNDSHEET => (data.start + 4).min(data.end)..data.end,
      _ => data,
    };
    for place in encrypted {
      let stretch = place / STRETCH;
      let stream_of_stretch = match over {
        Some((held, bytes)) if held == stretch => bytes,
        _ => {
          // A stream of more stretches than can be numbered is not a workbook's.
          let Ok(number) = u32::try_from(stretch) else {
            return;
          };
          let bytes = key.stream_of(number);
          over = Some((stretch, bytes));
          bytes
        }
      };
      stream[place] ^= stream_of_stretch[place % STRETCH];
    }
  }
}

/// A workbook's records with their lock taken off, when `password` opens it. `Ok(None)` for
/// records that are not locked. A lock the password does not open, and one of a kind this
/// release does not open, is an unsupported format.
pub(crate) fn unlocked(stream: &[u8], password: &str) -> Result<Option<Vec<u8>>, SpreadsheetError> {
  let Some((stated, after)) = stated_lock(stream) else {
    return Ok(None);
  };
  let key = Lock::read(stated)
    .and_then(|lock| lock.key(password))
    .ok_or_else(|| SpreadsheetError::UnsupportedFormat("an encrypted workbook".to_string()))?;
  let mut plain = stream.to_vec();
  decrypt(&mut plain, after, &key);
  Ok(Some(plain))
}

#[cfg(test)]
mod tests;
