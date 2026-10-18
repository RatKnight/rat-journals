use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Algorithm, Argon2, Params, Version,
};
use zeroize::Zeroize;

/// Argon2id parameters of the legacy PHC-string hashes (`hash_password`): m=64 MiB, t=3,
/// p=4. Only used to read and upgrade pre-v2 journals.
const MEMORY_SIZE_KIB: u32 = 65536; // 64 MB
const ITERATIONS: u32 = 3;
const PARALLELISM: u32 = 4;

/// Argon2id parameters for every new password slot: m=256 MiB, t=4, p=4. Under one
/// second per unlock on a desktop (about 0.5 s measured), and each guess needs 256 MiB of fast memory, which
/// sharply limits how many guesses a GPU can run in parallel.
const SLOT_MEMORY_KIB: u32 = 262_144; // 256 MB
const SLOT_ITERATIONS: u32 = 4;
const SLOT_PARALLELISM: u32 = 4;

/// Error type for password operations
#[derive(Debug)]
pub enum PasswordError {
    HashingFailed(String),
    VerificationFailed(String),
    InvalidHash(String),
}

impl std::fmt::Display for PasswordError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PasswordError::HashingFailed(msg) => write!(f, "Password hashing failed: {}", msg),
            PasswordError::VerificationFailed(msg) => {
                write!(f, "Password verification failed: {}", msg)
            }
            PasswordError::InvalidHash(msg) => write!(f, "Invalid password hash: {}", msg),
        }
    }
}

impl std::error::Error for PasswordError {}

/// Generates a cryptographically secure random salt
pub fn generate_salt() -> SaltString {
    SaltString::generate(&mut OsRng)
}

/// Hashes a password using Argon2id with specified parameters
///
/// # Arguments
/// * `password` - The password to hash (will be zeroized after use)
/// * `salt` - The salt to use for hashing
///
/// # Returns
/// The password hash in PHC string format
pub fn hash_password(mut password: String, salt: &SaltString) -> Result<String, PasswordError> {
    // Create Argon2id instance with our parameters
    let params = Params::new(MEMORY_SIZE_KIB, ITERATIONS, PARALLELISM, None)
        .map_err(|e| PasswordError::HashingFailed(format!("Invalid parameters: {}", e)))?;

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    // Hash the password
    let password_hash = argon2
        .hash_password(password.as_bytes(), salt)
        .map_err(|e| PasswordError::HashingFailed(e.to_string()))?;

    // Zeroize the password from memory
    password.zeroize();

    Ok(password_hash.to_string())
}

/// Argon2id cost parameters as stored in a v2 password slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KdfParams {
    pub m_cost_kib: u32,
    pub t_cost: u32,
    pub p_cost: u32,
}

impl KdfParams {
    /// The parameters used for every newly written password slot.
    pub const DEFAULT: KdfParams = KdfParams {
        m_cost_kib: SLOT_MEMORY_KIB,
        t_cost: SLOT_ITERATIONS,
        p_cost: SLOT_PARALLELISM,
    };

    /// True when these parameters cost less per guess than [`KdfParams::DEFAULT`].
    pub fn weaker_than_default(&self) -> bool {
        self.m_cost_kib < Self::DEFAULT.m_cost_kib || self.t_cost < Self::DEFAULT.t_cost
    }

    /// Rejects parameters read from disk that are weaker than the OWASP minimum
    /// (m=19 MiB, t=2) or large enough to exhaust memory (m > 1 GiB) when a crafted
    /// journal or backup is opened.
    pub fn validate(&self) -> Result<(), String> {
        if !(19_456..=1_048_576).contains(&self.m_cost_kib)
            || !(2..=32).contains(&self.t_cost)
            || !(1..=16).contains(&self.p_cost)
        {
            return Err("Unsupported password slot parameters".to_string());
        }
        Ok(())
    }
}

/// Derives a 32-byte wrapping key from a password and salt with raw Argon2id.
///
/// Unlike a PHC hash string, the output is never written to disk: only the salt and
/// parameters are stored, so the key can be recomputed only by someone who knows the
/// password.
pub fn derive_wrapping_key(
    password: &[u8],
    salt: &[u8],
    params: KdfParams,
) -> Result<zeroize::Zeroizing<[u8; 32]>, String> {
    params.validate()?;
    let argon_params = Params::new(params.m_cost_kib, params.t_cost, params.p_cost, Some(32))
        .map_err(|e| format!("Invalid Argon2 parameters: {}", e))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, argon_params);
    let mut out = zeroize::Zeroizing::new([0u8; 32]);
    argon2
        .hash_password_into(password, salt, out.as_mut())
        .map_err(|e| format!("Key derivation failed: {}", e))?;
    Ok(out)
}

