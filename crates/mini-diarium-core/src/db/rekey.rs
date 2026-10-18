//! Master-key rotation for journals that still carry legacy key-wrapping material.
//!
//! Two legacy slot formats expose the master key to anyone holding a copy of the journal (or
//! of any backup):
//!
//! - **Password slots before format v2** derived their wrapping key from the Argon2 hash
//!   *stored inside the slot*, so the key could be unwrapped without the password — today.
//! - **X25519 key-file slots** store the recipient public key. A large quantum computer
//!   (Shor's algorithm) could recover the private key from it and unwrap the master key
//!   without the key file — "harvest now, decrypt later".
//!
//! Re-wrapping the same master key in a new format is not enough: every existing copy still
//! yields that key, and it would keep decrypting everything written from now on. So the
//! master key is replaced:
//!
//! 1. a fresh random master key is generated;
//! 2. every auth slot is re-wrapped for it (see [`RotationCredentials`] for what each slot
//!    type needs). A password or v2 key-file slot can only be re-wrapped with its own
//!    credential, so rotation fails with [`ERR_NEEDS_ALL_METHODS`] — changing nothing —
//!    unless the caller supplies every one of them;
//! 3. every encrypted column is decrypted with the old key and re-encrypted with the new one,
//!    and the master-key–keyed fingerprints (tags, images, attachments) and the
//!    `require_all_auth` MAC are recomputed;
//! 4. the legacy v1/v2 `metadata` password hash is deleted;
//!
//! all inside one write transaction, so a failure leaves the journal exactly as it was. After
//! the commit the file is `VACUUM`ed so the old blobs do not linger in free pages.
//!
//! Snapshots taken before the rotation are still encrypted with the old key and still hold the
//! legacy slots. They cannot be restored over the rotated journal (restore requires the same
//! master key) and should be deleted once the user has a post-rotation backup.

use crate::auth::{AutoKeyMethod, KeyFileMethod, KeypairMethod, PasswordMethod};
use crate::crypto::cipher;
use crate::db::queries::with_write_transaction;
use crate::db::schema::DatabaseConnection;
use log::{info, warn};
use rand_core::{OsRng, RngCore};
use rusqlite::{params, OptionalExtension};
use zeroize::Zeroizing;

/// Returned by [`rotate_master_key`] when a password or key-file slot cannot be re-wrapped
/// because its credential was not supplied. Nothing was changed.
pub const ERR_NEEDS_ALL_METHODS: &str =
    "Upgrading this journal's encryption needs all of its unlock methods at once";

/// The credentials available for re-wrapping slots during [`rotate_master_key`].
///
/// - `password` re-wraps the password slot it opens (v2 format).
/// - `key_files` re-wrap the key-file slots they open; a legacy X25519 slot is converted to
///   the v2 (symmetric) format. A legacy X25519 slot whose file is absent is re-wrapped with
///   its stored public key, which keeps it working but leaves it quantum-vulnerable and
///   counted in [`RekeyOutcome::legacy_key_file_slots_remaining`].
/// - `auto_key` re-wraps the local-key slot; without it that slot is removed.
pub struct RotationCredentials<'a> {
    pub password: Option<&'a str>,
    pub key_files: &'a [KeyFileMethod],
    pub auto_key: Option<&'a [u8; 32]>,
}

/// What [`rotate_master_key`] did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct RekeyOutcome {
    /// Number of encrypted values re-encrypted under the new master key.
    pub values_reencrypted: usize,
    /// Local-key slots that could not be re-wrapped and were removed.
    pub slots_removed: usize,
    /// Legacy X25519 key-file slots kept (re-wrapped by public key) because their key file was
    /// not supplied. Until they are upgraded the journal is not quantum-resistant.
    pub legacy_key_file_slots_remaining: usize,
}

/// Legacy key-wrapping material still present in a journal.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct LegacyMaterial {
    /// A pre-v2 password slot or the v1/v2 `metadata` password hash.
    pub password: bool,
    /// Public keys of the legacy X25519 key-file slots.
    pub key_file_public_keys: Vec<[u8; 32]>,
}

impl LegacyMaterial {
    pub fn any(&self) -> bool {
        self.password || !self.key_file_public_keys.is_empty()
    }

    /// True when `key_files` include the file for every legacy key-file slot.
    pub fn key_files_cover(&self, key_files: &[KeyFileMethod]) -> bool {
        self.key_file_public_keys
            .iter()
            .all(|pk| key_files.iter().any(|kf| &kf.legacy_public_key() == pk))
    }
}

