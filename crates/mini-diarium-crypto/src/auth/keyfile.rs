use crate::auth::{keypair, SecretBytes};
use crate::crypto::cipher;
use hkdf::Hkdf;
use rand_core::{OsRng, RngCore};
use sha2::Sha256;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Magic prefix of a v2 key-file slot blob.
const V2_MAGIC: &[u8; 4] = b"MDKF";
const V2_VERSION: u8 = 2;
/// magic(4) + version(1)
const V2_HEADER_LEN: usize = 5;
/// header + nonce(12) + encrypted 32-byte master key + tag(16)
const V2_BLOB_LEN: usize = V2_HEADER_LEN + 12 + 32 + 16;

const SLOT_ID_INFO: &[u8] = b"mini-diarium:keyfile:slot-id:v2";
const WRAP_KEY_INFO: &[u8] = b"mini-diarium:keyfile:wrap:v2";

/// Key-file auth method, slot format v2: symmetric and quantum-resistant.
///
/// A key file holds 32 random secret bytes (64 hex characters). The slot stores:
/// - in `auth_slots.public_key`: a **slot id**, HKDF-SHA256(secret, info = slot-id), used
///   only to find the slot. It is a one-way function of the secret and reveals nothing
///   usable to unwrap the key.
/// - in `auth_slots.wrapped_key`: `["MDKF"][0x02][nonce: 12][encrypted master key][tag: 16]`,
///   AES-256-GCM under HKDF-SHA256(secret, info = wrap), with the header and slot id as
///   associated data.
///
/// Nothing in the slot is a public key, so unlike the legacy X25519 slots
/// ([`keypair::KeypairMethod`]) there is no discrete-log problem for a quantum computer to
/// solve: unwrapping needs the file. Security rests on AES-256 and HMAC-SHA256 only.
///
/// Existing key files (an X25519 private key) are also 32 random bytes, so the same file is
/// accepted as a v2 secret: an old key file keeps working after its slot is upgraded.
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct KeyFileMethod {
    secret: [u8; 32],
}

impl KeyFileMethod {
    /// Uses the 32 bytes read from a key file (v2 or a legacy X25519 private key).
    pub fn from_secret(secret: [u8; 32]) -> Self {
        Self { secret }
    }

    /// Generates a new random key-file secret.
    pub fn generate() -> Self {
        let mut secret = [0u8; 32];
        OsRng.fill_bytes(&mut secret);
        Self { secret }
    }

    /// The key-file contents: the secret as 64 lowercase hex characters.
    pub fn file_contents(&self) -> zeroize::Zeroizing<String> {
        zeroize::Zeroizing::new(hex::encode(self.secret))
    }

    /// The slot id stored in `auth_slots.public_key` to find this key file's slot.
    pub fn slot_id(&self) -> [u8; 32] {
        self.expand(SLOT_ID_INFO)
    }

    /// The X25519 public key this file would have had as a legacy key file, used to find a
    /// legacy slot so it can be upgraded.
    pub fn legacy_public_key(&self) -> [u8; 32] {
        keypair::derive_public_key(self.secret)
    }

    fn expand(&self, info: &[u8]) -> [u8; 32] {
        let hk = Hkdf::<Sha256>::new(None, &self.secret);
        let mut out = [0u8; 32];
        hk.expand(info, &mut out)
            .expect("HKDF expand: 32 bytes always fits");
        out
    }

    fn aad(&self) -> Vec<u8> {
        let mut aad = Vec::with_capacity(V2_HEADER_LEN + 32);
        aad.extend_from_slice(V2_MAGIC);
        aad.push(V2_VERSION);
        aad.extend_from_slice(&self.slot_id());
        aad
    }

    /// Returns true if `wrapped` is a v2 key-file slot blob (a legacy X25519 blob is 92
    /// bytes and has no magic).
    pub fn is_v2_blob(wrapped: &[u8]) -> bool {
        wrapped.len() == V2_BLOB_LEN && wrapped.starts_with(V2_MAGIC) && wrapped[4] == V2_VERSION
    }

    /// Wraps the master key. Returns a v2 `wrapped_key` blob.
    pub fn wrap_master_key(&self, master_key: &[u8]) -> Result<Vec<u8>, String> {
        let mut wrap_bytes = self.expand(WRAP_KEY_INFO);
        let wrap_key = cipher::Key::from_slice(&wrap_bytes).ok_or("Invalid wrapping key size")?;
        wrap_bytes.zeroize();
        let encrypted = cipher::encrypt_with_aad(&wrap_key, master_key, &self.aad())
            .map_err(|e| format!("Failed to wrap master key: {}", e))?;
        let mut blob = Vec::with_capacity(V2_HEADER_LEN + encrypted.len());
        blob.extend_from_slice(V2_MAGIC);
        blob.push(V2_VERSION);
        blob.extend_from_slice(&encrypted);
        Ok(blob)
    }

