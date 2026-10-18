// The pure cryptographic auth layer (cipher-backed master-key wrapping, keypair helpers, and
// the SecretBytes/KeypairFiles value types) lives in the rusqlite-free `mini-diarium-crypto`
// crate as of open-core M3a (TODO-0082). Re-export it here so the curated core façade
// (crates/mini-diarium-core/API.md) is byte-for-byte unchanged for consumers.
pub use mini_diarium_crypto::auth::{
    derive_public_key, generate_keypair, AutoKeyMethod, KeyFileMethod, KeypairFiles, KeypairMethod,
    PasswordMethod, PrivateKeyMethod, SecretBytes,
};

// Keep the sub-module-qualified paths that the in-core db modules use
// (`crate::auth::{password, keypair, auto_key}::…`) resolving without touching those call
// sites. Sealed to the crate — they are not part of the public façade.
pub(crate) use mini_diarium_crypto::auth::{auto_key, password};
// Production code reaches key files through `KeyFileMethod`; the X25519 module only builds
// legacy slots for tests (`add_keypair_slot` and its fixtures).
#[cfg(any(test, feature = "test-support"))]
pub(crate) use mini_diarium_crypto::auth::keypair;

/// Information about a registered auth method (returned to frontend)
#[derive(Debug, serde::Serialize)]
pub struct AuthMethodInfo {
    pub id: i64,
    pub slot_type: String,
    pub label: String,
    /// X25519 public key fingerprint (hex-encoded), None for password slots
    pub public_key_hex: Option<String>,
    pub created_at: String,
    pub last_used: Option<String>,
}

/// Wraps the current session's master key with a new password and stores it as a
/// `password` auth slot, returning the new slot's row id.
///
/// The master key is read from the open connection (`db.key()`), wrapped with a fresh
/// Argon2id-derived key, and inserted — the wrap→insert order mirrors the historical
/// `register_password` path. Callers own the identity gate, empty-password check, and
/// duplicate-slot check; this performs only the master-key wrap and the insert.
pub fn add_password_slot(
    db: &crate::db::DatabaseConnection,
    label: &str,
    password: &str,
) -> Result<i64, String> {
    if crate::db::uses_key_shares(db) {
        return Err(crate::db::ERR_SHARED_METHODS_FIXED.to_string());
    }
    let method = password::PasswordMethod::new(password.to_string());
    let wrapped_key = method
        .wrap_master_key(db.key().as_bytes())
        .map_err(|e| format!("Failed to wrap master key: {}", e))?;
    let now = chrono::Utc::now().to_rfc3339();
    crate::db::insert_auth_slot(db, "password", label, None, &wrapped_key, &now)
}

/// Re-wraps the journal's password slot with the current (stronger) Argon2id parameters
/// when it was written with weaker ones. Returns true if the slot was rewritten.
///
/// The master key is unchanged: a weak-parameter v2 slot was never exposed, only cheaper
/// to guess, so a re-wrap is enough. `password` must open the slot (it was just used to
/// unlock); a mismatch is an error and nothing is written.
pub fn strengthen_password_slot(
    db: &crate::db::DatabaseConnection,
    password: &str,
) -> Result<bool, String> {
    let Some((slot_id, wrapped)) = crate::db::get_password_slot(db)? else {
        return Ok(false);
    };
    if !PasswordMethod::has_weak_params(&wrapped) {
        return Ok(false);
    }
    let method = PasswordMethod::new(password.to_string());
    let current = method.unwrap_master_key(&wrapped)?;
    if crate::db::uses_key_shares(db) {
        // The slot holds this method's share of the key, not the key: re-wrap the share.
        let rewrapped = method.wrap_master_key(&current)?;
        crate::db::update_auth_slot_wrapped_key(db, slot_id, &rewrapped)?;
        return Ok(true);
    }
    if current[..] != db.key().as_bytes()[..] {
        return Err("The password slot does not hold this journal's key".to_string());
    }
    let rewrapped = method.wrap_master_key(db.key().as_bytes())?;
    crate::db::update_auth_slot_wrapped_key(db, slot_id, &rewrapped)?;
    Ok(true)
}

/// Wraps the current session's master key for a key file (slot format v2, symmetric) and
/// stores it as a `keypair` auth slot, returning the new slot's row id.
///
/// The slot's `public_key` column holds the key file's slot id, not a public key. Callers
/// own the identity gate; this performs only the wrap and the insert.
pub fn add_key_file_slot(
    db: &crate::db::DatabaseConnection,
    label: &str,
    key_file: &KeyFileMethod,
) -> Result<i64, String> {
    if crate::db::uses_key_shares(db) {
        return Err(crate::db::ERR_SHARED_METHODS_FIXED.to_string());
    }
    let wrapped_key = key_file
        .wrap_master_key(db.key().as_bytes())
        .map_err(|e| format!("Failed to wrap master key for key file: {}", e))?;
    let now = chrono::Utc::now().to_rfc3339();
    crate::db::insert_auth_slot(
        db,
        "keypair",
        label,
        Some(&key_file.slot_id()),
        &wrapped_key,
        &now,
    )
}