/// Encrypted columns that hold `cipher::encrypt` output keyed by the master key.
/// (table, column). Every table listed has a rowid.
const ENCRYPTED_COLUMNS: &[(&str, &str)] = &[
    ("entries", "title_encrypted"),
    ("entries", "text_encrypted"),
    ("entries", "entry_metadata_encrypted"),
    ("entries", "preview_enc"),
    ("tags", "name_encrypted"),
    ("images", "data"),
    ("images", "thumbnail_data"),
    ("attachments", "data"),
    ("entry_attachments", "name_encrypted"),
];

/// Returns the legacy key-wrapping material still in the journal.
pub fn legacy_material(db: &DatabaseConnection) -> Result<LegacyMaterial, String> {
    legacy_material_conn(db.conn())
}

/// Returns true when the journal still carries legacy password material: a password slot in
/// the pre-v2 format, or the v1/v2 `metadata` password hash. Either one exposes the current
/// master key (or an older key) to anyone holding the file.
pub fn has_legacy_password_material(db: &DatabaseConnection) -> Result<bool, String> {
    Ok(legacy_material(db)?.password)
}

/// [`legacy_material`] on a raw connection; needs no key, so the locked-journal peek uses it.
pub(crate) fn legacy_material_conn(conn: &rusqlite::Connection) -> Result<LegacyMaterial, String> {
    let rows: Vec<(String, Option<Vec<u8>>, Vec<u8>)> = {
        let mut stmt = conn
            .prepare(
                "SELECT type, public_key, wrapped_key FROM auth_slots \
                 WHERE type IN ('password', 'keypair')",
            )
            .map_err(|e| format!("Failed to read auth slots: {}", e))?;
        let rows = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .map_err(|e| format!("Failed to read auth slots: {}", e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("Failed to read auth slots: {}", e))?;
        rows
    };
    let mut material = LegacyMaterial::default();
    for (slot_type, public_key, wrapped) in rows {
        match slot_type.as_str() {
            "password" if PasswordMethod::is_legacy_blob(&wrapped) => material.password = true,
            "keypair" if !KeyFileMethod::is_v2_blob(&wrapped) => {
                if let Some(pk) = public_key
                    .as_deref()
                    .and_then(|p| <[u8; 32]>::try_from(p).ok())
                {
                    material.key_file_public_keys.push(pk);
                }
            }
            _ => {}
        }
    }
    if !material.password {
        material.password = legacy_metadata_present(conn)?;
    }
    Ok(material)
}

fn table_exists(conn: &rusqlite::Connection, table: &str) -> Result<bool, String> {
    conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
        params![table],
        |_| Ok(()),
    )
    .optional()
    .map(|r| r.is_some())
    .map_err(|e| format!("Failed to inspect schema: {}", e))
}

fn legacy_metadata_present(conn: &rusqlite::Connection) -> Result<bool, String> {
    if !table_exists(conn, "metadata")? {
        return Ok(false);
    }
    conn.query_row(
        "SELECT 1 FROM metadata WHERE key IN ('password_hash', 'salt') LIMIT 1",
        [],
        |_| Ok(()),
    )
    .optional()
    .map(|r| r.is_some())
    .map_err(|e| format!("Failed to inspect metadata: {}", e))
}

