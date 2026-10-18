//! Chunked AES-256-GCM encryption for vault objects (stored files and the vault index).
//!
//! Format (see `docs/VAULT_FORMAT.md`):
//!
//! ```text
//! header (25 bytes): "RJV1" | kind u8 | object id (16) | chunk size u32 LE
//! chunks:            AES-256-GCM(ciphertext || tag), nonce = index u64 BE | 0 0 0 | final flag
//! ```
//!
//! Every chunk authenticates the header as associated data. The chunk index in the nonce
//! stops reordering and swapping, and the final flag stops truncation: dropping trailing
//! chunks leaves a last chunk that was not encrypted as final. The caller must use a key
//! that is unique to the object id and must never encrypt two plaintexts under the same
//! id; the vault derives each object's key from a fresh random id on every write.

use crate::crypto::cipher;
use aes_gcm::{
    aead::{Aead, KeyInit, Payload},
    Aes256Gcm, Nonce,
};
use std::io::{self, Read, Write};
use zeroize::Zeroizing;

/// Magic prefix of an encrypted vault object.
pub const MAGIC: &[u8; 4] = b"RJV1";
/// Plaintext bytes per chunk when writing.
pub const CHUNK_SIZE: u32 = 1 << 20;
/// Encoded header length: magic(4) + kind(1) + id(16) + chunk size(4).
pub const HEADER_LEN: usize = 25;
const TAG_LEN: usize = 16;
const MIN_CHUNK: u32 = 16;
const MAX_CHUNK: u32 = 16 << 20;

/// What an encrypted object holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectKind {
    File = 1,
    Index = 2,
}

/// The plaintext header that starts every encrypted object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamHeader {
    pub kind: ObjectKind,
    pub object_id: [u8; 16],
    pub chunk_size: u32,
}

impl StreamHeader {
    /// A header with the standard chunk size.
    pub fn new(kind: ObjectKind, object_id: [u8; 16]) -> Self {
        Self {
            kind,
            object_id,
            chunk_size: CHUNK_SIZE,
        }
    }

    pub fn to_bytes(&self) -> [u8; HEADER_LEN] {
        let mut out = [0u8; HEADER_LEN];
        out[..4].copy_from_slice(MAGIC);
        out[4] = self.kind as u8;
        out[5..21].copy_from_slice(&self.object_id);
        out[21..25].copy_from_slice(&self.chunk_size.to_le_bytes());
        out
    }

    pub fn parse(bytes: &[u8; HEADER_LEN]) -> Result<Self, StreamError> {
        if &bytes[..4] != MAGIC {
            return Err(StreamError::Format("not a vault object"));
        }
        let kind = match bytes[4] {
            1 => ObjectKind::File,
            2 => ObjectKind::Index,
            _ => return Err(StreamError::Format("unknown object kind")),
        };
        let mut object_id = [0u8; 16];
        object_id.copy_from_slice(&bytes[5..21]);
        let chunk_size = u32::from_le_bytes(bytes[21..25].try_into().unwrap());
        if !(MIN_CHUNK..=MAX_CHUNK).contains(&chunk_size) {
            return Err(StreamError::Format("unsupported chunk size"));
        }
        Ok(Self {
            kind,
            object_id,
            chunk_size,
        })
    }
}

/// Why reading or writing an encrypted object failed.
#[derive(Debug)]
pub enum StreamError {
    Io(io::Error),
    /// Not a vault object, or an unsupported header.
    Format(&'static str),
    /// Authentication failed: wrong key, or the object was modified, reordered, or truncated.
    Tampered,
}

impl std::fmt::Display for StreamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StreamError::Io(e) => write!(f, "I/O error: {}", e),
            StreamError::Format(m) => write!(f, "Invalid vault object: {}", m),
            StreamError::Tampered => write!(
                f,
                "Vault object failed authentication (wrong key, or modified or truncated)"
            ),
        }
    }
}

impl std::error::Error for StreamError {}

impl From<io::Error> for StreamError {
    fn from(e: io::Error) -> Self {
        StreamError::Io(e)
    }
}

fn nonce_for(index: u64, last: bool) -> [u8; 12] {
    let mut n = [0u8; 12];
    n[..8].copy_from_slice(&index.to_be_bytes());
    n[11] = u8::from(last);
    n
}

/// Reads until `buf` is full or the reader is exhausted; returns the bytes read.
fn read_full<R: Read>(src: &mut R, buf: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        match src.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(filled)
}