    /// Unwraps the master key from a v2 blob.
    pub fn unwrap_master_key(&self, wrapped: &[u8]) -> Result<SecretBytes, String> {
        if !Self::is_v2_blob(wrapped) {
            return Err("Invalid key-file slot".to_string());
        }
        let mut wrap_bytes = self.expand(WRAP_KEY_INFO);
        let wrap_key = cipher::Key::from_slice(&wrap_bytes).ok_or("Invalid wrapping key size")?;
        wrap_bytes.zeroize();
        cipher::decrypt_with_aad(&wrap_key, &wrapped[V2_HEADER_LEN..], &self.aad())
            .map(SecretBytes)
            .map_err(|_| {
                "Failed to decrypt master key: wrong key file or corrupted data".to_string()
            })
    }

    /// Unwraps a key-file slot of either format: v2 directly, or a legacy X25519 slot by
    /// using the file's bytes as the X25519 private key.
    pub fn unwrap_any(&self, wrapped: &[u8]) -> Result<SecretBytes, String> {
        if Self::is_v2_blob(wrapped) {
            self.unwrap_master_key(wrapped)
        } else {
            keypair::PrivateKeyMethod {
                private_key: self.secret,
            }
            .unwrap_master_key(wrapped)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn master_key() -> Vec<u8> {
        let mut k = vec![0u8; 32];
        OsRng.fill_bytes(&mut k);
        k
    }

    #[test]
    fn roundtrip_and_format() {
        let mk = master_key();
        let kf = KeyFileMethod::generate();
        let blob = kf.wrap_master_key(&mk).unwrap();
        assert!(KeyFileMethod::is_v2_blob(&blob));
        assert_eq!(kf.unwrap_master_key(&blob).unwrap(), mk);
        assert_eq!(kf.unwrap_any(&blob).unwrap(), mk);
    }

    #[test]
    fn wrong_file_fails() {
        let blob = KeyFileMethod::generate()
            .wrap_master_key(&master_key())
            .unwrap();
        assert!(KeyFileMethod::generate().unwrap_master_key(&blob).is_err());
    }

    #[test]
    fn tampering_fails() {
        let kf = KeyFileMethod::generate();
        let blob = kf.wrap_master_key(&master_key()).unwrap();
        for i in [0, 4, 5, 20, blob.len() - 1] {
            let mut bad = blob.clone();
            bad[i] ^= 0x01;
            assert!(kf.unwrap_master_key(&bad).is_err(), "byte {i}");
        }
    }

    #[test]
    fn blob_reveals_no_secret_material() {
        let mk = master_key();
        let kf = KeyFileMethod::generate();
        let blob = kf.wrap_master_key(&mk).unwrap();
        assert!(!blob
            .windows(32)
            .any(|w| w == kf.secret || w == mk.as_slice()));
        assert_ne!(kf.slot_id(), kf.secret);
        assert_ne!(kf.slot_id(), kf.legacy_public_key());
    }

    #[test]
    fn legacy_key_file_bytes_open_legacy_and_v2_slots() {
        let mk = master_key();
        let kp = keypair::generate_keypair().unwrap();
        let mut private = [0u8; 32];
        private.copy_from_slice(&hex::decode(&kp.private_key_hex).unwrap());
        let mut public = [0u8; 32];
        public.copy_from_slice(&hex::decode(&kp.public_key_hex).unwrap());

        let legacy_blob = keypair::KeypairMethod { public_key: public }
            .wrap_master_key(&mk)
            .unwrap();
        assert!(!KeyFileMethod::is_v2_blob(&legacy_blob));

        let kf = KeyFileMethod::from_secret(private);
        assert_eq!(kf.legacy_public_key(), public);
        assert_eq!(kf.unwrap_any(&legacy_blob).unwrap(), mk);

        // The same old file opens the upgraded slot.
        let v2 = kf.wrap_master_key(&mk).unwrap();
        assert_eq!(kf.unwrap_any(&v2).unwrap(), mk);
    }

    #[test]
    fn file_contents_roundtrip() {
        let kf = KeyFileMethod::generate();
        let text = kf.file_contents();
        assert_eq!(text.len(), 64);
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&hex::decode(text.as_str()).unwrap());
        assert_eq!(KeyFileMethod::from_secret(bytes).slot_id(), kf.slot_id());
    }
}