/// Replaces the journal's master key and re-wraps every auth slot for it.
///
/// See the module docs for the full procedure and [`RotationCredentials`] for what each slot
/// needs. Fails with [`ERR_NEEDS_ALL_METHODS`] when a password or v2 key-file slot's
/// credential is missing. On any error nothing is changed.
pub fn rotate_master_key(
    db: &mut DatabaseConnection,
    credentials: &RotationCredentials<'_>,
) -> Result<RekeyOutcome, String> {
    // Rotation writes the full new key into every slot; a split-key journal would lose its
    // split. Such a journal never has legacy material (splitting requires none), so this
    // is only a guard.
    if crate::db::uses_key_shares(db) {
        return Err(crate::db::ERR_SHARED_METHODS_FIXED.to_string());
    }
    let mut new_key_bytes = Zeroizing::new([0u8; 32]);
    OsRng.fill_bytes(new_key_bytes.as_mut());
    let new_key = cipher::Key::from_slice(new_key_bytes.as_ref()).ok_or("Invalid key size")?;

    let outcome = with_write_transaction(db, || {
        let conn = db.conn();
        let old_key = db.key();
        // First, so a missing credential fails before any re-encryption work is done.
        let (slots_removed, legacy_key_file_slots_remaining) =
            rewrap_slots(conn, old_key, &new_key_bytes, credentials)?;
        let mut outcome = RekeyOutcome {
            values_reencrypted: 0,
            slots_removed,
            legacy_key_file_slots_remaining,
        };

        for (table, column) in ENCRYPTED_COLUMNS {
            if table_exists(conn, table)? {
                outcome.values_reencrypted +=
                    reencrypt_column(conn, old_key, &new_key, table, column)?;
            }
        }
        recompute_fingerprints(conn, &new_key)?;

        if crate::db::queries::db_settings::get_db_setting_conn(conn, "require_all_auth_mac")
            .is_some()
        {
            crate::db::queries::db_settings::write_require_all_auth_mac_conn(
                conn,
                new_key.as_bytes(),
            )?;
        }

        // Re-MAC the clock high-water mark for the new key (an invalid one stays invalid).
        use crate::clock::{decode_mark, encode_mark, HIGH_WATER_SETTING};
        use crate::db::queries::db_settings::{get_db_setting_conn, set_db_setting_conn};
        if let Some(stored) = get_db_setting_conn(conn, crate::deadman::JOURNAL_SETTING) {
            if let Some(dm) = crate::deadman::decode(old_key.as_bytes(), &stored) {
                set_db_setting_conn(
                    conn,
                    crate::deadman::JOURNAL_SETTING,
                    &crate::deadman::encode(new_key.as_bytes(), &dm),
                )?;
            }
        }
        if let Some(stored) = get_db_setting_conn(conn, HIGH_WATER_SETTING) {
            if let Some(secs) = decode_mark(old_key.as_bytes(), &stored) {
                set_db_setting_conn(
                    conn,
                    HIGH_WATER_SETTING,
                    &encode_mark(new_key.as_bytes(), secs),
                )?;
            }
        }

        if table_exists(conn, "metadata")? {
            conn.execute(
                "DELETE FROM metadata WHERE key IN ('password_hash', 'salt')",
                [],
            )
            .map_err(|e| format!("Failed to remove legacy password hash: {}", e))?;
        }
        Ok(outcome)
    })?;

    db.encryption_key = new_key;

    // Old ciphertext and the legacy slot now sit in free pages; rewrite the file without them.
    if let Err(e) = db.conn().execute_batch("VACUUM") {
        warn!(
            "Key rotation: VACUUM failed, old data may remain in free pages: {}",
            e
        );
    }

    info!(
        "Master key rotated: {} values re-encrypted, {} slot(s) removed, {} legacy key-file slot(s) left",
        outcome.values_reencrypted, outcome.slots_removed, outcome.legacy_key_file_slots_remaining
    );
    Ok(outcome)
}

