use crate::auth::SecretBytes;
use crate::crypto::{cipher, password as pwd};
use rand_core::{OsRng, RngCore};
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Magic prefix of a v2 password slot blob.
///
/// A legacy blob starts with a little-endian `u32` PHC-string length (~100), so its first
/// four bytes can never equal this value (`0x5750444D` as a `u32`).
const V2_MAGIC: &[u8; 4] = b"MDPW";
const V2_VERSION: u8 = 2;
const V2_SALT_LEN: usize = 16;
/// magic(4) + version(1) + m_cost(4) + t_cost(4) + p_cost(4) + salt(16)
const V2_HEADER_LEN: usize = 4 + 1 + 12 + V2_SALT_LEN;

/// Auth method that wraps/unwraps the master key using a password (Argon2id + AES-256-GCM).
///
/// The v2 `wrapped_key` blob stored in `auth_slots.wrapped_key`:
///   ["MDPW"][version: 0x02][m_cost: u32 LE][t_cost: u32 LE][p_cost: u32 LE][salt: 16 bytes]
///   [nonce: 12 bytes][encrypted master key][tag: 16 bytes]
///
/// The wrapping key is the raw Argon2id output for (password, salt, params) and is never
/// stored. The header is authenticated as AES-GCM associated data, and a wrong password is
/// detected by the GCM tag check.
///
/// Legacy blobs (`[phc_len: u32 LE][phc_hash][nonce][ciphertext][tag]`) derived the
/// wrapping key from the hash embedded in the PHC string, so anyone holding the blob could
/// unwrap the master key without the password. They are still readable so that existing
/// journals can be upgraded (see [`PasswordMethod::is_legacy_blob`]), but are never written.
#[derive(ZeroizeOnDrop)]
pub struct PasswordMethod {
    pub password: String,
}

impl PasswordMethod {
    pub fn new(password: String) -> Self {
        Self { password }
    }

    /// Returns true if `wrapped` uses the legacy format whose wrapping key is recoverable
    /// from the blob alone. A master key that was ever stored this way must be treated as
    /// exposed and rotated, not merely re-wrapped.
    pub fn is_legacy_blob(wrapped: &[u8]) -> bool {
        !wrapped.starts_with(V2_MAGIC)
    }

    /// Wraps the master key for storage. Returns a v2 `wrapped_key` blob.
    pub fn wrap_master_key(&self, master_key: &[u8]) -> Result<Vec<u8>, String> {
        let params = pwd::KdfParams::DEFAULT;
        let mut salt = [0u8; V2_SALT_LEN];
        OsRng.fill_bytes(&mut salt);

        let mut header = Vec::with_capacity(V2_HEADER_LEN);
        header.extend_from_slice(V2_MAGIC);
        header.push(V2_VERSION);
        header.extend_from_slice(&params.m_cost_kib.to_le_bytes());
        header.extend_from_slice(&params.t_cost.to_le_bytes());
        header.extend_from_slice(&params.p_cost.to_le_bytes());
        header.extend_from_slice(&salt);

        let key_bytes = pwd::derive_wrapping_key(self.password.as_bytes(), &salt, params)?;
        let wrap_key =
            cipher::Key::from_slice(key_bytes.as_ref()).ok_or("Invalid wrapping key size")?;
        drop(key_bytes); // Zeroizing: wiped here

        let encrypted = cipher::encrypt_with_aad(&wrap_key, master_key, &header)
            .map_err(|e| format!("Failed to encrypt master key: {}", e))?;

        let mut result = header;
        result.extend_from_slice(&encrypted);
        Ok(result)
    }

    /// Unwraps the master key from a `wrapped_key` blob (v2 or legacy) using the password.
    ///
    /// Returns `Err("Incorrect password")` if the password is wrong.
    pub fn unwrap_master_key(&self, wrapped: &[u8]) -> Result<SecretBytes, String> {
        if Self::is_legacy_blob(wrapped) {
            self.unwrap_legacy(wrapped)
        } else {
            self.unwrap_v2(wrapped)
        }
    }