/// Encrypts everything in `src` and writes header + chunks to `dst`. Returns the
/// plaintext length.
pub fn encrypt_stream<R: Read, W: Write>(
    key: &cipher::Key,
    header: &StreamHeader,
    mut src: R,
    mut dst: W,
) -> Result<u64, StreamError> {
    let header_bytes = header.to_bytes();
    let cipher = Aes256Gcm::new(key.as_bytes().into());
    let size = header.chunk_size as usize;
    dst.write_all(&header_bytes)?;

    let mut current = Zeroizing::new(vec![0u8; size]);
    let mut next = Zeroizing::new(vec![0u8; size]);
    let mut current_len = read_full(&mut src, &mut current)?;
    let mut index: u64 = 0;
    let mut total: u64 = 0;
    loop {
        // A chunk is final when nothing follows it.
        let next_len = if current_len == size {
            read_full(&mut src, &mut next)?
        } else {
            0
        };
        let last = next_len == 0;
        let nonce = nonce_for(index, last);
        let ct = cipher
            .encrypt(
                &Nonce::try_from(&nonce[..]).unwrap(),
                Payload {
                    msg: &current[..current_len],
                    aad: &header_bytes,
                },
            )
            .map_err(|_| StreamError::Format("encryption failed"))?;
        dst.write_all(&ct)?;
        total += current_len as u64;
        if last {
            break;
        }
        std::mem::swap(&mut current, &mut next);
        current_len = next_len;
        index = index
            .checked_add(1)
            .ok_or(StreamError::Format("object too large"))?;
    }
    dst.flush()?;
    Ok(total)
}

/// Reads and validates the header of an encrypted object.
pub fn read_header<R: Read>(src: &mut R) -> Result<StreamHeader, StreamError> {
    let mut bytes = [0u8; HEADER_LEN];
    if read_full(src, &mut bytes)? != HEADER_LEN {
        return Err(StreamError::Format("truncated header"));
    }
    StreamHeader::parse(&bytes)
}

/// Decrypts the chunks that follow `header` (already read with [`read_header`]) into
/// `dst`. Returns the plaintext length.
///
/// Each chunk is authenticated before it is written, but an object is only known to be
/// complete once this returns `Ok`: on error, discard whatever was written to `dst`.
pub fn decrypt_stream<R: Read, W: Write>(
    key: &cipher::Key,
    header: &StreamHeader,
    mut src: R,
    mut dst: W,
) -> Result<u64, StreamError> {
    let header_bytes = header.to_bytes();
    let cipher = Aes256Gcm::new(key.as_bytes().into());
    let full = header.chunk_size as usize + TAG_LEN;

    let mut current = vec![0u8; full];
    let mut next = vec![0u8; full];
    let mut current_len = read_full(&mut src, &mut current)?;
    let mut index: u64 = 0;
    let mut total: u64 = 0;
    loop {
        if current_len < TAG_LEN {
            // Nothing (or a fragment) where a chunk was required: truncated.
            return Err(StreamError::Tampered);
        }
        let next_len = if current_len == full {
            read_full(&mut src, &mut next)?
        } else {
            0
        };
        let last = next_len == 0;
        let nonce = nonce_for(index, last);
        let plain = Zeroizing::new(
            cipher
                .decrypt(
                    &Nonce::try_from(&nonce[..]).unwrap(),
                    Payload {
                        msg: &current[..current_len],
                        aad: &header_bytes,
                    },
                )
                .map_err(|_| StreamError::Tampered)?,
        );
        dst.write_all(&plain)?;
        total += plain.len() as u64;
        if last {
            break;
        }
        std::mem::swap(&mut current, &mut next);
        current_len = next_len;
        index = index.checked_add(1).ok_or(StreamError::Tampered)?;
    }
    dst.flush()?;
    Ok(total)
}

/// Pull-based counterpart of [`decrypt_stream`]: a [`Read`] that yields the plaintext of
/// an encrypted object chunk by chunk, authenticating each chunk before serving it.
///
/// The same caveat applies as for [`decrypt_stream`]: the object is only known to be
/// complete once EOF (`Ok(0)`) is reached; an error means discard everything read so far.
pub struct DecryptReader<R: Read> {
    cipher: Aes256Gcm,
    header_bytes: [u8; HEADER_LEN],
    full: usize,
    src: R,
    /// Ciphertext of the chunk to decrypt next.
    current: Vec<u8>,
    current_len: usize,
    /// Decrypted chunk being served, and how much of it was consumed.
    plain: Zeroizing<Vec<u8>>,
    pos: usize,
    index: u64,
    done: bool,
}