/// Derives a 32-byte encryption key from a PHC hash string (Argon2 output)
///
/// **Legacy only.** The PHC string contains the hash itself, so a key taken from it is
/// only as secret as the string. It is kept solely to read v1/v2 journals and
/// pre-v2 password slots so they can be upgraded; never use it for new data.
pub fn derive_key_from_phc_hash(phc_hash: &str) -> Result<Vec<u8>, String> {
    let parsed =
        PasswordHash::new(phc_hash).map_err(|e| format!("Failed to parse password hash: {}", e))?;
    let hash_value = parsed
        .hash
        .ok_or_else(|| "No hash in password hash string".to_string())?;
    let hash_bytes = hash_value.as_bytes().to_vec();
    if hash_bytes.len() < 32 {
        return Err("Hash too short for key derivation".to_string());
    }
    Ok(hash_bytes[..32].to_vec())
}

/// Verifies a password against a stored hash
///
/// # Arguments
/// * `password` - The password to verify (will be zeroized after use)
/// * `hash_string` - The stored password hash in PHC string format
///
/// # Returns
/// `Ok(())` if the password matches, `Err` otherwise
pub fn verify_password(mut password: String, hash_string: &str) -> Result<(), PasswordError> {
    // Parse the stored hash
    let parsed_hash =
        PasswordHash::new(hash_string).map_err(|e| PasswordError::InvalidHash(e.to_string()))?;

    // Create Argon2 instance
    let argon2 = Argon2::default();

    // Verify the password
    let result = argon2
        .verify_password(password.as_bytes(), &parsed_hash)
        .map_err(|e| PasswordError::VerificationFailed(e.to_string()));

    // Zeroize the password from memory
    password.zeroize();

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_salt() {
        let salt1 = generate_salt();
        let salt2 = generate_salt();

        // Salts should be different
        assert_ne!(salt1.as_str(), salt2.as_str());

        // Salts should be the expected length (22 characters in base64)
        assert_eq!(salt1.as_str().len(), 22);
        assert_eq!(salt2.as_str().len(), 22);
    }

    #[test]
    fn test_hash_password_success() {
        let password = "my_secure_password_123".to_string();
        let salt = generate_salt();

        let result = hash_password(password, &salt);
        assert!(result.is_ok());

        let hash = result.unwrap();
        // Hash should start with $argon2id$ prefix
        assert!(hash.starts_with("$argon2id$"));
        // Hash should contain our parameters
        assert!(hash.contains("m=65536"));
        assert!(hash.contains("t=3"));
        assert!(hash.contains("p=4"));
    }

    #[test]
    fn test_hash_password_deterministic() {
        let password1 = "same_password".to_string();
        let password2 = "same_password".to_string();
        let salt = generate_salt();

        let hash1 = hash_password(password1, &salt).unwrap();
        let hash2 = hash_password(password2, &salt).unwrap();

        // Same password and salt should produce the same hash
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_hash_password_different_salts() {
        let password1 = "same_password".to_string();
        let password2 = "same_password".to_string();
        let salt1 = generate_salt();
        let salt2 = generate_salt();

        let hash1 = hash_password(password1, &salt1).unwrap();
        let hash2 = hash_password(password2, &salt2).unwrap();

        // Same password with different salts should produce different hashes
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_verify_password_correct() {
        let password = "correct_password".to_string();
        let salt = generate_salt();
        let hash = hash_password(password.clone(), &salt).unwrap();

        let result = verify_password(password, &hash);
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_password_incorrect() {
        let password = "correct_password".to_string();
        let wrong_password = "wrong_password".to_string();
        let salt = generate_salt();
        let hash = hash_password(password, &salt).unwrap();

        let result = verify_password(wrong_password, &hash);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            PasswordError::VerificationFailed(_)
        ));
    }

    #[test]
    fn test_verify_password_invalid_hash() {
        let password = "some_password".to_string();
        let invalid_hash = "not_a_valid_hash";

        let result = verify_password(password, invalid_hash);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PasswordError::InvalidHash(_)));
    }

    #[test]
    fn test_verify_password_empty() {
        let password = "".to_string();
        let salt = generate_salt();
        let hash = hash_password(password.clone(), &salt).unwrap();

        let result = verify_password(password, &hash);
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_password_unicode() {
        let password = "пароль🔐密码".to_string();
        let salt = generate_salt();
        let hash = hash_password(password.clone(), &salt).unwrap();

        let result = verify_password(password, &hash);
        assert!(result.is_ok());
    }

    #[test]
    fn test_hash_long_password() {
        let password = "a".repeat(1000);
        let salt = generate_salt();

        let result = hash_password(password, &salt);
        assert!(result.is_ok());
    }
}