    fn unwrap_v2(&self, wrapped: &[u8]) -> Result<SecretBytes, String> {
        if wrapped.len() < V2_HEADER_LEN + 12 + 16 {
            return Err("Invalid wrapped key: too short".to_string());
        }
        if wrapped[4] != V2_VERSION {
            return Err("Unsupported password slot version".to_string());
        }
        let read_u32 = |at: usize| u32::from_le_bytes(wrapped[at..at + 4].try_into().unwrap());
        let params = pwd::KdfParams {
            m_cost_kib: read_u32(5),
            t_cost: read_u32(9),
            p_cost: read_u32(13),
        };
        let (header, encrypted) = wrapped.split_at(V2_HEADER_LEN);
        let salt = &header[17..];

        let key_bytes = pwd::derive_wrapping_key(self.password.as_bytes(), salt, params)?;
        let wrap_key =
            cipher::Key::from_slice(key_bytes.as_ref()).ok_or("Invalid wrapping key size")?;
        drop(key_bytes);

        cipher::decrypt_with_aad(&wrap_key, encrypted, header)
            .map(SecretBytes)
            .map_err(|_| "Incorrect password".to_string())
    }

    fn unwrap_legacy(&self, wrapped: &[u8]) -> Result<SecretBytes, String> {
        if wrapped.len() < 4 {
            return Err("Invalid wrapped key: too short".to_string());
        }

        // Parse phc_hash length
        let phc_len = u32::from_le_bytes(
            wrapped[..4]
                .try_into()
                .map_err(|_| "Invalid wrapped key header".to_string())?,
        ) as usize;

        if wrapped.len() < 4 + phc_len {
            return Err("Invalid wrapped key: truncated phc hash".to_string());
        }

        let phc_hash = std::str::from_utf8(&wrapped[4..4 + phc_len])
            .map_err(|_| "Invalid wrapped key: non-UTF8 phc hash".to_string())?;

        let encrypted_master = &wrapped[4 + phc_len..];

        // Verify the password against the embedded phc hash
        pwd::verify_password(self.password.clone(), phc_hash)
            .map_err(|_| "Incorrect password".to_string())?;

        // Derive wrapping key from the phc hash
        let mut wrapping_key_bytes =
            pwd::derive_key_from_phc_hash(phc_hash).map_err(|e| e.to_string())?;
        let wrap_key =
            cipher::Key::from_slice(&wrapping_key_bytes).ok_or("Invalid wrapping key size")?;

        // Decrypt master_key
        let master_key = cipher::decrypt(&wrap_key, encrypted_master);
        wrapping_key_bytes.zeroize(); // always runs
        let master_key = master_key.map_err(|_| "Incorrect password".to_string())?;

        Ok(SecretBytes(master_key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn random_master_key() -> Vec<u8> {
        let mut key = vec![0u8; 32];
        OsRng.fill_bytes(&mut key);
        key
    }

    /// Builds a blob in the pre-v2 format, exactly as older releases wrote it.
    fn legacy_wrap(password: &str, master_key: &[u8]) -> Vec<u8> {
        let salt = pwd::generate_salt();
        let phc_hash = pwd::hash_password(password.to_string(), &salt).unwrap();
        let key = pwd::derive_key_from_phc_hash(&phc_hash).unwrap();
        let wrap_key = cipher::Key::from_slice(&key).unwrap();
        let encrypted = cipher::encrypt(&wrap_key, master_key).unwrap();
        let mut blob = (phc_hash.len() as u32).to_le_bytes().to_vec();
        blob.extend_from_slice(phc_hash.as_bytes());
        blob.extend_from_slice(&encrypted);
        blob
    }

    #[test]
    fn test_wrap_unwrap_roundtrip() {
        let master_key = random_master_key();
        let method = PasswordMethod::new("my_secure_password".to_string());

        let wrapped = method.wrap_master_key(&master_key).unwrap();
        let recovered = method.unwrap_master_key(&wrapped).unwrap();

        assert_eq!(master_key, recovered);
    }

    #[test]
    fn test_wrong_password_fails() {
        let master_key = random_master_key();
        let method = PasswordMethod::new("correct_password".to_string());
        let wrapped = method.wrap_master_key(&master_key).unwrap();

        let wrong_method = PasswordMethod::new("wrong_password".to_string());
        let result = wrong_method.unwrap_master_key(&wrapped);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Incorrect password");
    }

    #[test]
    fn test_different_wraps_same_password() {
        let master_key = random_master_key();
        let method = PasswordMethod::new("password".to_string());

        let wrapped1 = method.wrap_master_key(&master_key).unwrap();
        let wrapped2 = method.wrap_master_key(&master_key).unwrap();

        // Different salts → different wrapped blobs
        assert_ne!(wrapped1, wrapped2);

        // But both unwrap to the same master_key
        let recovered1 = method.unwrap_master_key(&wrapped1).unwrap();
        let recovered2 = method.unwrap_master_key(&wrapped2).unwrap();
        assert_eq!(recovered1, master_key);
        assert_eq!(recovered2, master_key);
    }

    #[test]
    fn test_tampered_blob_fails() {
        let master_key = random_master_key();
        let method = PasswordMethod::new("password".to_string());

        let mut wrapped = method.wrap_master_key(&master_key).unwrap();
        // Tamper with the last byte (inside the ciphertext)
        *wrapped.last_mut().unwrap() ^= 0xFF;

        let result = method.unwrap_master_key(&wrapped);
        assert!(result.is_err());
    }

    #[test]
    fn test_tampered_header_fails() {
        let master_key = random_master_key();
        let method = PasswordMethod::new("password".to_string());
        let wrapped = method.wrap_master_key(&master_key).unwrap();

        // Flip a salt byte: the derived key changes and the header no longer matches the AAD.
        let mut bad_salt = wrapped.clone();
        bad_salt[20] ^= 0x01;
        assert!(method.unwrap_master_key(&bad_salt).is_err());

        // Raise t_cost: still within bounds, but authenticated as AAD, so it must fail.
        let mut bad_params = wrapped.clone();
        bad_params[9] = 4;
        assert!(method.unwrap_master_key(&bad_params).is_err());
    }

    #[test]
    fn test_out_of_range_params_rejected_before_kdf() {
        let master_key = random_master_key();
        let method = PasswordMethod::new("password".to_string());
        let mut wrapped = method.wrap_master_key(&master_key).unwrap();
        // m_cost = u32::MAX KiB would allocate ~4 TiB if not bounded.
        wrapped[5..9].copy_from_slice(&u32::MAX.to_le_bytes());
        let err = method.unwrap_master_key(&wrapped).unwrap_err();
        assert_eq!(err, "Unsupported password slot parameters");
    }

    #[test]
    fn test_short_blob_fails() {
        let method = PasswordMethod::new("password".to_string());
        let result = method.unwrap_master_key(&[0u8; 3]);
        assert!(result.is_err());
        let result = method.unwrap_master_key(b"MDPW\x02");
        assert!(result.is_err());
    }

    #[test]
    fn test_v2_blob_contains_no_key_material() {
        // The derived wrapping key must not appear anywhere in the stored blob.
        let master_key = random_master_key();
        let method = PasswordMethod::new("password".to_string());
        let wrapped = method.wrap_master_key(&master_key).unwrap();

        let salt = &wrapped[17..V2_HEADER_LEN];
        let key = pwd::derive_wrapping_key(b"password", salt, pwd::KdfParams::DEFAULT).unwrap();
        assert!(!wrapped.windows(32).any(|w| w == key.as_ref()));
        assert!(!wrapped.windows(32).any(|w| w == master_key.as_slice()));
        assert!(!String::from_utf8_lossy(&wrapped).contains("$argon2"));
    }

    #[test]
    fn test_new_blobs_are_v2_and_legacy_is_detected() {
        let master_key = random_master_key();
        let method = PasswordMethod::new("password".to_string());

        let v2 = method.wrap_master_key(&master_key).unwrap();
        assert!(!PasswordMethod::is_legacy_blob(&v2));
        assert_eq!(&v2[..4], V2_MAGIC);

        let legacy = legacy_wrap("password", &master_key);
        assert!(PasswordMethod::is_legacy_blob(&legacy));
    }

    #[test]
    fn test_legacy_blob_still_unwraps_for_upgrade() {
        let master_key = random_master_key();
        let legacy = legacy_wrap("password", &master_key);

        let method = PasswordMethod::new("password".to_string());
        assert_eq!(method.unwrap_master_key(&legacy).unwrap(), master_key);

        let wrong = PasswordMethod::new("nope".to_string());
        assert_eq!(
            wrong.unwrap_master_key(&legacy).unwrap_err(),
            "Incorrect password"
        );
    }
}