/// Wraps the master key for an X25519 public key (the **legacy**, quantum-vulnerable
/// key-file format) and stores it as a `keypair` slot. Test fixtures only: production code
/// creates key-file slots with [`add_key_file_slot`].
#[cfg(any(test, feature = "test-support"))]
pub fn add_keypair_slot(
    db: &crate::db::DatabaseConnection,
    label: &str,
    public_key: [u8; 32],
) -> Result<i64, String> {
    let method = keypair::KeypairMethod { public_key };
    let wrapped_key = method
        .wrap_master_key(db.key().as_bytes())
        .map_err(|e| format!("Failed to wrap master key for keypair: {}", e))?;
    let now = chrono::Utc::now().to_rfc3339();
    crate::db::insert_auth_slot(db, "keypair", label, Some(&public_key), &wrapped_key, &now)
}

#[cfg(test)]
mod slot_tests {
    use crate::db::schema::create_database;

    #[test]
    fn test_weak_password_slot_is_strengthened_in_place() {
        use crate::db::schema::open_database;
        let tmp = tempfile::Builder::new().suffix(".db").tempfile().unwrap();
        let db_path = tmp.path().to_str().unwrap().to_string();
        let backups = tempfile::tempdir().unwrap();

        let db = create_database(&db_path, "pw".to_string()).unwrap();
        // Rewrite the slot as the earlier 64 MiB / t=3 settings would have.
        let (slot_id, strong) = crate::db::get_password_slot(&db).unwrap().unwrap();
        assert!(!super::strengthen_password_slot(&db, "pw").unwrap());
        let salt = &strong[17..33];
        let weak_params = crate::crypto::password::KdfParams {
            m_cost_kib: 65_536,
            t_cost: 3,
            p_cost: 4,
        };
        let key = crate::crypto::password::derive_wrapping_key(b"pw", salt, weak_params).unwrap();
        let mut header = strong[..33].to_vec();
        header[5..9].copy_from_slice(&65_536u32.to_le_bytes());
        header[9..13].copy_from_slice(&3u32.to_le_bytes());
        let enc = crate::crypto::cipher::encrypt_with_aad(
            &crate::crypto::cipher::Key::from_slice(key.as_ref()).unwrap(),
            db.key().as_bytes(),
            &header,
        )
        .unwrap();
        let mut weak = header;
        weak.extend_from_slice(&enc);
        crate::db::update_auth_slot_wrapped_key(&db, slot_id, &weak).unwrap();
        assert!(super::PasswordMethod::has_weak_params(&weak));

        assert!(super::strengthen_password_slot(&db, "wrong").is_err());
        assert!(super::strengthen_password_slot(&db, "pw").unwrap());
        let (_, now) = crate::db::get_password_slot(&db).unwrap().unwrap();
        assert!(!super::PasswordMethod::has_weak_params(&now));
        drop(db);
        assert!(open_database(&db_path, "pw".to_string(), backups.path()).is_ok());
    }

    #[test]
    fn test_add_password_slot_wraps_and_unlocks() {
        use crate::db::schema::open_database;
        let tmp = tempfile::Builder::new().suffix(".db").tempfile().unwrap();
        let db_path = tmp.path().to_str().unwrap().to_string();
        let backups = tempfile::tempdir().unwrap();

        // Create with an initial password, then remove that slot so the only path
        // back in is the newly added one.
        let db = create_database(&db_path, "original".to_string()).unwrap();
        let (orig_id, _) = crate::db::get_password_slot(&db).unwrap().unwrap();
        let new_id = super::add_password_slot(&db, "Password", "second-pass").unwrap();
        assert!(new_id > 0);
        crate::db::delete_auth_slot(&db, orig_id).unwrap();
        drop(db);

        // Unlock with the newly wrapped password.
        let db2 = open_database(&db_path, "second-pass".to_string(), backups.path()).unwrap();
        assert_eq!(crate::db::count_auth_slots(&db2).unwrap(), 1);
    }

    #[test]
    fn test_add_keypair_slot_wraps_and_unlocks() {
        use crate::db::schema::open_database_with_keypair;
        let tmp = tempfile::Builder::new().suffix(".db").tempfile().unwrap();
        let db_path = tmp.path().to_str().unwrap().to_string();
        let backups = tempfile::tempdir().unwrap();

        let db = create_database(&db_path, "original".to_string()).unwrap();
        let kp = crate::auth::keypair::generate_keypair().unwrap();
        let mut pub_key = [0u8; 32];
        pub_key.copy_from_slice(&hex::decode(&kp.public_key_hex).unwrap());
        let mut priv_key = [0u8; 32];
        priv_key.copy_from_slice(&hex::decode(&kp.private_key_hex).unwrap());

        let slot_id = super::add_keypair_slot(&db, "My Key", pub_key).unwrap();
        assert!(slot_id > 0);
        drop(db);

        let db2 = open_database_with_keypair(&db_path, priv_key, backups.path()).unwrap();
        assert_eq!(crate::db::count_auth_slots(&db2).unwrap(), 2);
    }
}