fn reencrypt_column(
    conn: &rusqlite::Connection,
    old_key: &cipher::Key,
    new_key: &cipher::Key,
    table: &str,
    column: &str,
) -> Result<usize, String> {
    // Table and column names come from the ENCRYPTED_COLUMNS constant, never from input.
    let rowids: Vec<i64> = {
        let mut stmt = conn
            .prepare(&format!(
                "SELECT rowid FROM {table} WHERE {column} IS NOT NULL"
            ))
            .map_err(|e| format!("Failed to read {table}.{column}: {}", e))?;
        let rows = stmt
            .query_map([], |row| row.get(0))
            .map_err(|e| format!("Failed to read {table}.{column}: {}", e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("Failed to read {table}.{column}: {}", e))?;
        rows
    };

    let select = format!("SELECT {column} FROM {table} WHERE rowid = ?1");
    let update = format!("UPDATE {table} SET {column} = ?1 WHERE rowid = ?2");
    for rowid in &rowids {
        let blob: Vec<u8> = conn
            .query_row(&select, params![rowid], |row| row.get(0))
            .map_err(|e| format!("Failed to read {table}.{column} row {rowid}: {}", e))?;
        let plaintext = Zeroizing::new(
            cipher::decrypt(old_key, &blob)
                .map_err(|e| format!("Cannot rotate key: {table}.{column} row {rowid}: {e}"))?,
        );
        let reencrypted = cipher::encrypt(new_key, &plaintext)
            .map_err(|e| format!("Failed to re-encrypt {table}.{column}: {}", e))?;
        conn.execute(&update, params![reencrypted, rowid])
            .map_err(|e| format!("Failed to update {table}.{column} row {rowid}: {}", e))?;
    }
    Ok(rowids.len())
}

type FingerprintFn = fn(&cipher::Key, &[u8]) -> String;

/// (id, type, public_key, wrapped_key) of one `auth_slots` row.
type SlotRow = (i64, String, Option<Vec<u8>>, Vec<u8>);

/// Recomputes the master-key–keyed fingerprints from the (already re-encrypted) plaintexts.
fn recompute_fingerprints(
    conn: &rusqlite::Connection,
    new_key: &cipher::Key,
) -> Result<(), String> {
    let jobs: [(&str, &str, &str, FingerprintFn); 3] = [
        ("tags", "name_encrypted", "name_fingerprint", |k, p| {
            cipher::tag_name_fingerprint(k, &String::from_utf8_lossy(p))
        }),
        ("images", "data", "fingerprint", cipher::image_fingerprint),
        (
            "attachments",
            "data",
            "fingerprint",
            cipher::image_fingerprint,
        ),
    ];
    for (table, data_col, fp_col, fingerprint) in jobs {
        if !table_exists(conn, table)? {
            continue;
        }
        let rows: Vec<(i64, Vec<u8>)> = {
            let mut stmt = conn
                .prepare(&format!("SELECT rowid, {data_col} FROM {table}"))
                .map_err(|e| format!("Failed to read {table}: {}", e))?;
            let rows = stmt
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                .map_err(|e| format!("Failed to read {table}: {}", e))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| format!("Failed to read {table}: {}", e))?;
            rows
        };
        let update = format!("UPDATE {table} SET {fp_col} = ?1 WHERE rowid = ?2");
        for (rowid, blob) in rows {
            let plaintext = Zeroizing::new(
                cipher::decrypt(new_key, &blob)
                    .map_err(|e| format!("Cannot rotate key: {table} row {rowid}: {e}"))?,
            );
            conn.execute(&update, params![fingerprint(new_key, &plaintext), rowid])
                .map_err(|e| format!("Failed to update {table}.{fp_col}: {}", e))?;
        }
    }
    Ok(())
}