impl<R: Read> DecryptReader<R> {
    /// Starts reading the chunks that follow `header` (already read with [`read_header`]).
    pub fn new(key: &cipher::Key, header: &StreamHeader, mut src: R) -> Result<Self, StreamError> {
        let full = header.chunk_size as usize + TAG_LEN;
        let mut current = vec![0u8; full];
        let current_len = read_full(&mut src, &mut current)?;
        Ok(Self {
            cipher: Aes256Gcm::new(key.as_bytes().into()),
            header_bytes: header.to_bytes(),
            full,
            src,
            current,
            current_len,
            plain: Zeroizing::new(Vec::new()),
            pos: 0,
            index: 0,
            done: false,
        })
    }

    /// Decrypts the staged chunk into `self.plain` (mirrors one `decrypt_stream` step).
    fn fill(&mut self) -> Result<(), StreamError> {
        if self.current_len < TAG_LEN {
            return Err(StreamError::Tampered);
        }
        let mut next = vec![0u8; self.full];
        let next_len = if self.current_len == self.full {
            read_full(&mut self.src, &mut next)?
        } else {
            0
        };
        let last = next_len == 0;
        let nonce = nonce_for(self.index, last);
        let plain = self
            .cipher
            .decrypt(
                &Nonce::try_from(&nonce[..]).unwrap(),
                Payload {
                    msg: &self.current[..self.current_len],
                    aad: &self.header_bytes,
                },
            )
            .map_err(|_| StreamError::Tampered)?;
        self.plain = Zeroizing::new(plain);
        self.pos = 0;
        if last {
            self.done = true;
        } else {
            self.current = next;
            self.current_len = next_len;
            self.index = self.index.checked_add(1).ok_or(StreamError::Tampered)?;
        }
        Ok(())
    }
}