/// Re-wraps every auth slot for `new_key`. Returns (local-key slots removed, legacy key-file
/// slots kept). Fails — rolling everything back — on a password or v2 key-file slot whose
/// credential is missing ([`ERR_NEEDS_ALL_METHODS`]) or a slot type it does not know, rather
/// than delete a credential.
fn rewrap_slots(
    conn: &rusqlite::Connection,
    old_key: &cipher::Key,
    new_key: &[u8; 32],
    credentials: &RotationCredentials<'_>,
) -> Result<(usize, usize), String> {
    let slots: Vec<SlotRow> = {
        let mut stmt = conn
            .prepare("SELECT id, type, public_key, wrapped_key FROM auth_slots ORDER BY id ASC")
            .map_err(|e| format!("Failed to read auth slots: {}", e))?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .map_err(|e| format!("Failed to read auth slots: {}", e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("Failed to read auth slots: {}", e))?;
        rows
    };
    let opens = |unwrapped: Result<crate::auth::SecretBytes, String>| {
        unwrapped
            .map(|k| k[..] == old_key.as_bytes()[..])
            .unwrap_or(false)
    };

    let mut removed = 0;
    let mut legacy_remaining = 0;
    for (id, slot_type, public_key, wrapped) in slots {
        // (new wrapped_key, new public_key column value); None removes the slot.
        let rewrapped: Option<(Vec<u8>, Option<Vec<u8>>)> = match slot_type.as_str() {
            "password" => {
                let method = credentials
                    .password
                    .map(|p| PasswordMethod::new(p.to_string()))
                    .filter(|m| opens(m.unwrap_master_key(&wrapped)))
                    .ok_or(ERR_NEEDS_ALL_METHODS)?;
                Some((method.wrap_master_key(new_key)?, None))
            }
            "keypair" => {
                let public_key: [u8; 32] = public_key
                    .as_deref()
                    .and_then(|p| p.try_into().ok())
                    .ok_or_else(|| format!("Key-file slot {id} has no valid public key"))?;
                let legacy = !KeyFileMethod::is_v2_blob(&wrapped);
                let file = credentials.key_files.iter().find(|kf| {
                    let id_matches = if legacy {
                        kf.legacy_public_key() == public_key
                    } else {
                        kf.slot_id() == public_key
                    };
                    id_matches && opens(kf.unwrap_any(&wrapped))
                });
                match (file, legacy) {
                    // Upgrade (or re-wrap) to the v2 format with the supplied file.
                    (Some(kf), _) => {
                        Some((kf.wrap_master_key(new_key)?, Some(kf.slot_id().to_vec())))
                    }
                    // A legacy slot can still be re-wrapped from its public key.
                    (None, true) => {
                        legacy_remaining += 1;
                        Some((KeypairMethod { public_key }.wrap_master_key(new_key)?, None))
                    }
                    (None, false) => return Err(ERR_NEEDS_ALL_METHODS.to_string()),
                }
            }
            "auto" => match credentials.auto_key {
                Some(auto_key_bytes) => {
                    let method = AutoKeyMethod { auto_key_bytes };
                    if opens(method.unwrap_master_key(&wrapped)) {
                        Some((method.wrap_master_key(new_key)?, None))
                    } else {
                        None
                    }
                }
                None => None,
            },
            other => {
                return Err(format!(
                    "Cannot rotate key: unknown auth slot type '{other}'"
                ))
            }
        };

        match rewrapped {
            Some((blob, new_public_key)) => {
                conn.execute(
                    "UPDATE auth_slots SET wrapped_key = ?1 WHERE id = ?2",
                    params![blob, id],
                )
                .map_err(|e| format!("Failed to update auth slot {id}: {}", e))?;
                if let Some(pk) = new_public_key {
                    conn.execute(
                        "UPDATE auth_slots SET public_key = ?1 WHERE id = ?2",
                        params![pk, id],
                    )
                    .map_err(|e| format!("Failed to update auth slot {id}: {}", e))?;
                }
            }
            None => {
                warn!("Key rotation: removing {slot_type} slot {id}, it cannot be re-wrapped");
                conn.execute("DELETE FROM auth_slots WHERE id = ?1", params![id])
                    .map_err(|e| format!("Failed to remove auth slot {id}: {}", e))?;
                removed += 1;
            }
        }
    }
    Ok((removed, legacy_remaining))
}

/// Rewrites the journal's password slot in the legacy (pre-v2) format for the same master
/// key, exactly as older releases stored it. Test fixtures only.
#[cfg(any(test, feature = "test-support"))]
pub fn downgrade_password_slot_for_tests(
    db: &DatabaseConnection,
    password: &str,
) -> Result<(), String> {
    use crate::crypto::password as pwd;
    let (slot_id, _) = crate::db::get_password_slot(db)?.ok_or("No password slot")?;
    let phc_hash = pwd::hash_password(password.to_string(), &pwd::generate_salt())
        .map_err(|e| e.to_string())?;
    let key = pwd::derive_key_from_phc_hash(&phc_hash)?;
    let wrap_key = cipher::Key::from_slice(&key).ok_or("Invalid key size")?;
    let encrypted = cipher::encrypt(&wrap_key, db.key().as_bytes()).map_err(|e| e.to_string())?;
    let mut blob = (phc_hash.len() as u32).to_le_bytes().to_vec();
    blob.extend_from_slice(phc_hash.as_bytes());
    blob.extend_from_slice(&encrypted);
    crate::db::update_auth_slot_wrapped_key(db, slot_id, &blob)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::queries::images::test_support::valid_png_bytes;
    use crate::db::queries::images::upsert_image;
    use crate::db::schema::{
        create_database, create_database_auto, open_database, open_database_auto,
        open_database_with_keypair,
    };
    use crate::db::{
        add_attachment_to_entry, add_tag_to_entry, create_tag, get_all_tags, get_entry_by_id,
        get_image_by_id, insert_entry, list_entry_attachments, read_attachment_bytes, DiaryEntry,
        EntryMetadata,
    };

    fn downgrade_password_slot(db: &DatabaseConnection, password: &str) {
        downgrade_password_slot_for_tests(db, password).unwrap();
    }

    fn creds<'a>(
        password: Option<&'a str>,
        key_files: &'a [KeyFileMethod],
        auto_key: Option<&'a [u8; 32]>,
    ) -> RotationCredentials<'a> {
        RotationCredentials {
            password,
            key_files,
            auto_key,
        }
    }

    /// Adds a legacy X25519 key-file slot; returns the key-file bytes (the private key).
    fn add_legacy_key_file(db: &DatabaseConnection) -> [u8; 32] {
        let kp = crate::auth::generate_keypair().unwrap();
        let mut public_key = [0u8; 32];
        public_key.copy_from_slice(&hex::decode(&kp.public_key_hex).unwrap());
        let mut private_key = [0u8; 32];
        private_key.copy_from_slice(&hex::decode(&kp.private_key_hex).unwrap());
        crate::auth::add_keypair_slot(db, "Old key", public_key).unwrap();
        private_key
    }

    fn keypair_slots(db: &DatabaseConnection) -> Vec<(Vec<u8>, Vec<u8>)> {
        let mut stmt = db
            .conn()
            .prepare("SELECT public_key, wrapped_key FROM auth_slots WHERE type = 'keypair'")
            .unwrap();
        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        rows
    }

    struct Fixture {
        entry_id: i64,
        image_id: i64,
        attachment_id: i64,
    }

    fn populate(db: &DatabaseConnection) -> Fixture {
        let entry = DiaryEntry {
            id: 0,
            date: "2026-01-02".to_string(),
            title: "Private title".to_string(),
            text: "<p>Private body text</p>".to_string(),
            word_count: 3,
            date_created: "2026-01-02T00:00:00Z".to_string(),
            date_updated: "2026-01-02T00:00:00Z".to_string(),
            metadata: Some(EntryMetadata {
                font_family: Some("Serif".to_string()),
                font_size: Some(14.0),
            }),
            locked: false,
        };
        let entry_id = insert_entry(db, &entry).unwrap();
        let tag = create_tag(db, "secret-tag").unwrap();
        add_tag_to_entry(db, entry_id, tag.id).unwrap();
        let image_id = upsert_image(db, "image/png", &valid_png_bytes()).unwrap();
        let attachment_id = add_attachment_to_entry(db, entry_id, "notes.txt", b"attached")
            .unwrap()
            .id;
        Fixture {
            entry_id,
            image_id,
            attachment_id,
        }
    }

    fn assert_content_intact(db: &DatabaseConnection, f: &Fixture) {
        let entry = get_entry_by_id(db, f.entry_id).unwrap().unwrap();
        assert_eq!(entry.title, "Private title");
        assert_eq!(entry.text, "<p>Private body text</p>");
        assert_eq!(
            entry.metadata.unwrap().font_family.as_deref(),
            Some("Serif")
        );
        let tags = get_all_tags(db).unwrap();
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].name, "secret-tag");
        assert!(get_image_by_id(db, f.image_id).unwrap().is_some());
        let bytes = read_attachment_bytes(db, f.entry_id, f.attachment_id)
            .unwrap()
            .unwrap();
        assert_eq!(bytes.as_slice(), b"attached");
        assert_eq!(
            list_entry_attachments(db, f.entry_id).unwrap()[0].name,
            "notes.txt"
        );
        // Keyed fingerprints were recomputed: re-adding the same tag or image deduplicates.
        assert_eq!(create_tag(db, "secret-tag").unwrap().id, tags[0].id);
        assert_eq!(get_all_tags(db).unwrap().len(), 1);
        assert_eq!(
            upsert_image(db, "image/png", &valid_png_bytes()).unwrap(),
            f.image_id
        );
    }

    fn all_encrypted_blobs(db: &DatabaseConnection) -> Vec<Vec<u8>> {
        let mut blobs = Vec::new();
        for (table, column) in ENCRYPTED_COLUMNS {
            let mut stmt = db
                .conn()
                .prepare(&format!(
                    "SELECT {column} FROM {table} WHERE {column} IS NOT NULL ORDER BY rowid"
                ))
                .unwrap();
            let rows = stmt.query_map([], |row| row.get::<_, Vec<u8>>(0)).unwrap();
            blobs.extend(rows.map(|r| r.unwrap()));
        }
        blobs
    }

    #[test]
    fn rotation_replaces_key_rewraps_slots_and_keeps_content() {
        let tmp = tempfile::Builder::new().suffix(".db").tempfile().unwrap();
        let path = tmp.path().to_path_buf();
        let backups = tempfile::tempdir().unwrap();

        let mut db = create_database(&path, "pw".to_string()).unwrap();
        let fixture = populate(&db);
        let kp = crate::auth::generate_keypair().unwrap();
        let mut public_key = [0u8; 32];
        public_key.copy_from_slice(&hex::decode(&kp.public_key_hex).unwrap());
        let mut private_key = [0u8; 32];
        private_key.copy_from_slice(&hex::decode(&kp.private_key_hex).unwrap());
        crate::auth::add_keypair_slot(&db, "Key", public_key).unwrap();
        downgrade_password_slot(&db, "pw");
        db.conn()
            .execute(
                "INSERT INTO metadata (key, value) VALUES ('password_hash', 'x'), ('salt', 'y')",
                [],
            )
            .unwrap();
        assert!(has_legacy_password_material(&db).unwrap());

        let old_key = cipher::Key::from_slice(db.key().as_bytes()).unwrap();
        let outcome = rotate_master_key(&mut db, &creds(Some("pw"), &[], None)).unwrap();

        assert_eq!(outcome.slots_removed, 0);
        assert_eq!(outcome.legacy_key_file_slots_remaining, 1);
        assert!(outcome.values_reencrypted >= 8);
        assert_ne!(db.key().as_bytes(), old_key.as_bytes());
        assert!(!has_legacy_password_material(&db).unwrap());
        assert_content_intact(&db, &fixture);
        // Nothing in the journal still decrypts under the old (exposed) key.
        for blob in all_encrypted_blobs(&db) {
            assert!(cipher::decrypt(&old_key, &blob).is_err());
        }
        drop(db);

        let db = open_database(&path, "pw".to_string(), backups.path()).unwrap();
        assert_content_intact(&db, &fixture);
        drop(db);
        let db = open_database_with_keypair(&path, private_key, backups.path()).unwrap();
        assert_content_intact(&db, &fixture);
    }

    #[test]
    fn rotation_re_macs_the_clock_mark() {
        let tmp = tempfile::Builder::new().suffix(".db").tempfile().unwrap();
        let mut db = create_database(tmp.path(), "pw".to_string()).unwrap();
        downgrade_password_slot(&db, "pw");
        let mark = crate::clock::update_journal_mark(&db)
            .unwrap()
            .effective_now;
        crate::deadman::set_journal_dead_man(&db, Some(30)).unwrap();
        rotate_master_key(&mut db, &creds(Some("pw"), &[], None)).unwrap();
        assert_eq!(crate::deadman::journal_dead_man(&db).unwrap().days, 30);

        let check = crate::clock::update_journal_mark(&db).unwrap();
        assert!(!check.suspicious());
        assert!(check.effective_now >= mark);
    }

    #[test]
    fn rotation_with_wrong_password_changes_nothing() {
        let tmp = tempfile::Builder::new().suffix(".db").tempfile().unwrap();
        let mut db = create_database(tmp.path(), "pw".to_string()).unwrap();
        let fixture = populate(&db);
        downgrade_password_slot(&db, "pw");
        let old_key = *db.key().as_bytes();
        let blobs_before = all_encrypted_blobs(&db);

        assert!(rotate_master_key(&mut db, &creds(Some("wrong"), &[], None)).is_err());

        assert_eq!(db.key().as_bytes(), &old_key);
        assert_eq!(all_encrypted_blobs(&db), blobs_before);
        assert!(has_legacy_password_material(&db).unwrap());
        assert_content_intact(&db, &fixture);
    }

    #[test]
    fn local_key_slot_survives_when_key_supplied() {
        let tmp = tempfile::Builder::new().suffix(".db").tempfile().unwrap();
        let path = tmp.path().to_path_buf();
        let backups = tempfile::tempdir().unwrap();
        let auto_key = [7u8; 32];

        let mut db = create_database_auto(&path, &auto_key).unwrap();
        crate::auth::add_password_slot(&db, "Password", "pw").unwrap();
        downgrade_password_slot(&db, "pw");

        let outcome = rotate_master_key(&mut db, &creds(Some("pw"), &[], Some(&auto_key))).unwrap();
        assert_eq!(outcome.slots_removed, 0);
        drop(db);
        assert!(open_database_auto(&path, &auto_key, backups.path()).is_ok());
        assert!(open_database(&path, "pw".to_string(), backups.path()).is_ok());
    }

    #[test]
    fn local_key_slot_removed_when_key_missing() {
        let tmp = tempfile::Builder::new().suffix(".db").tempfile().unwrap();
        let path = tmp.path().to_path_buf();
        let backups = tempfile::tempdir().unwrap();
        let auto_key = [7u8; 32];

        let mut db = create_database_auto(&path, &auto_key).unwrap();
        crate::auth::add_password_slot(&db, "Password", "pw").unwrap();
        downgrade_password_slot(&db, "pw");

        let outcome = rotate_master_key(&mut db, &creds(Some("pw"), &[], None)).unwrap();
        assert_eq!(outcome.slots_removed, 1);
        drop(db);
        assert!(open_database_auto(&path, &auto_key, backups.path()).is_err());
        assert!(open_database(&path, "pw".to_string(), backups.path()).is_ok());
    }

    #[test]
    fn supplied_legacy_key_file_is_upgraded_to_v2() {
        let tmp = tempfile::Builder::new().suffix(".db").tempfile().unwrap();
        let path = tmp.path().to_path_buf();
        let backups = tempfile::tempdir().unwrap();

        let mut db = create_database(&path, "pw".to_string()).unwrap();
        let fixture = populate(&db);
        let file_bytes = add_legacy_key_file(&db);
        let legacy_public = KeyFileMethod::from_secret(file_bytes).legacy_public_key();
        let material = legacy_material(&db).unwrap();
        assert_eq!(material.key_file_public_keys, vec![legacy_public]);
        assert!(!material.password);

        let files = [KeyFileMethod::from_secret(file_bytes)];
        assert!(material.key_files_cover(&files));
        let outcome = rotate_master_key(&mut db, &creds(Some("pw"), &files, None)).unwrap();
        assert_eq!(outcome.legacy_key_file_slots_remaining, 0);

        assert!(!legacy_material(&db).unwrap().any());
        let slots = keypair_slots(&db);
        assert_eq!(slots.len(), 1);
        assert!(KeyFileMethod::is_v2_blob(&slots[0].1));
        assert_eq!(slots[0].0, files[0].slot_id().to_vec());
        // The X25519 public key is gone from the journal.
        assert_ne!(slots[0].0, legacy_public.to_vec());
        assert_content_intact(&db, &fixture);
        drop(db);

        // The same, unchanged key file still opens the journal; so does the password.
        let db = open_database_with_keypair(&path, file_bytes, backups.path()).unwrap();
        assert_content_intact(&db, &fixture);
        drop(db);
        assert!(open_database(&path, "pw".to_string(), backups.path()).is_ok());
    }

    #[test]
    fn key_file_only_journal_upgrades_with_its_file() {
        let tmp = tempfile::Builder::new().suffix(".db").tempfile().unwrap();
        let path = tmp.path().to_path_buf();
        let backups = tempfile::tempdir().unwrap();

        let mut db = create_database(&path, "pw".to_string()).unwrap();
        let file_bytes = add_legacy_key_file(&db);
        let (pw_slot, _) = crate::db::get_password_slot(&db).unwrap().unwrap();
        crate::db::delete_auth_slot(&db, pw_slot).unwrap();

        let files = [KeyFileMethod::from_secret(file_bytes)];
        rotate_master_key(&mut db, &creds(None, &files, None)).unwrap();
        assert!(!legacy_material(&db).unwrap().any());
        drop(db);
        assert!(open_database_with_keypair(&path, file_bytes, backups.path()).is_ok());
    }

    #[test]
    fn missing_v2_key_file_or_password_refuses_and_changes_nothing() {
        let tmp = tempfile::Builder::new().suffix(".db").tempfile().unwrap();
        let mut db = create_database(tmp.path(), "pw".to_string()).unwrap();
        let fixture = populate(&db);
        let key_file = KeyFileMethod::generate();
        crate::auth::add_key_file_slot(&db, "New key", &key_file).unwrap();
        let old_key = *db.key().as_bytes();
        let blobs_before = all_encrypted_blobs(&db);

        // Password only: the v2 key-file slot cannot be re-wrapped.
        let err = rotate_master_key(&mut db, &creds(Some("pw"), &[], None)).unwrap_err();
        assert_eq!(err, ERR_NEEDS_ALL_METHODS);
        // Key file only: the password slot cannot be re-wrapped.
        let files = [KeyFileMethod::from_secret(
            hex::decode(key_file.file_contents().as_str())
                .unwrap()
                .try_into()
                .unwrap(),
        )];
        let err = rotate_master_key(&mut db, &creds(None, &files, None)).unwrap_err();
        assert_eq!(err, ERR_NEEDS_ALL_METHODS);

        assert_eq!(db.key().as_bytes(), &old_key);
        assert_eq!(all_encrypted_blobs(&db), blobs_before);
        assert_content_intact(&db, &fixture);

        // Both together succeed.
        rotate_master_key(&mut db, &creds(Some("pw"), &files, None)).unwrap();
        assert_ne!(db.key().as_bytes(), &old_key);
        assert_content_intact(&db, &fixture);
    }

    #[test]
    fn new_journals_have_no_legacy_material() {
        let tmp = tempfile::Builder::new().suffix(".db").tempfile().unwrap();
        let db = create_database(tmp.path(), "pw".to_string()).unwrap();
        assert!(!has_legacy_password_material(&db).unwrap());
    }
}