impl<R: Read> Read for DecryptReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.pos == self.plain.len() {
            if self.done {
                return Ok(0);
            }
            self.fill().map_err(|e| match e {
                StreamError::Io(e) => e,
                e => io::Error::other(e),
            })?;
        }
        let n = buf.len().min(self.plain.len() - self.pos);
        buf[..n].copy_from_slice(&self.plain[self.pos..self.pos + n]);
        self.pos += n;
        // An empty final chunk (empty object) serves 0 bytes and is EOF at once.
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand_core::{OsRng, RngCore};

    fn key() -> cipher::Key {
        let mut k = [0u8; 32];
        OsRng.fill_bytes(&mut k);
        cipher::Key::from_slice(&k).unwrap()
    }

    fn header(chunk: u32) -> StreamHeader {
        let mut id = [0u8; 16];
        OsRng.fill_bytes(&mut id);
        StreamHeader {
            kind: ObjectKind::File,
            object_id: id,
            chunk_size: chunk,
        }
    }

    fn enc(k: &cipher::Key, h: &StreamHeader, data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        assert_eq!(
            encrypt_stream(k, h, data, &mut out).unwrap(),
            data.len() as u64
        );
        out
    }

    fn dec(k: &cipher::Key, blob: &[u8]) -> Result<Vec<u8>, StreamError> {
        let mut src = blob;
        let h = read_header(&mut src)?;
        let mut out = Vec::new();
        decrypt_stream(k, &h, src, &mut out)?;
        Ok(out)
    }

    fn data(len: usize) -> Vec<u8> {
        let mut d = vec![0u8; len];
        OsRng.fill_bytes(&mut d);
        d
    }

    #[test]
    fn roundtrips_all_boundary_sizes() {
        let k = key();
        let h = header(16);
        for len in [0, 1, 15, 16, 17, 31, 32, 33, 160, 161] {
            let d = data(len);
            let blob = enc(&k, &h, &d);
            // header + one tag per chunk (an empty object is one empty final chunk)
            let chunks = if len == 0 { 1 } else { len.div_ceil(16) };
            assert_eq!(blob.len(), HEADER_LEN + len + chunks * TAG_LEN, "len {len}");
            assert_eq!(dec(&k, &blob).unwrap(), d, "len {len}");
        }
    }

    #[test]
    fn standard_chunk_size_roundtrip() {
        let k = key();
        let mut id = [0u8; 16];
        OsRng.fill_bytes(&mut id);
        let h = StreamHeader::new(ObjectKind::Index, id);
        let d = data(CHUNK_SIZE as usize * 2 + 12345);
        assert_eq!(dec(&k, &enc(&k, &h, &d)).unwrap(), d);
    }

    #[test]
    fn wrong_key_fails() {
        let h = header(16);
        let blob = enc(&key(), &h, &data(40));
        assert!(matches!(dec(&key(), &blob), Err(StreamError::Tampered)));
    }

    #[test]
    fn every_flipped_byte_is_detected() {
        let k = key();
        let blob = enc(&k, &header(16), &data(50));
        for i in 0..blob.len() {
            let mut bad = blob.clone();
            bad[i] ^= 0x01;
            assert!(dec(&k, &bad).is_err(), "byte {i}");
        }
    }

    #[test]
    fn truncation_at_any_point_is_detected() {
        let k = key();
        let blob = enc(&k, &header(16), &data(50));
        for cut in 0..blob.len() {
            assert!(dec(&k, &blob[..cut]).is_err(), "cut at {cut}");
        }
    }

    #[test]
    fn dropping_whole_final_chunks_is_detected() {
        let k = key();
        let h = header(16);
        // 3 full chunks of 16: dropping the last whole chunk leaves a full chunk that
        // was not encrypted as final.
        let blob = enc(&k, &h, &data(48));
        let chunk = 16 + TAG_LEN;
        assert!(dec(&k, &blob[..blob.len() - chunk]).is_err());
    }

    #[test]
    fn reordered_chunks_are_detected() {
        let k = key();
        let blob = enc(&k, &header(16), &data(64));
        let chunk = 16 + TAG_LEN;
        let mut swapped = blob[..HEADER_LEN].to_vec();
        swapped.extend_from_slice(&blob[HEADER_LEN + chunk..HEADER_LEN + 2 * chunk]);
        swapped.extend_from_slice(&blob[HEADER_LEN..HEADER_LEN + chunk]);
        swapped.extend_from_slice(&blob[HEADER_LEN + 2 * chunk..]);
        assert!(dec(&k, &swapped).is_err());
    }

    #[test]
    fn trailing_data_is_detected() {
        let k = key();
        let mut blob = enc(&k, &header(16), &data(20));
        blob.extend_from_slice(&[0u8; 5]);
        assert!(dec(&k, &blob).is_err());
    }

    #[test]
    fn header_from_another_object_is_detected() {
        let k = key();
        let blob = enc(&k, &header(16), &data(20));
        let mut other = header(16).to_bytes().to_vec();
        other.extend_from_slice(&blob[HEADER_LEN..]);
        assert!(dec(&k, &other).is_err());
    }

    fn dec_via_reader(k: &cipher::Key, blob: &[u8]) -> Result<Vec<u8>, io::Error> {
        let mut src = blob;
        let h = read_header(&mut src).map_err(io::Error::other)?;
        let mut r = DecryptReader::new(k, &h, src).map_err(io::Error::other)?;
        let mut out = Vec::new();
        // Tiny reads on purpose, to cross chunk boundaries mid-call.
        let mut buf = [0u8; 7];
        loop {
            let n = r.read(&mut buf)?;
            if n == 0 {
                return Ok(out);
            }
            out.extend_from_slice(&buf[..n]);
        }
    }

    #[test]
    fn reader_roundtrips_and_detects_tampering() {
        let k = key();
        let h = header(16);
        for len in [0, 1, 15, 16, 17, 33, 160] {
            let d = data(len);
            let blob = enc(&k, &h, &d);
            assert_eq!(dec_via_reader(&k, &blob).unwrap(), d, "len {len}");
        }
        let blob = enc(&k, &h, &data(50));
        for cut in HEADER_LEN..blob.len() {
            assert!(dec_via_reader(&k, &blob[..cut]).is_err(), "cut {cut}");
        }
        let mut bad = blob.clone();
        bad[HEADER_LEN + 3] ^= 1;
        assert!(dec_via_reader(&k, &bad).is_err());
        assert!(dec_via_reader(&key(), &blob).is_err());
    }

    #[test]
    fn hostile_headers_are_rejected_before_allocation() {
        let mut h = header(16).to_bytes();
        h[21..25].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(matches!(
            StreamHeader::parse(&h),
            Err(StreamError::Format("unsupported chunk size"))
        ));
        let mut h = header(16).to_bytes();
        h[0] = b'X';
        assert!(StreamHeader::parse(&h).is_err());
        let mut h = header(16).to_bytes();
        h[4] = 9;
        assert!(StreamHeader::parse(&h).is_err());
    }
}
